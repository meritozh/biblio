//! Tags and authors: a table that loads more as it scrolls, with a dialog
//! that creates or renames one.
use super::*;

/// A tag or an author, as its table shows it.
struct Name {
    id: i64,
    name: String,
    count: i64,
    color: Option<String>,
}

const NAME_CHUNK: i64 = 100;

pub(super) fn names(app: App, tags: bool) -> AnyElement {
    let (kind, kinds) = if tags {
        ("tag", "tags")
    } else {
        ("author", "authors")
    };
    let q = signal(String::new());
    let list = Paged::<Name>::new(app, NAME_CHUNK, 0, move || {
        app.revision.get();
        let q = q.get();
        let db = app.db();
        let key = q.clone();
        let fetch = paged::fetcher(move |offset, limit| {
            let (db, q) = (db.clone(), q.clone());
            async move {
                if tags {
                    let rows = tag::tag_list(
                        db.clone(),
                        Some(true),
                        Some(limit),
                        Some(offset),
                        Some(q.clone()),
                    )
                    .await?
                    .tags
                    .into_iter()
                    .map(|t| Name {
                        id: t.id,
                        name: t.name,
                        count: t.usage_count,
                        color: t.color,
                    })
                    .collect();
                    Ok((rows, tag::tag_count(db, Some(q)).await?))
                } else {
                    let rows = author::author_list(
                        db.clone(),
                        Some(true),
                        Some(limit),
                        Some(offset),
                        Some(q.clone()),
                    )
                    .await?
                    .authors
                    .into_iter()
                    .map(|a| Name {
                        id: a.id,
                        name: a.name,
                        count: a.usage_count,
                        color: None,
                    })
                    .collect();
                    Ok((rows, author::author_count(db, Some(q)).await?))
                }
            }
        });
        (key, fetch)
    });
    // The record a dialog edits; `None` while it creates one.
    let editing = signal(None::<i64>);
    let open = signal(false);
    let draft = signal(String::new());
    let color = signal(String::new());
    let edit = move |row: Option<&Name>| {
        editing.set(row.map(|r| r.id));
        draft.set(row.map(|r| r.name.clone()).unwrap_or_default());
        color.set(row.and_then(|r| r.color.clone()).unwrap_or_default());
        open.set(true);
    };
    let remove = move |row: &Name| {
        let (id, shown, count) = (row.id, row.name.clone(), row.count);
        ask(
            format!("Delete the {kind} “{shown}”?"),
            format!(
                "It is removed from {}. This cannot be undone.",
                counted(count, "file", "files")
            ),
            "Delete",
            true,
            move || {
                if tags {
                    app.save(tag::tag_delete(app.db(), id));
                } else {
                    app.save(author::author_delete(app.db(), id));
                }
            },
        )
    };
    let header = page_header(
        if tags { Icon::Tags } else { Icon::Authors },
        if tags { "Tags" } else { "Authors" },
        Some(count_caption(move || list.caption(kind, kinds)).into_element()),
        Some(
            row()
                .gap_2()
                .child(
                    search_field(
                        if tags {
                            "Search tags"
                        } else {
                            "Search authors"
                        },
                        q,
                    )
                    .w(move || px(if compact() { 180. } else { 260. })),
                )
                .child(create(
                    if tags { "New tag" } else { "New author" },
                    move || edit(None),
                ))
                .into_element(),
        ),
    );
    let mut cols = vec![Col::custom("name", "Name", None)];
    if tags {
        cols.push(Col::mono("color", "Colour", Some(120.)));
    }
    cols.push(Col::muted("files", "Files", Some(96.)).end());
    cols.push(Col::command("edit", Icon::Edit));
    cols.push(Col::command("delete", Icon::Trash));
    let keys: Vec<&'static str> = cols.iter().map(|c| c.key).collect();
    let rows = move || {
        list.rows.with(|rows| {
            nsh_base::DataTableRows::new(rows.len())
                .texts("name", rows.iter().map(|r| r.name.as_str()))
                .texts(
                    "color",
                    rows.iter().map(|r| r.color.clone().unwrap_or_default()),
                )
                .texts("files", rows.iter().map(|r| r.count.to_string()))
                .texts("edit", rows.iter().map(|r| format!("Edit {}", r.name)))
                .texts("delete", rows.iter().map(|r| format!("Delete {}", r.name)))
        })
    };
    let name_cell = move |c: &nsh_base::DataTableCell| {
        let swatch = list.at(c.row as usize).and_then(|r| r.color.clone());
        let mut line = row().gap_2().min_w_0();
        if let Some(swatch) = swatch.and_then(|s| parse_colour(&s)) {
            line = line.child(
                view()
                    .w(px(10.))
                    .h(px(10.))
                    .rounded(px(5.))
                    .flex_shrink_0()
                    .bg(move || rgb(swatch)),
            );
        }
        line.child(label(c.text.to_string()).number_of_lines(1).flex_shrink(1.))
            .into_element()
    };
    let shown = dynamic(move || {
        // Only the state's kind: the table itself follows the rows.
        let state = (list.total.get().is_none(), list.total.get() == Some(0));
        match state {
            (true, _) => caption("Loading…").into_element(),
            (_, true) => empty(
                if q.with(String::is_empty) {
                    if tags {
                        "No tags yet"
                    } else {
                        "No authors yet"
                    }
                } else {
                    "No matching names"
                },
                if q.with(String::is_empty) {
                    "Create one, or let analysis add them while importing."
                } else {
                    "Try another search."
                },
            )
            .into_element(),
            _ => untracked(|| {
                let keys = keys.clone();
                table(
                    if tags { "Tags" } else { "Authors" },
                    cols.clone(),
                    move || list.len(),
                    rows,
                    name_cell,
                    move |t| {
                        let keys = std::rc::Rc::new(keys);
                        let double = keys.clone();
                        t.has_more(move || list.has_more())
                            .loading(move || list.loading.get())
                            .on_load_more(move || list.more())
                            .on_row_press(move |p: &nsh_base::DataTablePress| {
                                let Some(row) = list.at(p.row as usize) else {
                                    return;
                                };
                                match pressed_key(&keys, p) {
                                    Some("edit") => edit(Some(&row)),
                                    Some("delete") => remove(&row),
                                    _ => {}
                                }
                            })
                            .on_double_press(move |t: &nsh_base::DataTableTarget| {
                                let _ = &double;
                                if let Some(row) = t.row.and_then(|r| list.at(r as usize)) {
                                    edit(Some(&row));
                                }
                            })
                    },
                )
                .into_element()
            }),
        }
    });
    let dialog = modal(
        open,
        move || {
            match (editing.get_untracked(), tags) {
                (Some(_), true) => "Edit tag",
                (Some(_), false) => "Edit author",
                (None, true) => "New tag",
                (None, false) => "New author",
            }
            .to_owned()
        },
        440.,
        move || {
            let id = editing.get_untracked();
            let mut form = view().gap_4().child(field("Name", draft));
            if tags {
                form = form.child(field_help(
                    "Colour",
                    "A hex colour such as #6b7fd7, or empty.",
                    color,
                ));
            }
            let submit = primary(if id.is_some() { "Save" } else { "Create" }, move || {
                let name = draft.get_untracked();
                let hue = Some(color.get_untracked()).filter(|s| !s.trim().is_empty());
                let db = app.db();
                app.save_then(
                    async move {
                        match (id, tags) {
                            (Some(id), true) => {
                                tag::tag_update(db, id, Some(name), hue).await.map(|_| ())
                            }
                            (Some(id), false) => {
                                author::author_update(db, id, name).await.map(|_| ())
                            }
                            (None, true) => tag::tag_create(db, name, hue).await.map(|_| ()),
                            (None, false) => author::author_create(db, name).await.map(|_| ()),
                        }
                    },
                    move || open.set(false),
                );
            });
            form.child(dialog_actions(open, submit)).into_element()
        },
    );
    ui::page(header, page_body().child(shown).child(dialog)).into_element()
}

/// `#rgb` or `#rrggbb` as a colour.
fn parse_colour(s: &str) -> Option<u32> {
    let hex = s.trim().strip_prefix('#')?;
    let full = match hex.len() {
        3 => hex.chars().flat_map(|c| [c, c]).collect::<String>(),
        6 => hex.to_owned(),
        _ => return None,
    };
    u32::from_str_radix(&full, 16).ok()
}
