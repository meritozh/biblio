//! The shelf's views: the grid of slips and the table of files, which load
//! more as they scroll; the groups of a grouped view; the lucky picks.
use super::*;
use crate::reader::navigation;
use biblio_core::commands::text_open::Entry;
use nsh_base::{DataTable, DataTableApi, DataTableContextMenu, DataTableElement, DataTableRange};
use nsh_core::element::desktop::{MenuItem, popup_menu_at};

fn press_file(app: App, id: i64, title: &str, selection: Selection) {
    if selection.active.get_untracked() {
        selection.toggle(id);
    } else {
        navigation::activate(app, id, title, Entry::Shelf);
    }
}

/// How many slips fit side by side in `width`.
fn columns_for(width: f32) -> u32 {
    (((width + GRID_GAP_X) / (SLIP_MIN + GRID_GAP_X)).floor() as u32).max(1)
}

/// The shelf as a grid of slips, loading more as it scrolls.
pub(super) fn file_grid(
    app: App,
    files: Paged<Shelved>,
    selection: Selection,
    anchor: Stored<usize>,
) -> AnyElement {
    let list = virtual_list(
        move || files.len(),
        move |at: Position| {
            // A cell shows another file as it is recycled; it is made again
            // only when the file it shows changes.
            let item = computed(move || files.at(at.index()).map(Same));
            dynamic(move || match item.get() {
                Some(Same(item)) => untracked(|| slip(app, &item, selection)),
                None => view().into_element(),
            })
        },
    )
    .keys(move || {
        files
            .rows
            .with(|r| r.iter().map(|s| s.file.id).collect::<Vec<_>>())
    })
    .on_load_more(move || files.more())
    .test_id("shelf")
    .footer(loading_more(files));
    slip_grid(list, files.len(), anchor)
}

/// Apply the saved index after layout gives the grid its real column count.
fn slip_grid(list: VirtualList, count: usize, anchor: Stored<usize>) -> AnyElement {
    let width = signal(0f32);
    let start = anchor.get().min(count.saturating_sub(1));
    let pending = stored(start > 0);
    let handle = element_ref();
    let list = list
        .element_ref(&handle)
        .columns(move || columns_for(width.get() - inset().0 * 2.))
        .column_gap(px(GRID_GAP_X))
        .row_gap(px(GRID_GAP_Y))
        .content_padding_x(inset())
        .content_padding_top(px(4.))
        .content_padding_bottom(px(32.))
        .on_viewable_items_change(move |v: &ViewableItems| {
            if let Some((first, _)) = v.viewable
                && !pending.get()
            {
                anchor.replace(first);
            }
        })
        .flex_1()
        .min_h_0();
    view()
        .flex_1()
        .min_h_0()
        .on_layout(move |b: &Bounds<Points>| {
            let first_layout = width.get_untracked() == 0. && b.size.width.0 > 0.;
            width.set(b.size.width.0);
            if first_layout && pending.get() {
                spawn(async move {
                    next_frame().await;
                    pending.replace(false);
                    handle.scroll_to_index(start as u32, Percentage(0.), px(0.), false);
                });
            }
        })
        .child(list)
        .into_element()
}

/// "Loading…" under the last row while the next files are on their way.
fn loading_more(files: Paged<Shelved>) -> AnyElement {
    when(
        move || files.loading.get() && files.len() > 0,
        || view().items_center().py_4().child(caption("Loading…")),
    )
    .into_element()
}

/// An item compared by identity: a reload makes new ones, a chunk loaded
/// after it does not touch it.
struct Same<T>(Rc<T>);
impl<T> Clone for Same<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T> PartialEq for Same<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// The first nonempty range arrives once the table has data and a viewport.
/// Restore there before recording subsequent scrolls in the shared shelf state.
fn remember_rows(
    table: DataTableElement,
    handle: ComponentRef<DataTable>,
    anchor: Stored<usize>,
    count: impl Fn() -> usize + 'static,
) -> DataTableElement {
    let restore = stored(anchor.get() > 0);
    table
        .component_ref(&handle)
        .on_visible_rows_change(move |range: &DataTableRange| {
            if range.end <= range.start {
                return;
            }
            if restore.replace(false)
                && let Some(api) = handle.api::<DataTableApi>()
            {
                let row = anchor.get().min(count().saturating_sub(1));
                api.scroll_to_row(row as u32, Some(false));
                anchor.replace(api.visible_rows().start as usize);
            } else {
                anchor.replace(range.start as usize);
            }
        })
}

/// The shelf as a table: one row per file, loading more as it scrolls.
pub(super) fn file_table(
    app: App,
    files: Paged<Shelved>,
    selection: Selection,
    anchor: Stored<usize>,
) -> AnyElement {
    let cols = vec![
        Col::custom("title", "Title", None),
        Col::muted("authors", "Authors", Some(200.)),
        Col::muted("format", "Format", Some(80.)),
        Col::muted("status", "Status", Some(140.)),
        Col::muted("added", "Added", Some(110.)),
        Col::custom("favorite", "", Some(44.)),
    ];
    let keys: Rc<Vec<&'static str>> = Rc::new(cols.iter().map(|c| c.key).collect());
    let rows = move || {
        files.rows.with(|rows| {
            nsh_base::DataTableRows::new(rows.len())
                .texts("title", rows.iter().map(|s| s.file.display_name.as_str()))
                .texts("authors", rows.iter().map(|s| authors_of(&s.file)))
                .texts("format", rows.iter().map(|s| format_of(&s.file)))
                .texts("status", rows.iter().map(|s| status_of(&s.file)))
                .texts("added", rows.iter().map(|s| date_of(&s.file.created_at)))
                .texts(
                    "favorite",
                    rows.iter().map(|s| {
                        if s.file.is_favorite {
                            "Unfavorite"
                        } else {
                            "Favorite"
                        }
                    }),
                )
        })
    };
    let cell = move |c: &nsh_base::DataTableCell| -> AnyElement {
        let at = c.row as usize;
        if c.key.as_str() == "favorite" {
            let on = files.at(at).is_some_and(|s| s.file.is_favorite);
            return view()
                .label(c.text.to_string())
                .role(AccessibilityRole::Button)
                .w(px(28.))
                .h(px(28.))
                .items_center()
                .justify_center()
                .child(favorite_mark(on))
                .into_element();
        }
        let id = files.at(at).map(|s| s.file.id);
        let mut line = row().gap_3().min_w_0();
        if let Some(id) = id {
            line = line.child(when(
                move || selection.active.get(),
                move || {
                    let on = move || selection.ids.with(|s| s.contains(&id));
                    view()
                        .w(px(16.))
                        .h(px(16.))
                        .flex_shrink_0()
                        .items_center()
                        .justify_center()
                        .rounded(px(RADIUS_SM))
                        .border_1()
                        .border_color(move || if on() { accent() } else { keyline() })
                        .bg(move || if on() { accent() } else { sheet() })
                        .child(
                            icon(
                                Icon::Check,
                                move || if on() { on_accent() } else { clear() },
                            )
                            .w(px(12.))
                            .h(px(12.)),
                        )
                },
            ));
        }
        line.child(
            label(c.text.to_string())
                .font_weight(FontWeight::MEDIUM)
                .number_of_lines(1)
                .flex_shrink(1.),
        )
        .into_element()
    };
    let handle = component_ref::<DataTable>();
    view()
        .flex_1()
        .min_h_0()
        .px(inset)
        .pb_5()
        .child(table(
            "Files",
            cols,
            move || files.len(),
            rows,
            cell,
            move |t| {
                remember_rows(t, handle, anchor, move || files.len())
                    .has_more(move || files.has_more())
                    .loading(move || files.loading.get())
                    .on_load_more(move || files.more())
                    .row_classes(move || {
                        let picked = hex(colors().tint);
                        files.rows.with(|rows| {
                            selection.ids.with(|ids| {
                                rows.iter()
                                    .map(|s| {
                                        if selection.active.get() && ids.contains(&s.file.id) {
                                            format!("bg-[{picked}]").into()
                                        } else {
                                            SharedString::default()
                                        }
                                    })
                                    .collect::<Vec<SharedString>>()
                            })
                        })
                    })
                    .on_row_press(move |p: &nsh_base::DataTablePress| {
                        let Some(item) = files.at(p.row as usize) else {
                            return;
                        };
                        let id = item.file.id;
                        if pressed_key(&keys, p) == Some("favorite") {
                            app.save(file::update_favorite::file_set_favorite(
                                app.db(),
                                id,
                                !item.file.is_favorite,
                            ));
                        } else {
                            press_file(app, id, &item.file.display_name, selection);
                        }
                    })
                    .on_context_menu(move |hit: &DataTableContextMenu| {
                        let Some(api) = handle.api::<DataTableApi>() else {
                            return;
                        };
                        let Some(item) = hit.row.and_then(|row| files.at(row as usize)) else {
                            api.clear_menu_target();
                            return;
                        };
                        let id = item.file.id;
                        let closed = popup_menu_at(
                            vec![
                                MenuItem::new("View details")
                                    .on_select(move || app.page.set(Page::File(id))),
                            ],
                            point(hit.x, hit.y),
                        );
                        spawn(async move {
                            closed.await;
                            if let Some(api) = handle.api::<DataTableApi>() {
                                api.clear_menu_target();
                            }
                        });
                    })
            },
        ))
        .into_element()
}

/// The groups of a grouped view, as slips (grid) or rows (list); a press
/// opens one.
pub(super) fn group_shelf(
    app: App,
    groups: Signal<Option<Rc<Result<Vec<Collection>, String>>>>,
    opened: Signal<Option<Opened>>,
    grid: bool,
    anchor: Stored<usize>,
) -> AnyElement {
    let list = groups.get_untracked();
    let Some(Ok(list)) = list.as_deref() else {
        return view().into_element();
    };
    let items: Rc<Vec<(Opened, Option<i64>)>> = Rc::new(
        list.iter()
            .map(|g| {
                (
                    Opened {
                        title: g.title.clone(),
                        ids: Rc::new(g.file_ids.clone()),
                    },
                    g.cover_file_id.or_else(|| g.file_ids.first().copied()),
                )
            })
            .collect(),
    );
    let n = items.len();
    if !grid {
        let open_row = items.clone();
        let rows = nsh_base::DataTableRows::new(n)
            .texts("title", items.iter().map(|(g, _)| g.title.as_str()))
            .texts("files", items.iter().map(|(g, _)| g.ids.len().to_string()));
        return view()
            .flex_1()
            .min_h_0()
            .px(inset)
            .pb_5()
            .child(table(
                "Groups",
                vec![
                    Col::text("title", "Group", None),
                    Col::muted("files", "Files", Some(96.)).end(),
                ],
                move || n,
                move || rows.clone(),
                |_| view().into_element(),
                move |t| {
                    remember_rows(t, component_ref::<DataTable>(), anchor, move || n).on_row_press(
                        move |p: &nsh_base::DataTablePress| {
                            if let Some((g, _)) = open_row.get(p.row as usize) {
                                opened.set(Some(g.clone()));
                            }
                        },
                    )
                },
            ))
            .into_element();
    }
    let list = virtual_list(n, move |at: Position| {
        let items = items.clone();
        dynamic(move || {
            let Some((group, cover)) = items.get(at.index()).cloned() else {
                return view().into_element();
            };
            let count = counted(group.ids.len() as i64, "file", "files");
            let title_text = group.title.clone();
            view()
                .gap(px(6.))
                .child(
                    nsh_base::button()
                        .accessibility_label(title_text.clone())
                        .on_press(move || opened.set(Some(group.clone())))
                        .w_full()
                        .aspect_ratio(0.75)
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
                        .child(match cover {
                            Some(id) => detail::slip_face(
                                app,
                                id,
                                title_text.clone(),
                                String::new(),
                                count.clone(),
                                false,
                            ),
                            None => view().w_full().aspect_ratio(0.75).into_element(),
                        }),
                )
                .child(
                    label(title_text)
                        .font_weight(FontWeight::MEDIUM)
                        .number_of_lines(2),
                )
                .child(caption(count))
                .into_element()
        })
    });
    slip_grid(list, n, anchor)
}

/// Three files picked at random from what the shelf answers, with Shuffle
/// again; a press opens one.
pub(super) fn lucky_picks(
    app: App,
    cat_id: Option<i64>,
    query: Signal<String>,
    chips: Signal<Vec<search::Chip>>,
    open: Signal<bool>,
    selection: Selection,
) -> AnyElement {
    let round = signal(0u32);
    view()
        .gap_4()
        .child(dynamic(move || {
            round.get();
            let q = query.get_untracked();
            let filters = chips
                .get_untracked()
                .into_iter()
                .map(|c| c.1)
                .collect::<Vec<_>>();
            app.load(
                file::search_lucky_status::file_lucky(
                    app.db(),
                    cat_id,
                    Some(q),
                    Some(filters),
                    Some(3),
                ),
                move |picks| {
                    if picks.is_empty() {
                        return empty("Nothing to pick", "No file answers the current search.")
                            .into_element();
                    }
                    let mut shelf = view()
                        .grid()
                        .grid_template_columns("repeat(3, minmax(0, 1fr))")
                        .gap_x(px(20.));
                    for file in picks {
                        let id = file.id;
                        let title = file.display_name.clone();
                        shelf = shelf.child(
                            view()
                                .gap(px(6.))
                                .min_w_0()
                                .child(detail::card_face(app, file, move || {
                                    open.set(false);
                                    press_file(app, id, &title, selection);
                                }))
                                .child(
                                    label(file.display_name.clone())
                                        .font_weight(FontWeight::MEDIUM)
                                        .number_of_lines(2),
                                )
                                .child(caption(authors_of(file)).number_of_lines(1)),
                        );
                    }
                    shelf.into_element()
                },
            )
        }))
        .child(
            row()
                .justify_end()
                .gap_2()
                .child(button("Close", move || open.set(false)))
                .child(control(
                    "Shuffle again",
                    Variant::Primary,
                    ButtonSize::Md,
                    Some(Icon::Shuffle),
                    move || round.update(|n| *n += 1),
                )),
        )
        .into_element()
}

fn authors_of(file: &FileListItem) -> String {
    file.authors
        .iter()
        .map(|a| a.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}
fn status_of(file: &FileListItem) -> String {
    format!(
        "{} · {}",
        file.storage_kind.as_deref().unwrap_or("local"),
        file.file_status
    )
}
fn format_of(file: &FileListItem) -> String {
    std::path::Path::new(&file.path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_uppercase()
}
/// The date part of a stored timestamp.
fn date_of(stamp: &str) -> String {
    stamp.split([' ', 'T']).next().unwrap_or(stamp).to_owned()
}

/// The favourite toggle on a slip: always shown on a favourite, otherwise
/// only while the pointer is over its corner or it has the focus.
fn favorite(app: App, id: i64, on: bool) -> impl IntoElement {
    let name = if on { "Unfavorite" } else { "Favorite" };
    nsh_base::button()
        .accessibility_label(name)
        .selected(on)
        .w(px(28.))
        .h(px(28.))
        .items_center()
        .justify_center()
        .rounded(px(RADIUS))
        .bg(sheet)
        .opacity(if on { 1. } else { 0. })
        .transition()
        .duration(tween(INSTANT))
        .hover(|s| s.opacity(1.))
        .active(|s| s.scale(press_scale(0.95)))
        .focus(|s| s.opacity(1.).outline(px(2.)).outline_color(accent))
        .on_press(move || app.save(file::update_favorite::file_set_favorite(app.db(), id, !on)))
        .child(
            view()
                .w(px(28.))
                .h(px(28.))
                .items_center()
                .justify_center()
                .tooltip(hint(name))
                .child(favorite_mark(on)),
        )
}

/// Where the file is kept, as a small mark on its slip.
fn storage_mark(file: &FileListItem) -> View {
    let remote = file.storage_kind.as_deref() == Some("remote");
    view()
        .w(px(24.))
        .h(px(24.))
        .items_center()
        .justify_center()
        .rounded(px(RADIUS_SM))
        .bg(sheet)
        .tooltip(hint(if remote {
            "Remote storage"
        } else {
            "Local storage"
        }))
        .child(icon(
            if remote { Icon::Cloud } else { Icon::Drive },
            secondary,
        ))
}

/// The commands about one file, for its context menu.
fn file_menu(app: App, file: &FileListItem, selection: Selection, about: View) -> impl IntoElement {
    let id = file.id;
    let on = file.is_favorite;
    let path = file.path.clone();
    context_menu(about)
        .child(menu_item("View details", Some(Icon::Open), move || {
            app.page.set(Page::File(id))
        }))
        .child(menu_item(
            if on { "Unfavorite" } else { "Favorite" },
            Some(Icon::Star),
            move || app.save(file::update_favorite::file_set_favorite(app.db(), id, !on)),
        ))
        .child(menu_item("Copy path", Some(Icon::Copy), move || {
            set_clipboard(path.clone());
            app.message.set("Path copied".into());
        }))
        .child(menu_separator())
        .child(menu_item(
            if selection.ids.with_untracked(|s| s.contains(&id)) {
                "Deselect"
            } else {
                "Select"
            },
            Some(Icon::Select),
            move || {
                selection.active.set(true);
                selection.toggle(id);
            },
        ))
}

/// One file on the grid: its cover (or typeset slip) with the favourite
/// and storage marks, the title and the authors under it.
fn slip(app: App, item: &Shelved, selection: Selection) -> AnyElement {
    let file = &item.file;
    let id = file.id;
    let title = file.display_name.clone();
    let authors = authors_of(file);
    let cover = view()
        .w_full()
        .child(detail::card_face(app, file, move || {
            press_file(app, id, &title, selection)
        }))
        .child(view().absolute().top(px(8.)).left(px(8.)).child(favorite(
            app,
            id,
            file.is_favorite,
        )))
        .child(
            view()
                .absolute()
                .bottom(px(8.))
                .left(px(8.))
                .pointer_events(PointerEvents::None)
                .child(storage_mark(file)),
        )
        .child(
            view()
                .absolute()
                .top(px(12.))
                .right(px(12.))
                .child(selection.control(id, &file.display_name)),
        );
    let mut body = view().w_full().min_w_0().gap(px(4.)).child(cover).child(
        label(file.display_name.clone())
            .font_weight(FontWeight::MEDIUM)
            .number_of_lines(2)
            .pt(px(6.)),
    );
    if !authors.is_empty() {
        body = body.child(note(authors).text_size(px(13.)).number_of_lines(1));
    }
    for (name, value) in &item.fields {
        body = body.child(caption(format!("{name} · {value}")).number_of_lines(1));
    }
    file_menu(app, file, selection, body).into_element()
}
