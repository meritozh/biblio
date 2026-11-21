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
    pub fn control(self, id: i64) -> AnyElement {
        when(
            move || self.active.get(),
            move || {
                nsh_base::checkbox()
                    .checked(move || self.ids.with(|ids| ids.contains(&id)))
                    .accessibility_label(format!("Select file {id}"))
                    .on_change(move |_: &bool| self.toggle(id))
                    .child(
                        text(move || {
                            if self.ids.with(|s| s.contains(&id)) {
                                "☑ Selected"
                            } else {
                                "☐ Select"
                            }
                        })
                        .text_color(ink),
                    )
            },
        )
        .into_element()
    }
    pub fn bar(self, app: App) -> AnyElement {
        stack()
            .child(button("Select files", move || {
                self.active.update(|v| *v = !*v);
                self.clear();
            }))
            .child(when(
                move || self.active.get(),
                move || {
                    let ids = move || {
                        self.ids
                            .with_untracked(|s| s.iter().copied().collect::<Vec<_>>())
                    };
                    wrap()
                        .child(
                            text(move || format!("{} selected", self.ids.with(BTreeSet::len)))
                                .text_color(ink),
                        )
                        .child(button("Clear selection", move || self.clear()))
                        .child(self.assign(app, true))
                        .child(self.assign(app, false))
                        .child(confirm("Upload selected files", move || {
                            app.job(remote::file_upload_to_remote(app.db(), ids()), |_| {})
                        }))
                        .child(button("Download selected files", move || {
                            app.job(remote::file_download_from_remote(app.db(), ids()), |_| {})
                        }))
                        .child(confirm("Clear selected caches", move || {
                            let selected = ids();
                            let db = app.db();
                            app.job(
                                async move {
                                    let rows =
                                        biblio_core::commands::file::listing::file_list_by_ids(
                                            db.clone(),
                                            selected,
                                        )
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
                        }))
                        .child(confirm("Delete selected files and records", move || {
                            app.job(remote::file_delete_via_worker(app.db(), ids()), |_| {})
                        }))
                },
            ))
            .into_element()
    }
}

impl Selection {
    fn assign(self, app: App, tags: bool) -> AnyElement {
        let query = signal(String::new());
        let name = if tags { "Assign tag" } else { "Assign author" };
        stack()
            .child(input(name, query))
            .child(dynamic(move || {
                let q = query.get();
                if q.trim().is_empty() {
                    return view().into_element();
                }
                app.load(
                    crate::detail::find_names(app.db(), tags, q),
                    move |matches| {
                        let mut buttons = wrap();
                        for (target, name) in matches {
                            let target = *target;
                            buttons = buttons.child(button(name, move || {
                                let files = self
                                    .ids
                                    .with_untracked(|s| s.iter().copied().collect::<Vec<_>>());
                                let db = app.db();
                                app.local_job(
                                    async move {
                                        for &id in &files {
                                            if tags {
                                                tag::tag_assign(db.clone(), id, vec![target])
                                                    .await?;
                                            } else {
                                                author::author_assign(db.clone(), id, vec![target])
                                                    .await?;
                                            }
                                        }
                                        Ok(files.len())
                                    },
                                    move |n| {
                                        query.set(String::new());
                                        app.refresh();
                                        app.message.set(format!("{n} files updated"));
                                    },
                                );
                            }));
                        }
                        buttons.into_element()
                    },
                )
            }))
            .into_element()
    }
}
