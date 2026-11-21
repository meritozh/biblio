use crate::{app::App, detail, ui::*};
use base64::Engine;
use biblio_core::{
    Library,
    commands::{
        self as api,
        file::{self, folder_import},
        processing::{self, FilePreparedImport},
    },
};
use nsh_core::prelude::*;
use std::collections::BTreeMap;

async fn named_ids(
    db: &Library,
    tags: bool,
    names: String,
    mut ids: Vec<i64>,
) -> Result<Vec<i64>, String> {
    for name in names.lines().map(str::trim).filter(|v| !v.is_empty()) {
        let matches = detail::find_names(db.clone(), tags, name.into()).await?;
        let id = if let Some((id, _)) = matches.iter().find(|(_, n)| n.eq_ignore_ascii_case(name)) {
            *id
        } else if tags {
            api::tag::tag_create(db.clone(), name.into(), None)
                .await?
                .id
        } else {
            api::author::author_create(db.clone(), name.into())
                .await?
                .id
        };
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    Ok(ids)
}
pub(super) fn screen(app: App, item: FilePreparedImport) -> AnyElement {
    let name = signal(item.display_name.clone());
    let progress = signal(item.progress.clone().unwrap_or_default());
    let tags = signal(String::new());
    let authors = signal(item.unresolved_author_names.join("\n"));
    let category = signal(item.category_id.or(app.category.get_untracked()));
    let cover = signal(CoverChoice::Staged);
    let suggestions = signal(String::new());
    let path = item.path.clone();
    let waiting = app.pending.with_untracked(Vec::len);
    let mut body = view()
        .gap_3()
        .flex_1()
        .min_w(px(280.))
        .child(caption(&path).selectable(true).number_of_lines(2))
        .child(field("Title", name))
        .child(
            view()
                .gap_1()
                .child(field_label("Category"))
                .child(detail::category_picker(app, category)),
        )
        .child(field("Progress", progress))
        .child(text_area("Additional authors", authors))
        .child(caption("One author per line."))
        .child(text_area("Additional tags", tags))
        .child(caption("One tag per line."));
    if !item.suggested_tags.is_empty() {
        let mut suggested = wrap().gap(px(6.));
        for tag in &item.suggested_tags {
            let v = tag.clone();
            suggested = suggested.child(control(
                format!("Add tag · {tag}"),
                Variant::Default,
                ButtonSize::Sm,
                Some(Icon::Plus),
                move || {
                    tags.update(|t| {
                        t.push_str(&v);
                        t.push('\n');
                    })
                },
            ));
        }
        body = body.child(
            view()
                .gap_1()
                .child(field_label("Suggested tags"))
                .child(suggested),
        );
    }
    let staged = item.cover_mime_type.is_some();
    let preview_path = path.clone();
    let art = view()
        .gap_2()
        .w(px(160.))
        .flex_shrink_0()
        .child(dynamic(move || match cover.get() {
            CoverChoice::Staged if staged => app.load(
                processing::prepared_cover_get(app.db(), preview_path.clone()),
                |c| {
                    image(format!("data:{};base64,{}", c.mime_type, c.data))
                        .w(px(160.))
                        .aspect_ratio(0.75)
                        .rounded(px(RADIUS_SM))
                        .border_1()
                        .border_color(rule)
                        .into_element()
                },
            ),
            CoverChoice::File(path) => caption(format!(
                "Selected cover · {}",
                path.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            ))
            .into_element(),
            CoverChoice::Url(_) => caption("Selected VNDB cover").into_element(),
            _ => view()
                .w(px(160.))
                .aspect_ratio(0.75)
                .items_center()
                .justify_center()
                .rounded(px(RADIUS_SM))
                .border_1()
                .border_dashed()
                .border_color(keyline)
                .child(caption("No cover"))
                .into_element(),
        }))
        .child(
            control(
                "Choose import cover",
                Variant::Default,
                ButtonSize::Sm,
                Some(Icon::Cover),
                move || {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Images", &["jpg", "jpeg", "png", "webp"])
                        .pick_file()
                    {
                        cover.set(CoverChoice::File(path));
                    }
                },
            )
            .w_full(),
        )
        .child(
            ghost("Clear import cover", move || {
                cover.set(CoverChoice::Removed)
            })
            .w_full(),
        );
    let metadata = stored(
        item.metadata
            .iter()
            .map(|m| (m.key.clone(), (m.value.clone(), m.data_type.clone())))
            .collect::<BTreeMap<_, _>>(),
    );
    body = body.child(dynamic(move || {
        let Some(id) = category.get() else {
            return view().into_element();
        };
        let db = app.db();
        app.load(
            async move {
                let cat = api::category::category_get(db.clone(), id).await?;
                let defs = api::schemas::schema_list(db).await?;
                Ok(defs.into_iter().find(|d| d.schema.id == cat.schema_slug))
            },
            move |schema| {
                let mut rows = stack();
                let mut fields = BTreeMap::new();
                if let Some(schema) = schema {
                    for f in &schema.fields {
                        if f.semantic.is_none() && (f.form_visible || f.required) {
                            fields.insert(
                                f.field_key.clone(),
                                (f.label.clone(), f.field_type.clone()),
                            );
                        }
                    }
                }
                metadata.with(|values| {
                    for (k, (_, ty)) in values {
                        fields
                            .entry(k.clone())
                            .or_insert_with(|| (k.clone(), ty.clone()));
                    }
                });
                for (key, (label, ty)) in fields {
                    let value = signal(
                        metadata.with(|m| m.get(&key).map(|(v, _)| v.clone()).unwrap_or_default()),
                    );
                    effect(move || {
                        let v = value.get();
                        metadata.with(|m| {
                            m.insert(key.clone(), (v, ty.clone()));
                        });
                    });
                    rows = rows.child(field(&label, value));
                }
                rows.into_element()
            },
        )
    }));
    let cat_for_lookup = category.get_untracked();
    if let Some(id) = cat_for_lookup {
        let db = app.db();
        body = body.child(app.load(
            async move {
                let cat = api::category::category_get(db.clone(), id).await?;
                let defs = api::schemas::schema_list(db).await?;
                Ok(defs.iter().any(|s| {
                    s.schema.id == cat.schema_slug
                        && s.pipeline_steps
                            .iter()
                            .any(|p| p.step_key == "vndb" && p.enabled)
                }))
            },
            move |enabled| {
                if !enabled {
                    return view().into_element();
                }
                stack()
                    .child(button("Search VNDB", move || {
                        suggestions.set(name.get_untracked())
                    }))
                    .child(dynamic(move || {
                        let q = suggestions.get();
                        if q.is_empty() {
                            return view().into_element();
                        }
                        app.load(api::vndb::vndb_search(q), move |rows| {
                            let mut r = stack();
                            for row in rows {
                                let candidate = row.clone();
                                r = r.child(button(&row.title, move || {
                                    name.set(
                                        candidate
                                            .alttitle
                                            .clone()
                                            .unwrap_or_else(|| candidate.title.clone()),
                                    );
                                    authors.set(candidate.developers.join("\n"));
                                    if let Some(url) = &candidate.image_url {
                                        cover.set(CoverChoice::Url(url.clone()));
                                    }
                                    suggestions.set(String::new());
                                }));
                            }
                            r.into_element()
                        })
                    }))
                    .into_element()
            },
        ));
    }
    let dupe = item.duplicate_of.as_ref().map(|d| d.existing_file_id);
    if let Some(d) = &item.duplicate_of {
        body = body.child(label(format!(
            "Possible duplicate · {}",
            d.existing_display_name
        )));
    }
    let item = stored(item);
    let commit = move |replace: Option<i64>| {
        let mut data = item.with(|v| v.clone());
        data.display_name = name.get_untracked();
        data.progress = Some(progress.get_untracked());
        data.category_id = category.get_untracked();
        let path = data.path.clone();
        let root = app.roots.with(|r| r.get(&path).cloned());
        let db = app.db();
        data.metadata = metadata.with(|v| {
            v.iter()
                .filter(|(_, (val, _))| !val.trim().is_empty())
                .map(|(key, (value, ty))| processing::ExtractedField {
                    key: key.clone(),
                    value: value.clone(),
                    data_type: ty.clone(),
                })
                .collect()
        });
        let tags = tags.get_untracked();
        let authors = authors.get_untracked();
        let cover = cover.get_untracked();
        app.job(
            commit_review(db, data, replace, tags, authors, cover, root),
            move |warning| {
                remove(app, &path);
                app.refresh();
                app.message
                    .set(warning.unwrap_or_else(|| "Imported".into()));
            },
        );
    };
    let mut actions = wrap()
        .gap_2()
        .child(primary("Import this file", move || commit(None)));
    if let Some(id) = dupe {
        actions = actions.child(confirm(
            "Replace existing file",
            "Replace the existing file?",
            "The library record keeps its place; its file is replaced by this one.",
            true,
            move || commit(Some(id)),
        ));
        let source = path.clone();
        actions = actions.child(confirm(
            "Move incoming source to Trash",
            "Move the incoming file to the Trash?",
            "It is not imported; the copy already in the library stays.",
            true,
            move || {
                let db = app.db();
                let p = source.clone();
                app.job(
                    file::delete_move::file_delete_source(p.clone()),
                    move |_| {
                        processing::discard_prepared_cover(&db, &p);
                        remove(app, &p);
                    },
                );
            },
        ));
    }
    let keep = path.clone();
    let body = body.child(actions.child(button("Skip", move || {
        let db = app.db();
        processing::discard_prepared_cover(&db, &path);
        remove(app, &path);
    })));
    section_with(
        "Review import",
        if waiting == 1 {
            "1 file is ready for review.".to_owned()
        } else {
            format!("{waiting} files are ready for review.")
        },
    )
    .child(
        wrap()
            .items_start()
            .gap_x(px(32.))
            .gap_y(px(16.))
            .child(art)
            .child(body),
    )
    .child(when(
        move || app.pending.with(Vec::len) > 1,
        move || {
            let keep = keep.clone();
            confirm(
                "Import other non-duplicate files as analyzed",
                "Import the other analyzed files as they are?",
                "Files that may be duplicates stay for review.",
                false,
                move || {
                    let mut other = app.pending.get_untracked();
                    let mut counts = std::collections::HashMap::new();
                    for p in &other {
                        *counts
                            .entry((p.category_id, p.display_name.trim().to_lowercase()))
                            .or_insert(0) += 1;
                    }
                    other.retain(|p| {
                        p.path != keep
                            && p.duplicate_of.is_none()
                            && counts[&(p.category_id, p.display_name.trim().to_lowercase())] == 1
                    });
                    let other = app.roots.with(|r| {
                        other
                            .into_iter()
                            .map(|p| {
                                let root = r.get(&p.path).cloned();
                                (p, root)
                            })
                            .collect::<Vec<_>>()
                    });
                    let db = app.db();
                    app.job(
                        async move {
                            let mut complete = Vec::new();
                            let mut issues = Vec::new();
                            for (p, root) in other {
                                let path = p.path.clone();
                                let authors = p.unresolved_author_names.join("\n");
                                match commit_review(
                                    db.clone(),
                                    p,
                                    None,
                                    String::new(),
                                    authors,
                                    CoverChoice::Staged,
                                    root,
                                )
                                .await
                                {
                                    Ok(note) => {
                                        complete.push(path);
                                        issues.extend(note);
                                    }
                                    Err(e) => issues.push(format!("{path} · {e}")),
                                }
                            }
                            Ok((complete, issues))
                        },
                        move |(complete, issues)| {
                            let count = complete.len();
                            for p in complete {
                                remove(app, &p);
                            }
                            app.refresh();
                            app.message
                                .set(format!("{count} imported {}", issues.join("\n")));
                        },
                    );
                },
            )
        },
    ))
    .into_element()
}
#[derive(Clone, PartialEq)]
enum CoverChoice {
    Staged,
    Removed,
    File(std::path::PathBuf),
    Url(String),
}

async fn commit_review(
    db: Library,
    mut data: FilePreparedImport,
    replace: Option<i64>,
    tags: String,
    authors: String,
    cover: CoverChoice,
    root: Option<String>,
) -> Result<Option<String>, String> {
    data.tag_ids = named_ids(&db, true, tags, data.tag_ids).await?;
    data.author_ids = named_ids(&db, false, authors, data.author_ids).await?;
    let mut meta: Vec<_> = data
        .metadata
        .into_iter()
        .map(|m| api::MetadataInput {
            key: m.key,
            value: m.value,
            data_type: Some(m.data_type),
        })
        .collect();
    let inherit_cover = cover == CoverChoice::Staged && data.cover_mime_type.is_none();
    let staged =
        (cover == CoverChoice::Staged && data.cover_mime_type.is_some()).then(|| data.path.clone());
    let (mut bytes, mut mime) = match cover {
        CoverChoice::File(p) => (Some(std::fs::read(p).map_err(|e| e.to_string())?), None),
        CoverChoice::Url(url) => {
            let c = api::vndb::vndb_fetch_cover(url).await?;
            (
                Some(
                    base64::engine::general_purpose::STANDARD
                        .decode(c.data)
                        .map_err(|e| e.to_string())?,
                ),
                Some(c.mime_type),
            )
        }
        _ => (None, data.cover_mime_type),
    };
    let mut favorite = None;
    if let Some(id) = replace {
        let old = file::create_replace::file_get(db.clone(), id).await?;
        favorite = Some(old.is_favorite);
        if data.display_name.trim().is_empty() {
            data.display_name = old.display_name;
        }
        if data.progress.as_deref().unwrap_or("").trim().is_empty() {
            data.progress = old.progress;
        }
        data.category_id = data.category_id.or(old.category_id);
        if data.author_ids.is_empty() {
            data.author_ids = old.authors.into_iter().map(|a| a.id).collect();
        }
        if data.tag_ids.is_empty() {
            data.tag_ids = old.tags.into_iter().map(|t| t.id).collect();
        }
        if meta.is_empty() {
            meta = old
                .metadata
                .into_iter()
                .map(|m| api::MetadataInput {
                    key: m.key,
                    value: m.value,
                    data_type: Some(m.data_type),
                })
                .collect();
        }
        if inherit_cover {
            match api::cover::cover_get(db.clone(), id).await {
                Ok(c) => {
                    bytes = Some(
                        base64::engine::general_purpose::STANDARD
                            .decode(c.data)
                            .map_err(|e| e.to_string())?,
                    );
                    mime = Some(c.mime_type);
                }
                Err(e) if e == "COVER_NOT_FOUND" => {}
                Err(e) => return Err(e),
            }
        }
    }
    if let Some(id) = data.category_id {
        let cat = api::category::category_get(db.clone(), id).await?;
        if let Some(schema) = api::schemas::schema_list(db.clone())
            .await?
            .into_iter()
            .find(|s| s.schema.id == cat.schema_slug)
        {
            for f in schema.fields {
                if f.required
                    && f.semantic.is_none()
                    && !meta
                        .iter()
                        .any(|m| m.key == f.field_key && !m.value.trim().is_empty())
                {
                    return Err(format!("{} is required", f.label));
                }
            }
        }
    }
    let path = data.path;
    let result = match replace {
        Some(id) => {
            file::search_lucky_status::file_replace(
                db.clone(),
                id,
                path.clone(),
                data.display_name,
                data.category_id,
                Some(data.tag_ids),
                Some(data.author_ids),
                Some(meta),
                data.progress,
                favorite,
                bytes,
                mime,
                staged,
            )
            .await?
        }
        None => {
            file::create_replace::file_create(
                db.clone(),
                path.clone(),
                data.display_name,
                data.category_id,
                Some(data.tag_ids),
                Some(data.author_ids),
                Some(meta),
                data.progress,
                favorite,
                bytes,
                mime,
                staged,
            )
            .await?
        }
    };
    processing::discard_prepared_cover(&db, &path);
    let cleanup = if let Some(root) = root {
        folder_import::import_finalize(db, root, data.source_is_directory)
            .await
            .err()
    } else {
        None
    };
    Ok(result.warning.or(cleanup))
}
fn remove(app: App, path: &str) {
    app.roots.with(|r| r.remove(path));
    app.pending.update(|v| v.retain(|p| p.path != path));
}
