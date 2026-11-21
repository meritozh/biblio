use crate::{
    app::{App, Page},
    ui::*,
};
use biblio_core::commands::{
    self as api, author, cache, cover as artwork, file, metadata, remote, tag,
};
use nsh_core::prelude::*;

pub fn cover(app: App, id: i64, height: f32) -> AnyElement {
    app.load(cover_uri(app.db(), id), move |uri| match uri {
        Some(url) => image(url.clone()).h(px(height)).w_full().into_element(),
        None => view().into_element(),
    })
}
async fn cover_uri(db: biblio_core::Library, id: i64) -> Result<Option<String>, String> {
    match artwork::cover_get(db, id).await {
        Ok(c) => Ok(Some(format!("data:{};base64,{}", c.mime_type, c.data))),
        Err(e) if e == "COVER_NOT_FOUND" => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn card_face(app: App, file: &api::FileListItem) -> AnyElement {
    let id = file.id;
    let title = file.display_name.clone();
    let progress = file.progress.clone().unwrap_or_default();
    let mut tag_ids = file.tags.iter().map(|tag| tag.id).collect::<Vec<_>>();
    tag_ids.sort_unstable();
    let seed = tag_ids
        .into_iter()
        .reduce(|a, b| a.wrapping_mul(31).wrapping_add(b))
        .unwrap_or(id) as usize;
    let extension = std::path::Path::new(&file.path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_uppercase();
    let preview = app.load(cover_uri(app.db(), id), move |uri| {
        if let Some(uri) = uri {
            return stack()
                .gap_1()
                .child(image(uri.clone()).h(px(142.)).w_full())
                .child(label(&title))
                .into_element();
        }
        stack()
            .h(px(152.))
            .w_full()
            .justify_between()
            .p_3()
            .gap_1()
            .rounded_md()
            .bg(move || stock(seed))
            .child(
                row()
                    .pb_1()
                    .border_b_1()
                    .border_color(secondary)
                    .child(note(extension.clone()).text_xs().number_of_lines(1)),
            )
            .child(subheading(&title).number_of_lines(3))
            .child(
                row()
                    .pt_1()
                    .border_t_1()
                    .border_color(secondary)
                    .child(note(progress.clone()).text_xs().number_of_lines(1)),
            )
            .into_element()
    });
    nsh_base::button()
        .accessibility_label(file.display_name.clone())
        .on_press(move || app.page.set(Page::File(id)))
        .w_full()
        .rounded_md()
        .hover(|s| s.opacity(0.88))
        .active(|s| s.opacity(0.78))
        .focus(|s| s.outline(px(2.)).outline_color(accent))
        .child(preview)
        .into_element()
}
pub fn screen(app: App, id: i64) -> AnyElement {
    stack()
        .flex_1()
        .min_h_0()
        .child(button("Back to library", move || {
            app.page.set(Page::Library)
        }))
        .child(dynamic(move || {
            app.revision.get();
            app.load(file::create_replace::file_get(app.db(), id), move |f| {
                form(app, f)
            })
        }))
        .into_element()
}
fn form(app: App, file: &api::FileWithDetails) -> AnyElement {
    let id = file.id;
    let name = signal(file.display_name.clone());
    let progress = signal(file.progress.clone().unwrap_or_default());
    let path = file.path.clone();
    let local = file
        .local_cache_path
        .clone()
        .or_else(|| (file.storage_kind.as_deref() != Some("remote")).then(|| file.path.clone()));
    let cat = signal(file.category_id);
    let original = file.category_id;
    let favorite = file.is_favorite;
    let mut body = stack()
        .child(heading(&file.display_name))
        .child(note(&file.path))
        .child(cover(app, id, 180.))
        .child(
            wrap()
                .child(button("Open file", move || {
                    app.job(cache::cache_open(app.db(), id), |_| {})
                }))
                .child(button(
                    if favorite { "Unfavorite" } else { "Favorite" },
                    move || {
                        app.save(file::update_favorite::file_set_favorite(
                            app.db(),
                            id,
                            !favorite,
                        ))
                    },
                )),
        )
        .child(field("Title", name))
        .child(field("Progress", progress))
        .child(category_picker(app, cat))
        .child(primary("Save file", move || {
            app.save(file::update_favorite::file_update(
                app.db(),
                id,
                Some(name.get_untracked()),
                cat.get_untracked().filter(|v| Some(*v) != original),
                Some(progress.get_untracked()),
            ))
        }));
    let mut paths = wrap().child(button("Copy path", move || set_clipboard(path.clone())));
    if let Some(path) = local {
        paths = paths.child(button("Show in folder", move || {
            #[cfg(target_os = "macos")]
            let result = std::process::Command::new("open")
                .arg("-R")
                .arg(&path)
                .spawn()
                .map(|_| ());
            #[cfg(not(target_os = "macos"))]
            let result = open::that_detached(
                std::path::Path::new(&path)
                    .parent()
                    .unwrap_or(std::path::Path::new(".")),
            );
            if let Err(e) = result {
                app.message.set(e.to_string());
            }
        }));
    }
    body = body.child(paths);
    for (kind, is_tag) in [("Tags", true), ("Authors", false)] {
        let mut attached = wrap();
        if is_tag {
            for t in &file.tags {
                let tid = t.id;
                attached = attached.child(button(format!("{} ×", t.name), move || {
                    app.save(tag::tag_unassign(app.db(), id, vec![tid]))
                }));
            }
        } else {
            for a in &file.authors {
                let aid = a.id;
                attached = attached.child(button(format!("{} ×", a.name), move || {
                    app.save(author::author_unassign(app.db(), id, vec![aid]))
                }));
            }
        }
        let q = signal(String::new());
        body = body
            .child(section(kind))
            .child(attached)
            .child(input(&format!("Find {kind}"), q))
            .child(dynamic(move || {
                let name = q.get();
                if name.is_empty() {
                    return view().into_element();
                }
                let db = app.db();
                app.load(
                    async move { find_names(db, is_tag, name).await },
                    move |values| {
                        let mut r = wrap();
                        for (tid, name) in values {
                            let tid = *tid;
                            r = r.child(button(name, move || {
                                if is_tag {
                                    app.save(tag::tag_assign(app.db(), id, vec![tid]));
                                } else {
                                    app.save(author::author_assign(app.db(), id, vec![tid]));
                                }
                            }));
                        }
                        r.into_element()
                    },
                )
            }));
    }
    let mut keys = std::collections::HashSet::new();
    for m in &file.metadata {
        keys.insert(m.key.clone());
        body = body.child(metadata_field(
            app,
            id,
            m.key.clone(),
            m.value.clone(),
            m.data_type.clone(),
        ));
    }
    if let Some(category) = &file.category {
        let slug = category.schema_slug.clone();
        body = body.child(
            app.load(api::schemas::schema_list(app.db()), move |schemas| {
                let mut fields = stack();
                if let Some(s) = schemas.iter().find(|s| s.schema.id == slug) {
                    for f in &s.fields {
                        if f.semantic.is_none() && f.form_visible && !keys.contains(&f.field_key) {
                            fields = fields.child(metadata_field(
                                app,
                                id,
                                f.field_key.clone(),
                                String::new(),
                                f.field_type.clone(),
                            ));
                        }
                    }
                }
                fields.into_element()
            }),
        );
    }
    let key = signal(String::new());
    let value = signal(String::new());
    body = body
        .child(
            row()
                .child(input("Metadata key", key))
                .child(input("Value", value))
                .child(button("Add metadata", move || {
                    app.save(metadata::metadata_set(
                        app.db(),
                        id,
                        key.get_untracked(),
                        value.get_untracked(),
                        None,
                    ))
                })),
        )
        .child(
            wrap()
                .child(button("Choose cover", move || {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Images", &["jpg", "jpeg", "png", "webp"])
                        .pick_file()
                    {
                        let db = app.db();
                        app.save(async move {
                            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
                            artwork::cover_set(db, id, bytes, None).await
                        });
                    }
                }))
                .child(confirm("Remove cover", move || {
                    app.save(artwork::cover_delete(app.db(), id))
                })),
        );
    let mut actions = wrap();
    if file.storage_kind.as_deref() == Some("remote") {
        actions = actions.child(button("Download", move || {
            app.job(
                remote::file_download_from_remote(app.db(), vec![id]),
                |_| {},
            )
        }));
        if file.local_cache_path.is_some() {
            actions = actions.child(confirm("Clear cached file", move || {
                app.save(cache::cache_clear(app.db(), id))
            }));
        }
    } else {
        actions = actions.child(confirm("Move to Baidu Netdisk", move || {
            app.job(remote::file_upload_to_remote(app.db(), vec![id]), |_| {})
        }));
    }
    scroll()
        .flex_1()
        .min_h_0()
        .child(
            body.child(actions)
                .child(confirm("Delete file and library record", move || {
                    app.job(file::delete_move::file_delete(app.db(), id), move |_| {
                        app.page.set(Page::Library);
                        app.refresh();
                    })
                })),
        )
        .into_element()
}

fn metadata_field(app: App, id: i64, key: String, initial: String, ty: String) -> AnyElement {
    let value = signal(initial);
    let name = key.clone();
    stack()
        .child(field(&key, value))
        .child(
            wrap()
                .child(button("Save value", move || {
                    app.save(metadata::metadata_set(
                        app.db(),
                        id,
                        name.clone(),
                        value.get_untracked(),
                        Some(ty.clone()),
                    ))
                }))
                .child(confirm("Remove value", move || {
                    app.save(metadata::metadata_delete(app.db(), id, key.clone()))
                })),
        )
        .into_element()
}
pub async fn find_names(
    db: biblio_core::Library,
    tags: bool,
    query: String,
) -> Result<Vec<(i64, String)>, String> {
    if tags {
        Ok(tag::tag_list(db, None, Some(24), None, Some(query))
            .await?
            .tags
            .into_iter()
            .map(|t| (t.id, t.name))
            .collect())
    } else {
        Ok(author::author_list(db, None, Some(24), None, Some(query))
            .await?
            .authors
            .into_iter()
            .map(|a| (a.id, a.name))
            .collect())
    }
}
pub fn category_picker(app: App, selected: Signal<Option<i64>>) -> AnyElement {
    app.load(api::category::category_list(app.db()), move |categories| {
        let values = categories
            .iter()
            .map(|c| (c.id, c.name.clone()))
            .collect::<Vec<_>>();
        dynamic(move || {
            let active = selected.get();
            let mut r = wrap().child(label("Category"));
            for (id, n) in &values {
                let id = *id;
                r = r.child(button(
                    if active == Some(id) {
                        format!("✓ {n}")
                    } else {
                        n.clone()
                    },
                    move || selected.set(Some(id)),
                ));
            }
            r
        })
        .into_element()
    })
}
