//! Prompts: one card per prompt, edited in a dialog.
use super::*;

/// A prompt as its form edits it: id (0 for a new one), name, text,
/// schema and step.
type PromptDraft = (i64, String, String, String, String);

pub(super) fn prompt_list(app: App) -> AnyElement {
    let data = app.resource(move || {
        app.revision.get();
        prompts::prompt_list(app.db(), None, None)
    });
    let editing = signal(None::<PromptDraft>);
    let open = signal(false);
    let header = page_header(
        Icon::Prompts,
        "Prompts",
        Some(
            count_caption(move || match data.get().as_deref() {
                Some(Ok(list)) if !list.is_empty() => {
                    counted(list.len() as i64, "prompt", "prompts")
                }
                _ => String::new(),
            })
            .into_element(),
        ),
        Some(create("New prompt", move || {
            editing.set(None);
            open.set(true);
        })),
    );
    let shown = dynamic(move || match data.get().as_deref() {
        None => caption("Loading…").into_element(),
        Some(Err(e)) => label(e.clone()).into_element(),
        Some(Ok(list)) if list.is_empty() => empty(
            "No prompts yet",
            "Analysis uses the built-in prompt for each step until you add one.",
        )
        .into_element(),
        Some(Ok(list)) => {
            let mut rows = view().gap_3();
            for p in list {
                let draft: PromptDraft = (
                    p.id,
                    p.name.clone(),
                    p.content.clone(),
                    p.schema_slug
                        .clone()
                        .unwrap_or_else(|| p.mime_group.clone()),
                    p.step.clone(),
                );
                let id = p.id;
                let name = p.name.clone();
                let scope = format!("{} · {}", title_case(&draft.3), draft.4.replace('_', " "));
                let preview = p.content.split_whitespace().collect::<Vec<_>>().join(" ");
                let mut badges = row().gap_2().min_w_0().child(
                    label(p.name.clone())
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .number_of_lines(1)
                        .flex_shrink(1.),
                );
                badges = badges.child(badge(scope));
                if p.is_default {
                    badges = badges.child(badge("Active"));
                }
                let draft = stored(draft);
                rows = rows.child(
                    row()
                        .items_start()
                        .gap_4()
                        .px_4()
                        .py_3()
                        .rounded(px(RADIUS))
                        .border_1()
                        .border_color(rule)
                        .child(
                            view()
                                .flex_1()
                                .min_w_0()
                                .gap(px(6.))
                                .child(badges)
                                .child(note(preview).text_size(px(13.)).number_of_lines(1)),
                        )
                        .child(
                            row()
                                .gap_1()
                                .child(icon_button(format!("Edit {name}"), Icon::Edit, None, move || {
                                    editing.set(Some(draft.with(|d| d.clone())));
                                    open.set(true);
                                }))
                                .child(icon_button(format!("Delete {name}"), Icon::Trash, None, {
                                    let name = name.clone();
                                    move || {
                                        ask(
                                            format!("Delete the prompt “{name}”?"),
                                            "Analysis falls back to the default prompt for its step.",
                                            "Delete",
                                            true,
                                            move || app.save(prompts::prompt_delete(app.db(), id)),
                                        )
                                    }
                                })),
                        ),
                );
            }
            rows.into_element()
        }
    });
    let dialog = modal(
        open,
        move || {
            if editing.with_untracked(Option::is_some) {
                "Edit prompt"
            } else {
                "New prompt"
            }
            .to_owned()
        },
        640.,
        move || prompt_form(app, editing.get_untracked(), open),
    );
    ui::page(
        header,
        page_scroll(
            960.,
            view()
                .gap_5()
                .child(notice(
                    "Prompts steer the language model during analysis. Each schema and pipeline step runs its active prompt at import time; the others wait until you set them as default.",
                ))
                .child(shown)
                .child(dialog),
        ),
    )
    .into_element()
}

fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn prompt_form(app: App, p: Option<PromptDraft>, open: Signal<bool>) -> AnyElement {
    let (id, n, c, s, k) = p.unwrap_or((
        0,
        String::new(),
        String::new(),
        "novel".into(),
        "filename".into(),
    ));
    let name = signal(n);
    let content = signal(c);
    let slug = signal(s);
    let step = signal(k);
    let close = move || open.set(false);
    let submit = primary(
        if id == 0 {
            "Create prompt"
        } else {
            "Save prompt"
        },
        move || {
            let payload = prompts::PromptCreate {
                name: name.get_untracked(),
                content: content.get_untracked(),
                schema_slug: slug.get_untracked(),
                step: step.get_untracked(),
            };
            if id == 0 {
                app.save_then(prompts::prompt_create(app.db(), payload), close);
            } else {
                app.save_then(prompts::prompt_update(app.db(), id, payload), close);
            }
        },
    );
    let mut foot = dialog_actions(open, submit);
    if id != 0 {
        foot = row()
            .justify_between()
            .gap_2()
            .child(ghost("Set as default", move || {
                app.save_then(prompts::prompt_set_default(app.db(), id), close)
            }))
            .child(foot);
    }
    view()
        .gap_4()
        .child(field("Prompt name", name))
        .child(
            wrap()
                .gap_3()
                .child(field("Schema ID", slug).flex_1().min_w(px(160.)))
                .child(field("Pipeline step", step).flex_1().min_w(px(160.))),
        )
        .child(
            view().gap_1().child(field_label("Prompt text")).child(
                input("Prompt text", content)
                    .multiline(true)
                    .min_h(px(200.)),
            ),
        )
        .child(foot)
        .into_element()
}
