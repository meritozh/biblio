mod review;

use crate::{
    app::{App, Page},
    detail,
    ui::*,
};
use biblio_core::commands::{self as api, file::folder_import, processing};
use nsh_core::prelude::*;
use std::collections::HashSet;

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
    let mut pickers = row().gap_2();
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
    let drop_area = view()
        .items_center()
        .justify_center()
        .gap_3()
        .py_8()
        .px_4()
        .min_h(px(200.))
        .border_1()
        .rounded(px(RADIUS))
        .transition_colors()
        .duration(tween(INSTANT))
        .style_with(move |s| if dragging.get() { s } else { s.border_dashed() })
        .border_color(move || if dragging.get() { accent() } else { keyline() })
        .bg(move || if dragging.get() { tint() } else { clear() })
        .child(icon(Icon::Import, secondary))
        .child(subheading("Drop files or folders here").text_center())
        .child(caption("or").text_center())
        .child(pickers);
    let sources = dynamic(move || {
        let list = app.sources.get();
        if list.is_empty() {
            return view().hidden().into_element();
        }
        let mut rows = view().border_t_1().border_color(rule);
        for path in list {
            let name = std::path::Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone());
            let doomed = path.clone();
            rows = rows.child(
                row()
                    .gap_3()
                    .min_h(px(36.))
                    .border_b_1()
                    .border_color(rule)
                    .child(
                        label(name)
                            .number_of_lines(1)
                            .flex_shrink_0()
                            .max_w(px(240.)),
                    )
                    .child(caption(path.clone()).number_of_lines(1).flex_1())
                    .child(icon_button(
                        format!("Remove {path}"),
                        Icon::Close,
                        None,
                        move || app.sources.update(|v| v.retain(|p| p != &doomed)),
                    )),
            );
        }
        rows.into_element()
    });
    let controls = wrap()
        .gap_2()
        .child(primary("Analyze selected files", move || analyze(app)))
        .child(confirm(
            "Discard import batch",
            "Discard this import batch?",
            "The chosen sources, files being analyzed and analyses waiting for review are dropped. Nothing has been imported yet.",
            true,
            move || {
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
            },
        ))
        .child(
            text(move || {
                format!(
                    "{} selected · {} analyzing · {} ready for review",
                    app.sources.with(Vec::len),
                    app.analyzing.with(Vec::len),
                    app.pending.with(Vec::len)
                )
            })
            .text_size(px(13.))
            .text_color(secondary)
            .pl_2(),
        );
    view()
        .flex_1()
        .min_h_0()
        .gap_4()
        .on_file_drag_enter(move |_: &()| dragging.set(true))
        .on_file_drag_leave(move |_: &()| dragging.set(false))
        .on_file_drop(move |e: &FileDropEvent| {
            dragging.set(false);
            drop_urls(app, &e.uris);
        })
        .child(heading("Import"))
        .child(
            scroll().flex_1().min_h_0().child(
                view()
                    .gap_4()
                    .pb_8()
                    .max_w(px(760.))
                    .child(setting(
                        "Category",
                        Some("Files are analyzed with this category's schema."),
                        detail::category_picker(app, app.category),
                    ))
                    .child(drop_area)
                    .child(sources)
                    .child(controls)
                    .child(dynamic(move || {
                        if first.get().is_some() {
                            let item = app.pending.get_untracked().remove(0);
                            untracked(|| review::screen(app, item))
                        } else {
                            view().into_element()
                        }
                    })),
            ),
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
