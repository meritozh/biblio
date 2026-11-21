use crate::{app::App, ui::*};
use biblio_core::{
    Library,
    commands::{author, file::FilterCondition, tag},
};
use nsh_core::prelude::*;
use serde_json::json;

#[derive(Clone, Debug, PartialEq)]
pub struct Chip(pub String, pub FilterCondition);
#[derive(Clone, PartialEq)]
enum Candidate {
    Field(&'static str),
    Value(Chip),
}
impl Candidate {
    fn label(&self) -> String {
        match self {
            Self::Field(f) => format!("{f}…"),
            Self::Value(c) => c.0.clone(),
        }
    }
    fn kind(&self) -> &'static str {
        match self {
            Self::Field(_) => "Filter by",
            Self::Value(c) if c.0.starts_with("Tag · ") => "Tag",
            Self::Value(c) if c.0.starts_with("Author · ") => "Author",
            Self::Value(_) => "Filter",
        }
    }
}
const FIELDS: &[(&str, &[&str])] = &[
    ("Authors", &["author", "by"]),
    ("Tags", &["tag"]),
    ("Cached", &["cache"]),
    ("Status", &["status"]),
    ("Storage", &["store"]),
    ("Favorite", &["star", "fav", "favourite"]),
    ("Progress", &["progress"]),
];
async fn suggestions(
    db: Library,
    field: Option<&'static str>,
    query: String,
) -> Result<Vec<Candidate>, String> {
    let q = query.trim().to_lowercase();
    let mut out = Vec::new();
    let matches = |f| field == Some(f) || (field.is_none() && !q.is_empty());
    let condition = |name: &str, op: &str| FilterCondition {
        field: name.into(),
        op: op.into(),
        ..Default::default()
    };
    if matches("Tags") {
        for t in tag::tag_list(db.clone(), None, Some(8), None, Some(q.clone()))
            .await?
            .tags
        {
            out.push(Candidate::Value(Chip(
                format!("Tag · {}", t.name),
                FilterCondition {
                    tag_id: Some(t.id),
                    ..condition("tags", "includes")
                },
            )));
        }
    }
    if matches("Authors") {
        for a in author::author_list(db, None, Some(8), None, Some(q.clone()))
            .await?
            .authors
        {
            out.push(Candidate::Value(Chip(
                format!("Author · {}", a.name),
                FilterCondition {
                    author_id: Some(a.id),
                    ..condition("authors", "includes")
                },
            )));
        }
    }
    for (group, name, values) in [
        (
            "Cached",
            "local_cache",
            &[("Cached", "not_empty"), ("Not cached", "empty")][..],
        ),
        (
            "Status",
            "file_status",
            &[
                ("Available", "available"),
                ("Missing", "missing"),
                ("Moved", "moved"),
            ],
        ),
        (
            "Storage",
            "storage_kind",
            &[("Local", "local"), ("Remote", "remote")],
        ),
        (
            "Favorite",
            "favorite",
            &[("Favorited", "true"), ("Not favorited", "false")],
        ),
    ] {
        if matches(group) {
            for &(label, value) in values {
                if label.to_lowercase().contains(&q) {
                    let c = if group == "Cached" {
                        condition(name, value)
                    } else {
                        FilterCondition {
                            value: Some(if group == "Favorite" {
                                json!(value == "true")
                            } else {
                                json!(value)
                            }),
                            ..condition(name, "is")
                        }
                    };
                    out.push(Candidate::Value(Chip(label.into(), c)));
                }
            }
        }
    }
    if field == Some("Progress") && !q.is_empty() {
        out.push(Candidate::Value(Chip(
            format!("Progress · {}", query.trim()),
            FilterCondition {
                text: Some(query.trim().into()),
                ..condition("progress", "contains")
            },
        )));
    }
    if field.is_none() {
        for &(name, aliases) in FIELDS {
            if name.to_lowercase().starts_with(&q) || aliases.iter().any(|a| a.contains(&q)) {
                out.push(Candidate::Field(name));
            }
        }
    }
    out.truncate(12);
    Ok(out)
}

pub fn bar(app: App, committed: Signal<String>, chips: Signal<Vec<Chip>>) -> AnyElement {
    let input_ref = element_ref();
    let buffer = signal(committed.get_untracked());
    let field = signal(None::<&'static str>);
    let prefix = signal(None::<String>);
    let choices = signal(Vec::<Candidate>::new());
    let active = signal(None::<usize>);
    let focus = signal(false);
    let composing = signal(false);
    let open = signal(false);
    effect(move || {
        let text = buffer.get();
        let mode = field.get();
        let saved = prefix.get();
        if !open.get() {
            return;
        }
        let q = saved
            .as_ref()
            .and_then(|p| text.strip_prefix(p))
            .unwrap_or(&text)
            .trim()
            .to_owned();
        let db = app.db();
        let task = crate::app::run_service(async move {
            tokio::time::sleep(std::time::Duration::from_millis(120)).await;
            suggestions(db, mode, q).await
        });
        let abort = task.abort_handle();
        cleanup(move || abort.abort());
        spawn(async move {
            if let Ok(Ok(rows)) = task.await {
                choices.set(rows);
                if mode.is_none() {
                    committed.set(text);
                }
            }
        });
    });
    let select = move |c: Candidate| {
        match c {
            Candidate::Field(f) => {
                field.set(Some(f));
                prefix.set(Some(buffer.get_untracked()));
                open.set(true);
            }
            Candidate::Value(c) => {
                chips.update(|v| v.push(c));
                field.set(None);
                prefix.set(None);
                // The typed text only found the suggestion: it is not a
                // search of its own once the condition stands in for it.
                open.set(false);
                buffer.set(String::new());
                committed.set(String::new());
            }
        }
        choices.set(vec![]);
        active.set(None);
        if !focus.get_untracked() {
            input_ref.focus();
        }
    };
    let accept = move || {
        if let Some(c) = active
            .get_untracked()
            .and_then(|i| choices.get_untracked().get(i).cloned())
        {
            select(c);
        } else if field.get_untracked() == Some("Progress") {
            if let Some(c) = choices.get_untracked().first().cloned() {
                select(c);
            }
        } else if field.get_untracked().is_none() {
            committed.set(buffer.get_untracked());
            open.set(false);
        }
    };
    effect(move || {
        if !focus.get() || composing.get() || !open.get() {
            return;
        }
        let n = choices.get().len();
        let opts = KeyOptions {
            when: nsh_core::keymap::When::Always,
            repeat: true,
        };
        if n > 0 {
            for (key, forward) in [("down", true), ("up", false)] {
                on_key_with(key, opts, move |_| {
                    active.update(|at| {
                        *at = Some(match *at {
                            None if forward => 0,
                            None => n - 1,
                            Some(i) if forward => (i + 1) % n,
                            Some(i) => (i + n - 1) % n,
                        })
                    })
                });
            }
            if active.get().is_some() {
                on_key_with("tab", opts, move |_| accept());
            }
        }
        on_key_with("escape", opts, move |_| {
            open.set(false);
            field.set(None);
            prefix.set(None);
            active.set(None);
        });
    });
    // Focus the field when the app asks (mod+K), also when the request
    // came before this field existed (from another page): once mounted.
    effect(move || {
        let asked = app.find.get();
        if app.found.with(|f| *f != asked) {
            app.found.with(|f| *f = asked);
            spawn(async move {
                timer(std::time::Duration::from_millis(16)).await;
                input_ref.focus();
            });
        }
    });
    let entry = input_face()
        .label("Search library")
        .submit_behavior(SubmitBehavior::Submit)
        .placeholder("Search titles, paths, tags or authors")
        .value(move || buffer.get())
        .element_ref(&input_ref)
        .auto_correct(false)
        .auto_capitalize(AutoCapitalize::None)
        .on_focus(move || {
            focus.set(true);
            open.set(true);
        })
        .on_blur(move || {
            focus.set(false);
            spawn(async move {
                timer(std::time::Duration::from_millis(150)).await;
                if !focus.get_untracked() {
                    open.set(false);
                    active.set(None);
                    field.set(None);
                    prefix.set(None);
                }
            });
        })
        .on_composition_change(move |c: &Composition| composing.set(c.composing))
        .on_change(move |v: &str| {
            if prefix.get_untracked().is_some_and(|p| !v.starts_with(&p)) {
                prefix.set(None);
            }
            choices.set(vec![]);
            active.set(None);
            open.set(true);
            buffer.set(v.to_owned());
        })
        .on_submit(move |_: &str| {
            if !composing.get_untracked() {
                accept()
            }
        })
        .pl(px(34.))
        .pr(px(36.))
        .flex_1();
    // Clears the typed search; conditions have their own chips.
    let reset = move || {
        field.set(None);
        prefix.set(None);
        active.set(None);
        buffer.set(String::new());
        committed.set(String::new());
        open.set(false);
    };
    let suggestions = dynamic(move || {
        let list = choices.get();
        if !open.get() || list.is_empty() {
            return view().hidden().into_element();
        }
        let mut rows = view()
            .absolute()
            .top(px(36.))
            .left(px(0.))
            .right(px(0.))
            .z_index(10)
            .p(px(4.))
            .gap(px(1.))
            .rounded(px(RADIUS))
            .border_1()
            .border_color(keyline)
            .bg(sheet)
            .shadow_md();
        for (i, c) in list.into_iter().enumerate() {
            let on = move || active.get() == Some(i);
            let kind = c.kind();
            let name = c.label();
            rows = rows.child(
                nsh_base::button()
                    .accessibility_label(name.clone())
                    .selected(on)
                    .on_press(move || select(c.clone()))
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .min_h(px(30.))
                    .px(px(10.))
                    .rounded(px(RADIUS_SM))
                    .bg(move || if on() { wash() } else { clear() })
                    .hover(|s| s.bg(wash))
                    .focus(|s| s.bg(wash).outline(px(2.)).outline_color(accent))
                    .child(label(name).number_of_lines(1).flex_1())
                    .child(caption(kind)),
            );
        }
        rows.into_element()
    });
    view()
        .w_full()
        .min_w_0()
        .child(
            view()
                .w_full()
                .justify_center()
                .child(entry)
                .child(
                    view()
                        .absolute()
                        .left(px(11.))
                        .pointer_events(PointerEvents::None)
                        .child(icon(Icon::Search, secondary)),
                )
                .child(when(
                    move || !buffer.with(String::is_empty),
                    move || {
                        view().absolute().right(px(2.)).child(icon_button(
                            "Clear",
                            Icon::Close,
                            None,
                            reset,
                        ))
                    },
                )),
        )
        .child(suggestions)
        .into_element()
}

/// The active conditions, each a chip that removes it.
pub fn chips(chips: Signal<Vec<Chip>>) -> AnyElement {
    dynamic(move || {
        let list = chips.get();
        if list.is_empty() {
            return view().hidden().into_element();
        }
        let mut r = wrap().gap(px(6.));
        for (i, c) in list.iter().enumerate() {
            r = r.child(chip(c.0.clone(), move || {
                chips.update(|cs| {
                    cs.remove(i);
                })
            }));
        }
        if list.len() > 1 {
            r = r.child(ghost("Clear filters", move || chips.set(vec![])));
        }
        r.into_element()
    })
    .into_element()
}
