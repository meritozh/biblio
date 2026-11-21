//! Selection mode: a tick on each file and a bar of bulk commands.
use crate::{app::App, ui::*};
use biblio_core::commands::{author, cache, remote, tag};
use nsh_core::prelude::*;
use std::collections::BTreeSet;

#[derive(Clone, Copy)]
pub struct Selection {
    pub active: Signal<bool>,
    pub ids: Signal<BTreeSet<i64>>,
}
impl Selection {
    pub fn new() -> Self {
        Self {
            active: signal(false),
            ids: signal(BTreeSet::new()),
        }
    }
    pub fn toggle(self, id: i64) {
        self.ids.update(|ids| {
            if !ids.remove(&id) {
                ids.insert(id);
            }
        });
    }
    pub fn clear(self) {
        self.ids.set(BTreeSet::new());
    }
    /// The tick for one file while selecting.
    pub fn control(self, id: i64, name: &str) -> AnyElement {
        let name = format!("Select {name}");
        when(
            move || self.active.get(),
            move || {
                tick(
                    name.clone(),
                    move || self.ids.with(|ids| ids.contains(&id)),
                    move || self.toggle(id),
                )
            },
        )
        .into_element()
    }
    /// The bulk commands, shown while selecting. `shown` gives the files
    /// loaded on the shelf.
    pub fn bar(self, app: App, shown: impl Fn() -> Vec<i64> + Copy + 'static) -> AnyElement {
        when(
            move || self.active.get(),
            move || {
                let ids = move || {
                    self.ids
                        .with_untracked(|s| s.iter().copied().collect::<Vec<_>>())
                };
                let count = move || self.ids.with_untracked(BTreeSet::len);
                let more = menu(trigger("More selection commands", Some(Icon::More), false, false))
                    .child(menu_item("Upload to Baidu Netdisk", Some(Icon::Upload), move || {
                        let n = count();
                        ask(
                            format!("Upload {n} files to Baidu Netdisk?"),
                            "Each file is encrypted on this computer, uploaded, and then removed from local storage.",
                            "Upload",
                            false,
                            move || app.job(remote::file_upload_to_remote(app.db(), ids()), |_| {}),
                        )
                    }))
                    .child(menu_item("Clear cached copies", Some(Icon::Trash), move || {
                        let n = count();
                        ask(
                            format!("Clear cached copies of {n} files?"),
                            "Remote files keep their remote copy; only the local cache is removed.",
                            "Clear caches",
                            false,
                            move || clear_caches(app, ids()),
                        )
                    }))
                    .child(menu_separator())
                    .child(menu_danger("Delete selected files", Icon::Trash, move || {
                        let n = count();
                        ask(
                            format!("Delete {n} files and their library records?"),
                            "The files are removed from storage, including remote copies. This cannot be undone.",
                            "Delete",
                            true,
                            move || app.job(remote::file_delete_via_worker(app.db(), ids()), |_| {}),
                        )
                    }));
                wrap()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .rounded(px(RADIUS))
                    .bg(tint)
                    .child(
                        text(move || format!("{} selected", self.ids.with(BTreeSet::len)))
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ink)
                            .min_w(px(88.)),
                    )
                    .child(ghost("Select all shown", move || {
                        let shown = untracked(shown);
                        self.ids.update(|s| s.extend(shown));
                    }))
                    .child(ghost("Clear selection", move || self.clear()))
                    .child(view().w(px(1.)).h(px(20.)).bg(keyline))
                    .child(self.assign(app, true))
                    .child(self.assign(app, false))
                    .child(control(
                        "Download",
                        Variant::Ghost,
                        ButtonSize::Sm,
                        Some(Icon::Download),
                        move || app.job(remote::file_download_from_remote(app.db(), ids()), |_| {}),
                    ))
                    .child(more)
            },
        )
        .into_element()
    }
}

fn clear_caches(app: App, selected: Vec<i64>) {
    let db = app.db();
    app.job(
        async move {
            let rows = biblio_core::commands::file::listing::file_list_by_ids(db.clone(), selected)
                .await?;
            let mut count = 0;
            for row in rows {
                if row.local_cache_path.is_some() {
                    cache::cache_clear(db.clone(), row.id).await?;
                    count += 1;
                }
            }
            Ok(count)
        },
        move |n| {
            app.message.set(format!("{n} caches cleared"));
            app.refresh();
        },
    );
}

impl Selection {
    /// Assign a tag or an author to every selected file, from a popover
    /// that searches the names.
    fn assign(self, app: App, tags: bool) -> AnyElement {
        let name = if tags { "Assign tag" } else { "Assign author" };
        popover(
            trigger(
                name,
                Some(if tags { Icon::Tags } else { Icon::Authors }),
                true,
                true,
            )
            .border_color(clear)
            .min_h(px(28.)),
            move || {
                let query = signal(String::new());
                view()
                    .gap_2()
                    .child(field_label(name))
                    .child(input(if tags { "Find tag" } else { "Find author" }, query))
                    .child(dynamic(move || {
                        let q = query.get();
                        if q.trim().is_empty() {
                            return caption("Type to find a name.").into_element();
                        }
                        app.load(
                            crate::detail::find_names(app.db(), tags, q),
                            move |matches| {
                                if matches.is_empty() {
                                    return caption("No matching names.").into_element();
                                }
                                let mut list = view().gap(px(1.));
                                for (target, name) in matches {
                                    let target = *target;
                                    list = list.child(
                                        control(
                                            name,
                                            Variant::Ghost,
                                            ButtonSize::Sm,
                                            None,
                                            move || {
                                                let files = self.ids.with_untracked(|s| {
                                                    s.iter().copied().collect::<Vec<_>>()
                                                });
                                                let db = app.db();
                                                app.local_job(
                                                    async move {
                                                        for &id in &files {
                                                            if tags {
                                                                tag::tag_assign(
                                                                    db.clone(),
                                                                    id,
                                                                    vec![target],
                                                                )
                                                                .await?;
                                                            } else {
                                                                author::author_assign(
                                                                    db.clone(),
                                                                    id,
                                                                    vec![target],
                                                                )
                                                                .await?;
                                                            }
                                                        }
                                                        Ok(files.len())
                                                    },
                                                    move |n| {
                                                        query.set(String::new());
                                                        app.refresh();
                                                        app.message
                                                            .set(format!("{n} files updated"));
                                                    },
                                                );
                                            },
                                        )
                                        .w_full()
                                        .justify_start(),
                                    );
                                }
                                list.into_element()
                            },
                        )
                    }))
                    .into_element()
            },
        )
        .into_element()
    }
}
