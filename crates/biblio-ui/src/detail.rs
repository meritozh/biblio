//! One file: its slip, the commands about it, and its form.
use crate::{
    app::{App, Page},
    ui::*,
};
use biblio_core::commands::{
    self as api, author, cache, cover as artwork, file, metadata, remote, tag,
};
use nsh_core::prelude::*;

async fn cover_uri(db: biblio_core::Library, id: i64) -> Result<Option<String>, String> {
    match artwork::cover_get(db, id).await {
        Ok(c) => Ok(Some(format!("data:{};base64,{}", c.mime_type, c.data))),
        Err(e) if e == "COVER_NOT_FOUND" => Ok(None),
        Err(e) => Err(e),
    }
}

fn extension_of(path: &str) -> String {
    std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_uppercase()
}

/// A catalogue slip in a 3:4 frame: the real cover when there is one,
/// otherwise the title typeset on a plain ruled slip. `with_title` adds
/// the title under a real cover (the shelf; the file page has its own).
pub(crate) fn slip_face(
    app: App,
    id: i64,
    title: String,
    extension: String,
    progress: String,
    with_title: bool,
) -> AnyElement {
    app.load(cover_uri(app.db(), id), move |uri| {
        if let Some(uri) = uri {
            let mut art = view().gap_2().child(
                image(uri.clone())
                    .w_full()
                    .aspect_ratio(0.75)
                    .rounded(px(RADIUS_SM))
                    .border_1()
                    .border_color(rule),
            );
            if with_title {
                art = art.child(
                    label(&title)
                        .font_weight(FontWeight::MEDIUM)
                        .number_of_lines(2),
                );
            }
            return art.into_element();
        }
        view()
            .w_full()
            .aspect_ratio(0.75)
            .justify_between()
            .p_3()
            .gap_2()
            .rounded(px(RADIUS_SM))
            .bg(sheet)
            .border_1()
            .border_color(keyline)
            // The leading corners stay clear for the shelf's favourite and
            // storage marks.
            .child(
                caption(extension.clone())
                    .letter_spacing(px(0.8))
                    .number_of_lines(1)
                    .self_end(),
            )
            .child(
                subheading(&title)
                    .text_size(px(16.))
                    .line_height(px(21.))
                    .number_of_lines(5),
            )
            .child(
                row()
                    .justify_end()
                    .pt_2()
                    .min_h(px(25.))
                    .border_t_1()
                    .border_color(rule)
                    .child(caption(progress.clone()).number_of_lines(1)),
            )
            .into_element()
    })
}

/// The shelf's slip: pressing it opens the file.
pub fn card_face(app: App, file: &api::FileListItem) -> AnyElement {
    let id = file.id;
    nsh_base::button()
        .accessibility_label(file.display_name.clone())
        .on_press(move || app.page.set(Page::File(id)))
        .w_full()
        .rounded(px(RADIUS_SM))
        .transition()
        .duration(tween(INSTANT))
        .hover(|s| s.opacity(0.86))
        .active(|s| s.opacity(0.74).scale(press_scale(0.98)))
        .focus(|s| {
            s.outline(px(2.))
                .outline_color(accent)
                .outline_offset(px(2.))
        })
        .child(slip_face(
            app,
            id,
            file.display_name.clone(),
            extension_of(&file.path),
            file.progress.clone().unwrap_or_default(),
            false,
        ))
        .into_element()
}

pub fn screen(app: App, id: i64) -> AnyElement {
    dynamic(move || {
        app.revision.get();
        app.load(file::create_replace::file_get(app.db(), id), move |f| {
            file_page(app, f)
        })
    })
    .into_element()
}

fn reveal_in_folder(app: App, path: String) {
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
}

fn choose_cover(app: App, id: i64) {
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
}

/// Every command about the file that is not part of its form.
fn actions(app: App, file: &api::FileWithDetails) -> impl IntoElement {
    let id = file.id;
    let name = file.display_name.clone();
    let path = file.path.clone();
    let local = file
        .local_cache_path
        .clone()
        .or_else(|| (file.storage_kind.as_deref() != Some("remote")).then(|| file.path.clone()));
    let remote_file = file.storage_kind.as_deref() == Some("remote");
    let cached = file.local_cache_path.is_some();
    let mut m = menu(trigger("More actions", Some(Icon::More), false, false))
        .align(Align::End)
        .child(menu_item("Copy path", Some(Icon::Copy), move || {
            set_clipboard(path.clone());
            app.message.set("Path copied".into());
        }));
    if let Some(local) = local {
        m = m.child(menu_item("Show in folder", Some(Icon::Reveal), move || {
            reveal_in_folder(app, local.clone())
        }));
    }
    m = m
        .child(menu_separator())
        .child(menu_item("Choose cover…", Some(Icon::Cover), move || {
            choose_cover(app, id)
        }))
        .child(menu_item("Remove cover", Some(Icon::Close), move || {
            ask(
                "Remove the cover?",
                "The file keeps its catalogue slip; you can choose a cover again later.",
                "Remove cover",
                true,
                move || app.save(artwork::cover_delete(app.db(), id)),
            )
        }))
        .child(menu_separator());
    if remote_file {
        m = m.child(menu_item("Download", Some(Icon::Download), move || {
            app.job(
                remote::file_download_from_remote(app.db(), vec![id]),
                |_| {},
            )
        }));
        if cached {
            m = m.child(menu_item(
                "Clear cached file",
                Some(Icon::Trash),
                move || {
                    ask(
                        "Clear the cached copy?",
                        "The remote copy stays; open the file again to download it.",
                        "Clear cached file",
                        false,
                        move || app.save(cache::cache_clear(app.db(), id)),
                    )
                },
            ));
        }
    } else {
        m = m.child(menu_item(
            "Move to Baidu Netdisk",
            Some(Icon::Upload),
            move || {
                ask(
                    "Move this file to Baidu Netdisk?",
                    "It is encrypted on this computer, uploaded, and removed from local storage.",
                    "Move to Baidu Netdisk",
                    false,
                    move || app.job(remote::file_upload_to_remote(app.db(), vec![id]), |_| {}),
                )
            },
        ));
    }
    m.child(menu_separator())
        .child(menu_danger("Delete file and library record", Icon::Trash, move || {
            ask(
                format!("Delete “{name}”?"),
                "The file is removed from storage, including any remote copy, and its library record is deleted. This cannot be undone.",
                "Delete",
                true,
                move || {
                    app.job(file::delete_move::file_delete(app.db(), id), move |_| {
                        app.page.set(Page::Library);
                        app.refresh();
                    })
                },
            )
        }))
}

fn file_page(app: App, file: &api::FileWithDetails) -> AnyElement {
    let id = file.id;
    let favorite = file.is_favorite;
    let header = header_bar(
        icon_button("Back to library", Icon::Back, Some("⌘["), move || {
            app.page.set(Page::Library)
        })
        .into_element(),
        file.display_name.clone(),
        Some(
            caption(extension_of(&file.path))
                .letter_spacing(px(0.6))
                .into_element(),
        ),
        Some(actions(app, file).into_element()),
    );
    let aside = view()
        .gap_2()
        .w(move || if compact() { px(160.) } else { px(200.) })
        .flex_shrink_0()
        .child(slip_face(
            app,
            id,
            file.display_name.clone(),
            extension_of(&file.path),
            file.progress.clone().unwrap_or_default(),
            false,
        ))
        .child(
            control(
                "Open file",
                Variant::Default,
                ButtonSize::Md,
                Some(Icon::Open),
                move || app.job(cache::cache_open(app.db(), id), |_| {}),
            )
            .w_full(),
        )
        .child(
            control(
                if favorite { "Unfavorite" } else { "Favorite" },
                Variant::Ghost,
                ButtonSize::Md,
                Some(Icon::Star),
                move || {
                    app.save(file::update_favorite::file_set_favorite(
                        app.db(),
                        id,
                        !favorite,
                    ))
                },
            )
            .w_full(),
        );
    let body = form(app, file);
    page(
        header,
        page_scroll(
            960.,
            wrap()
                .items_start()
                .gap_x(px(40.))
                .gap_y(px(24.))
                .child(aside)
                .child(body),
        ),
    )
    .into_element()
}

fn form(app: App, file: &api::FileWithDetails) -> View {
    let id = file.id;
    let name = signal(file.display_name.clone());
    let progress = signal(file.progress.clone().unwrap_or_default());
    let cat = signal(file.category_id);
    let original = file.category_id;
    let mut body = view()
        .gap_4()
        .flex_1()
        .min_w(px(280.))
        .max_w(px(640.))
        // The header names the file; the form starts with where it lives.
        .child(caption(&file.path).selectable(true).number_of_lines(2))
        .child(
            section("Details")
                .child(field("Title", name))
                .child(field("Progress", progress))
                .child(
                    view()
                        .gap_1()
                        .child(field_label("Category"))
                        .child(category_picker(app, cat)),
                )
                .child(primary("Save file", move || {
                    app.save(file::update_favorite::file_update(
                        app.db(),
                        id,
                        Some(name.get_untracked()),
                        cat.get_untracked().filter(|v| Some(*v) != original),
                        Some(progress.get_untracked()),
                    ))
                })),
        );
    for (kind, is_tag) in [("Tags", true), ("Authors", false)] {
        let mut attached = wrap().gap(px(6.));
        let names: Vec<(i64, String)> = if is_tag {
            file.tags.iter().map(|t| (t.id, t.name.clone())).collect()
        } else {
            file.authors
                .iter()
                .map(|a| (a.id, a.name.clone()))
                .collect()
        };
        if names.is_empty() {
            attached = attached.child(caption(if is_tag {
                "No tags yet."
            } else {
                "No authors yet."
            }));
        }
        for (other, label_text) in names {
            attached = attached.child(chip(label_text, move || {
                if is_tag {
                    app.save(tag::tag_unassign(app.db(), id, vec![other]))
                } else {
                    app.save(author::author_unassign(app.db(), id, vec![other]))
                }
            }));
        }
        let q = signal(String::new());
        let finder = if is_tag { "Find tags" } else { "Find authors" };
        body = body.child(
            section(kind)
                .child(attached)
                .child(input(finder, q))
                .child(dynamic(move || {
                    let name = q.get();
                    if name.is_empty() {
                        return view().into_element();
                    }
                    let db = app.db();
                    app.load(
                        async move { find_names(db, is_tag, name).await },
                        move |values| {
                            if values.is_empty() {
                                return caption("No matching names.").into_element();
                            }
                            let mut r = wrap().gap(px(6.));
                            for (other, name) in values {
                                let other = *other;
                                r = r.child(control(
                                    name,
                                    Variant::Default,
                                    ButtonSize::Sm,
                                    Some(Icon::Plus),
                                    move || {
                                        if is_tag {
                                            app.save(tag::tag_assign(app.db(), id, vec![other]));
                                        } else {
                                            app.save(author::author_assign(
                                                app.db(),
                                                id,
                                                vec![other],
                                            ));
                                        }
                                    },
                                ));
                            }
                            r.into_element()
                        },
                    )
                })),
        );
    }
    let mut meta = section("Metadata");
    let mut keys = std::collections::HashSet::new();
    for m in &file.metadata {
        keys.insert(m.key.clone());
        meta = meta.child(metadata_field(
            app,
            id,
            m.key.clone(),
            m.value.clone(),
            m.data_type.clone(),
        ));
    }
    if let Some(category) = &file.category {
        let slug = category.schema_slug.clone();
        meta = meta.child(
            app.load(api::schemas::schema_list(app.db()), move |schemas| {
                let mut fields = view().gap_3();
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
    meta = meta.child(
        view().gap_1().child(field_label("Add a value")).child(
            wrap()
                .gap_2()
                .child(input("Metadata key", key).flex_1().min_w(px(140.)))
                .child(input("Value", value).flex_1().min_w(px(140.)))
                .child(control(
                    "Add metadata",
                    Variant::Default,
                    ButtonSize::Md,
                    Some(Icon::Plus),
                    move || {
                        app.save(metadata::metadata_set(
                            app.db(),
                            id,
                            key.get_untracked(),
                            value.get_untracked(),
                            None,
                        ))
                    },
                )),
        ),
    );
    body.child(meta)
}

fn metadata_field(app: App, id: i64, key: String, initial: String, ty: String) -> AnyElement {
    let value = signal(initial);
    let name = key.clone();
    let doomed = key.clone();
    view()
        .gap_1()
        .child(field_label(key.clone()))
        .child(
            row()
                .gap_2()
                .child(input(&key, value).flex_1())
                .child(small("Save value", move || {
                    app.save(metadata::metadata_set(
                        app.db(),
                        id,
                        name.clone(),
                        value.get_untracked(),
                        Some(ty.clone()),
                    ))
                }))
                .child(icon_button(
                    format!("Remove value {key}"),
                    Icon::Trash,
                    None,
                    move || {
                        let doomed = doomed.clone();
                        ask(
                            format!("Remove “{doomed}”?"),
                            "The value is deleted from this file.",
                            "Remove value",
                            true,
                            move || {
                                app.save(metadata::metadata_delete(app.db(), id, doomed.clone()))
                            },
                        )
                    },
                )),
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
/// A select of the library's categories bound to `selected`.
pub fn category_picker(app: App, selected: Signal<Option<i64>>) -> AnyElement {
    app.load(api::category::category_list(app.db()), move |categories| {
        if categories.is_empty() {
            return caption("Create a category first, under Categories.").into_element();
        }
        let items = categories
            .iter()
            .map(|c| (c.id.to_string(), c.name.clone()))
            .collect::<Vec<_>>();
        select(
            "Category",
            items,
            move || selected.get().map(|v| v.to_string()),
            move |v| selected.set(v.parse().ok()),
        )
        .into_element()
    })
}
