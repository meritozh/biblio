use crate::{
    app::{App, Page},
    detail,
    ui::*,
};
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
use std::collections::{BTreeMap, HashSet};

pub fn drop_urls(app: App, urls: &[String]) {
    let paths = urls
        .iter()
        .filter_map(|s| {
            url::Url::parse(s)
                .ok()?
                .to_file_path()
                .ok()?
                .into_os_string()
                .into_string()
                .ok()
        })
        .collect();
    add_sources(app, paths);
    app.page.set(Page::Import);
}
fn add_sources(app: App, paths: Vec<String>) {
    app.sources.update(|old| {
        for p in paths {
            if !old.contains(&p) {
                old.push(p);
            }
        }
    });
}
pub fn screen(app: App) -> AnyElement {
    let first = computed(move || app.pending.with(|v| v.first().map(|p| p.path.clone())));
    let dragging = signal(false);
    let mut pickers = wrap().justify_center();
    for (name, folder) in [("Choose files", false), ("Choose folders", true)] {
        pickers = pickers.child(button(name, move || {
            let dialog = rfd::FileDialog::new();
            let paths = if folder {
                dialog.pick_folders()
            } else {
                dialog.pick_files()
            };
            if let Some(paths) = paths {
                add_sources(
                    app,
                    paths
                        .into_iter()
                        .map(|p| p.to_string_lossy().into_owned())
                        .collect(),
                );
            }
        }));
    }
    let drop_area = stack()
        .items_center()
        .justify_center()
        .py_6()
        .px_4()
        .min_h(px(176.))
        .border_1()
        .rounded_lg()
        .border_color(move || if dragging.get() { accent() } else { rule() })
        .bg(move || if dragging.get() { stock(0) } else { sheet() })
        .child(crate::artwork::icon(crate::artwork::Icon::Import, accent))
        .child(subheading("Drop files or folders here").text_center())
        .child(pickers);
    let controls = wrap()
        .child(primary("Analyze selected files", move || analyze(app)))
        .child(confirm("Discard import batch", move || {
            let db = app.db();
            app.job(
                async move {
                    processing::cancel_processing(db.clone()).await;
                    processing::prepared_cover_clear(db).await;
                    Ok(())
                },
                move |_| {
                    app.sources.set(vec![]);
                    app.pending.set(vec![]);
                    app.analyzing.set(vec![]);
                    app.roots.with(|r| r.clear());
                },
            );
        }));
    scroll()
        .flex_1()
        .min_h_0()
        .on_file_drag_enter(move |_: &()| dragging.set(true))
        .on_file_drag_leave(move |_: &()| dragging.set(false))
        .on_file_drop(move |e: &FileDropEvent| {
            dragging.set(false);
            drop_urls(app, &e.uris);
        })
        .child(
            stack()
                .child(heading("Import"))
                .child(detail::category_picker(app, app.category))
                .child(drop_area)
                .child(
                    text(move || {
                        format!(
                            "{} selected · {} analyzing · {} ready for review",
                            app.sources.with(Vec::len),
                            app.analyzing.with(Vec::len),
                            app.pending.with(Vec::len)
                        )
                    })
                    .text_color(secondary),
                )
                .child(controls)
                .child(dynamic(move || {
                    if first.get().is_some() {
                        let item = app.pending.get_untracked().remove(0);
                        untracked(|| review(app, item))
                    } else {
                        view().into_element()
                    }
                })),
        )
        .into_element()
}
fn analyze(app: App) {
    let Some(cat) = app.category.get_untracked() else {
        app.message.set("Choose an import category".into());
        return;
    };
    let sources = app.sources.get_untracked();
    if sources.is_empty() {
        app.message.set("Choose files or folders first".into());
        return;
    }
    let db = app.db();
    app.job(
        async move {
            if !api::settings::storage_check_access(db.clone()).await? {
                return Err("Choose a writable storage folder in Settings".into());
            }
            let category = api::category::category_get(db.clone(), cat).await?;
            let schemas = api::schemas::schema_list(db).await?;
            let schema = schemas
                .iter()
                .find(|s| s.schema.id == category.schema_slug)
                .ok_or("Unknown category schema")?;
            let allowed: Vec<String> = serde_json::from_str(&schema.schema.accepted_extensions)
                .map_err(|e| e.to_string())?;
            let mut picked = folder_import::expand_drop_paths(
                sources,
                Some(schema.schema.pipeline_template.clone()),
            )
            .await?;
            picked.files.retain(|p| {
                let path = std::path::Path::new(p);
                path.is_dir()
                    || allowed.is_empty()
                    || path
                        .extension()
                        .and_then(|s| s.to_str())
                        .is_some_and(|s| allowed.iter().any(|ext| ext.eq_ignore_ascii_case(s)))
            });
            Ok(picked)
        },
        move |picked| {
            let mut seen: HashSet<_> = app.analyzing.get_untracked().into_iter().collect();
            seen.extend(
                app.pending
                    .with(|v| v.iter().map(|p| p.path.clone()).collect::<Vec<_>>()),
            );
            let paths: Vec<_> = picked
                .files
                .into_iter()
                .filter(|p| seen.insert(p.clone()))
                .collect();
            if paths.is_empty() {
                app.message.set("No new files match this category".into());
                return;
            }
            app.roots
                .with(|r| r.extend(picked.path_folder_roots.clone()));
            app.analyzing.update(|v| v.extend(paths.clone()));
            app.sources.set(vec![]);
            let db = app.db();
            let queued = paths.clone();
            app.job(
                async move {
                    Ok(processing::enqueue_import(
                        db,
                        paths,
                        Some(picked.path_folder_roots),
                        Some(cat),
                    )
                    .await)
                },
                move |result| {
                    if let Err(error) = result {
                        app.analyzing.update(|v| v.retain(|p| !queued.contains(p)));
                        add_sources(app, queued);
                        app.message.set(error);
                    }
                },
            );
        },
    );
}

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
fn review(app: App, item: FilePreparedImport) -> AnyElement {
    let name = signal(item.display_name.clone());
    let progress = signal(item.progress.clone().unwrap_or_default());
    let tags = signal(String::new());
    let authors = signal(item.unresolved_author_names.join("\n"));
    let category = signal(item.category_id.or(app.category.get_untracked()));
    let cover = signal(CoverChoice::Staged);
    let suggestions = signal(String::new());
    let path = item.path.clone();
    let mut body = stack()
        .child(section("Review import"))
        .child(label(&path))
        .child(field("Title", name))
        .child(detail::category_picker(app, category))
        .child(field("Progress", progress))
        .child(label("Additional authors, one per line"))
        .child(input("Additional authors", authors).multiline(true))
        .child(label("Additional tags, one per line"))
        .child(input("Additional tags", tags).multiline(true));
    for tag in &item.suggested_tags {
        let v = tag.clone();
        body = body.child(button(format!("Add tag · {tag}"), move || {
            tags.update(|t| {
                t.push_str(&v);
                t.push('\n');
            })
        }));
    }
    let staged = item.cover_mime_type.is_some();
    let preview_path = path.clone();
    body = body
        .child(dynamic(move || match cover.get() {
            CoverChoice::Staged if staged => app.load(
                processing::prepared_cover_get(app.db(), preview_path.clone()),
                |c| {
                    image(format!("data:{};base64,{}", c.mime_type, c.data))
                        .h(px(140.))
                        .into_element()
                },
            ),
            CoverChoice::File(path) => {
                label(format!("Selected cover · {}", path.display())).into_element()
            }
            CoverChoice::Url(_) => label("Selected VNDB cover").into_element(),
            _ => view().into_element(),
        }))
        .child(
            wrap()
                .child(button("Choose import cover", move || {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Images", &["jpg", "jpeg", "png", "webp"])
                        .pick_file()
                    {
                        cover.set(CoverChoice::File(path));
                    }
                }))
                .child(button("Clear import cover", move || {
                    cover.set(CoverChoice::Removed)
                })),
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
    let mut actions = wrap().child(primary("Import this file", move || commit(None)));
    if let Some(id) = dupe {
        actions = actions.child(confirm("Replace existing file", move || commit(Some(id))));
        let source = path.clone();
        actions = actions.child(confirm("Move incoming source to Trash", move || {
            let db = app.db();
            let p = source.clone();
            app.job(
                file::delete_move::file_delete_source(p.clone()),
                move |_| {
                    processing::discard_prepared_cover(&db, &p);
                    remove(app, &p);
                },
            );
        }));
    }
    let keep = path.clone();
    body.child(actions.child(button("Skip", move || {
        let db = app.db();
        processing::discard_prepared_cover(&db, &path);
        remove(app, &path);
    })))
    .child(when(
        move || app.pending.with(Vec::len) > 1,
        move || {
            let keep = keep.clone();
            confirm("Import other non-duplicate files as analyzed", move || {
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
            })
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
