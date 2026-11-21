use crate::artwork::{Icon, icon};
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
                committed.set(buffer.get_untracked());
                open.set(false);
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
        .pl(px(38.))
        .min_h(px(42.))
        .flex_1();
    stack()
        .gap_1()
        .child(
            row()
                .child(
                    view()
                        .flex_1()
                        .min_w_0()
                        .justify_center()
                        .child(entry)
                        .child(
                            view()
                                .absolute()
                                .left(px(12.))
                                .pointer_events(PointerEvents::None)
                                .child(icon(Icon::Search, secondary)),
                        ),
                )
                .child(when(
                    move || !buffer.with(String::is_empty) || !chips.with(Vec::is_empty),
                    move || {
                        button("Clear", move || {
                            field.set(None);
                            prefix.set(None);
                            active.set(None);
                            buffer.set(String::new());
                            committed.set(String::new());
                            chips.set(vec![]);
                            open.set(false);
                        })
                    },
                )),
        )
        .child(dynamic(move || {
            let mut r = wrap();
            if open.get() {
                for (i, c) in choices.get().into_iter().enumerate() {
                    let selected = active.get() == Some(i);
                    r = r.child(button(
                        format!("{}{}", if selected { "› " } else { "" }, c.label()),
                        move || select(c.clone()),
                    ));
                }
            }
            r
        }))
        .child(dynamic(move || {
            let mut r = wrap();
            for (i, c) in chips.get().iter().enumerate() {
                r = r.child(button(format!("{} ×", c.0), move || {
                    chips.update(|cs| {
                        cs.remove(i);
                    })
                }));
            }
            r
        }))
        .into_element()
}
