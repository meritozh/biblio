//! The library: a collection's header, its toolbar (search, filters, sort,
//! layout, selection, arrangement) and the shelf of results.
use crate::{
    app::{App, Page},
    detail, filters, imports, search,
    selection::Selection,
    ui::*,
};
use biblio_core::commands::{
    Category, FileListItem, category,
    file::{self, FilterCondition},
};
use nsh_core::prelude::*;
use serde_json::Value;
use std::{collections::HashMap, rc::Rc};

const PAGE_SIZE: i32 = 30;

#[derive(Clone, Copy)]
pub struct BrowseState {
    category: Stored<Option<Option<i64>>>,
    query: Signal<String>,
    chips: Signal<Vec<search::Chip>>,
    sort: Signal<String>,
    desc: Signal<bool>,
    mode: Signal<String>,
    selection: Selection,
    offset: Signal<i32>,
    grid: Signal<bool>,
    lucky: Signal<bool>,
    /// The size of the result set shown, for the header's count.
    total: Signal<Option<i64>>,
    /// The files on the page shown, for "Select this page".
    page_ids: Signal<Vec<i64>>,
}
impl BrowseState {
    pub fn new() -> Self {
        Self {
            category: stored(None),
            query: signal(String::new()),
            chips: signal(vec![]),
            sort: signal("name".into()),
            desc: signal(false),
            mode: signal("flat".into()),
            selection: Selection::new(),
            offset: signal(0),
            grid: signal(true),
            lucky: signal(false),
            total: signal(None),
            page_ids: signal(vec![]),
        }
    }
}
pub fn screen(app: App) -> AnyElement {
    dynamic(move || {
        let category = app.category.get();
        if let Some(id) = category {
            app.load(category::category_get(app.db(), id), move |c| {
                browse(app, Some(c))
            })
        } else {
            untracked(|| browse(app, None))
        }
    })
    .into_element()
}

fn browse(app: App, category: Option<&Category>) -> AnyElement {
    let config = category
        .and_then(|c| c.view_config.as_deref())
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .unwrap_or_default();
    let cat_id = category.map(|c| c.id);
    let schema = category
        .map(|c| c.schema_slug.clone())
        .unwrap_or_else(|| "novel".into());
    let title_text = category.map(|c| c.name.as_str()).unwrap_or("All files");
    let BrowseState {
        category: seen,
        query,
        chips,
        sort,
        desc,
        mode,
        selection,
        offset,
        grid,
        lucky,
        total,
        page_ids,
    } = app.browse;
    if seen.with(|p| *p) != Some(cat_id) {
        seen.with(|p| *p = Some(cat_id));
        query.set(String::new());
        let defaults: Vec<FilterCondition> =
            serde_json::from_value(config["conditions"].clone()).unwrap_or_default();
        chips.set(defaults.into_iter().map(crate::filters::chip).collect());
        sort.set(config["sort"]["by"].as_str().unwrap_or("name").to_owned());
        desc.set(config["sort"]["desc"].as_bool().unwrap_or(false));
        mode.set(config["view_mode"].as_str().unwrap_or("flat").to_owned());
        offset.set(0);
        selection.clear();
        lucky.set(false);
        total.set(None);
    }
    let previous = stored((
        query.get_untracked(),
        chips.get_untracked(),
        sort.get_untracked(),
        desc.get_untracked(),
        mode.get_untracked(),
    ));
    effect(move || {
        let next = (query.get(), chips.get(), sort.get(), desc.get(), mode.get());
        previous.with(|last| {
            if *last != next {
                *last = next;
                offset.set(0);
                selection.clear();
            }
        });
    });
    let schema_filter = category.map(|c| c.schema_slug.clone());
    let count = text(move || match total.get() {
        Some(1) => "1 file".to_owned(),
        Some(n) if n > 1 => format!("{n} files"),
        // The empty state says it; a "0 files" caption would repeat it.
        _ => String::new(),
    })
    .text_size(px(13.))
    .text_color(secondary);
    let header = page_header(
        title_text,
        Some(count.into_element()),
        Some(
            icon_label("Import files", Icon::Plus, move || {
                app.page.set(Page::Import)
            })
            .into_element(),
        ),
    );
    let tools = wrap()
        .gap_2()
        .min_w_0()
        .flex_shrink(1.)
        .child(filters::editor(app, chips))
        .child(sort_menu(app, sort, desc, schema_filter))
        .child(segmented(
            "Layout",
            vec![
                Segment::icon("grid", "Grid", Icon::Grid),
                Segment::icon("list", "List", Icon::List),
            ],
            move || if grid.get() { "grid" } else { "list" },
            move |v| grid.set(v == "grid"),
        ))
        .child(toggle_button(
            "Select files",
            Icon::Select,
            move || selection.active.get(),
            move || {
                selection.active.update(|v| *v = !*v);
                selection.clear();
            },
        ))
        .child(arrange_menu(app, mode, lucky, cat_id.is_some()));
    let toolbar = wrap()
        .items_start()
        .gap_2()
        .pt_4()
        .z_index(2)
        .child(
            view()
                .flex_1()
                .min_w(px(240.))
                .child(search::bar(app, query, chips)),
        )
        .child(tools);
    let results = dynamic(move || {
        app.revision.get();
        let q = query.get();
        let filters = chips.get().into_iter().map(|c| c.1).collect::<Vec<_>>();
        let db = app.db();
        let group = mode.get();
        let random = lucky.get();
        let is_grid = grid.get();
        if group != "flat" && q.is_empty() && filters.is_empty() && !random {
            return app.load(
                file::collections::collection_list(db, group, schema.clone(), cat_id),
                move |groups| {
                    total.set(Some(groups.iter().map(|g| g.file_ids.len() as i64).sum()));
                    page_ids.set(vec![]);
                    if groups.is_empty() {
                        return shelf_empty(false, cat_id.is_some());
                    }
                    let mut body = view().gap(px(2.)).pb_6();
                    for group in groups {
                        let ids = group.file_ids.clone();
                        let count = format!("{}", ids.len());
                        let expanded = signal(false);
                        body = body.child(reveal(
                            group.title.clone(),
                            Some(count),
                            expanded,
                            move || {
                                view()
                                    .pt_2()
                                    .pb_4()
                                    .child(group_page(app, ids.clone(), is_grid, selection))
                                    .into_element()
                            },
                        ));
                    }
                    body.into_element()
                },
            );
        }
        let pos = offset.get();
        let key = sort.get();
        let reverse = desc.get();
        let filtered = !q.trim().is_empty() || !filters.is_empty();
        app.load(
            async move {
                if random {
                    let files = file::search_lucky_status::file_lucky(
                        db,
                        cat_id,
                        Some(q),
                        Some(filters),
                        Some(3),
                    )
                    .await?;
                    return Ok(file::listing::FileListResponse {
                        total: files.len() as i64,
                        files,
                    });
                }
                file::listing::file_list(
                    db,
                    cat_id,
                    None,
                    None,
                    Some(key),
                    Some(reverse),
                    Some(filters),
                    Some(q),
                    Some(PAGE_SIZE),
                    Some(pos),
                )
                .await
            },
            move |page| {
                total.set(Some(page.total));
                page_ids.set(page.files.iter().map(|f| f.id).collect());
                if page.total == 0 {
                    return shelf_empty(filtered, cat_id.is_some());
                }
                let shown = page.files.len() as i64;
                let mut body =
                    view()
                        .gap_4()
                        .pb_6()
                        .child(file_rows(app, &page.files, is_grid, selection));
                if page.total > i64::from(PAGE_SIZE) {
                    body = body.child(pager(
                        format!("{}–{} of {}", pos + 1, i64::from(pos) + shown, page.total),
                        (pos > 0).then_some(move || offset.set((pos - PAGE_SIZE).max(0))),
                        (i64::from(pos + PAGE_SIZE) < page.total)
                            .then_some(move || offset.set(pos + PAGE_SIZE)),
                        ("Previous", "Next"),
                    ));
                }
                body.into_element()
            },
        )
    });
    stack()
        .flex_1()
        .min_h_0()
        .gap_3()
        .on_file_drop(move |e: &FileDropEvent| imports::drop_urls(app, &e.uris))
        .child(header)
        .child(toolbar)
        .child(search::chips(chips))
        .child(selection.bar(app, page_ids))
        .child(
            scroll()
                .flex_1()
                .min_h_0()
                .child(view().pt_2().child(results)),
        )
        .into_element()
}

fn sort_menu(
    app: App,
    sort: Signal<String>,
    desc: Signal<bool>,
    schema_filter: Option<String>,
) -> impl IntoElement {
    let by = move |name: &'static str, key: &'static str| {
        menu_radio(
            name,
            move || sort.with(|s| s == key),
            move || sort.set(key.into()),
        )
    };
    let schema_filter = stored(schema_filter);
    menu(trigger("Sort", Some(Icon::Sort), true, true))
        .child(menu_heading("Sort by"))
        .child(by("Name", "name"))
        .child(by("Date added", "created"))
        .child(by("Date modified", "updated"))
        .child(app.load(
            biblio_core::commands::schemas::schema_list(app.db()),
            move |schemas| {
                let mut extra = view().gap(px(1.));
                let mut seen = std::collections::HashSet::new();
                for s in schemas {
                    if schema_filter.with(|f| f.as_ref().is_some_and(|slug| slug != &s.schema.id)) {
                        continue;
                    }
                    for f in &s.fields {
                        if f.sortable && f.semantic.is_none() && seen.insert(f.field_key.clone()) {
                            let key = stored(format!("field:{}", f.field_key));
                            extra = extra.child(menu_radio(
                                f.label.clone(),
                                move || key.with(|k| sort.with(|s| s == k)),
                                move || sort.set(key.with(|k| k.clone())),
                            ));
                        }
                    }
                }
                extra.into_element()
            },
        ))
        .child(menu_separator())
        .child(menu_check(
            "Reverse order",
            move || desc.get(),
            move || desc.update(|v| *v = !*v),
        ))
}

fn arrange_menu(
    app: App,
    mode: Signal<String>,
    lucky: Signal<bool>,
    in_category: bool,
) -> impl IntoElement {
    let arrange = move |name: &'static str, key: &'static str| {
        menu_radio(
            name,
            move || !lucky.get() && mode.with(|m| m == key),
            move || {
                lucky.set(false);
                mode.set(key.into());
            },
        )
    };
    let mut m = menu(trigger("View options", Some(Icon::More), false, false))
        .child(menu_heading("Show"))
        .child(arrange("All files", "flat"));
    if in_category {
        m = m
            .child(arrange("Group by author", "author"))
            .child(arrange("Group by title", "name_prefix"));
    }
    m.child(menu_separator()).child(menu_item(
        "Pick at random",
        Some(Icon::Shuffle),
        move || {
            lucky.set(true);
            app.refresh();
        },
    ))
}

/// "31–60 of 74" with Previous and Next at the foot of a result list.
fn pager(
    range: String,
    previous: Option<impl Fn() + 'static>,
    next: Option<impl Fn() + 'static>,
    names: (&str, &str),
) -> View {
    let mut buttons = row().gap_1();
    let has_previous = previous.is_some();
    let has_next = next.is_some();
    buttons = buttons.child(
        icon_button(names.0, Icon::Back, None, move || {
            if let Some(f) = &previous {
                f()
            }
        })
        .disabled(!has_previous),
    );
    buttons = buttons.child(
        icon_button(names.1, Icon::Forward, None, move || {
            if let Some(f) = &next {
                f()
            }
        })
        .disabled(!has_next),
    );
    row()
        .justify_between()
        .pt_3()
        .border_t_1()
        .border_color(rule)
        .child(caption(range))
        .child(buttons)
}

fn shelf_empty(filtered: bool, category: bool) -> AnyElement {
    let (heading, help) = if filtered {
        (
            "No matching files",
            "Try another search or remove a filter.",
        )
    } else if category {
        (
            "This category is empty",
            "Drop files here to import them into this category.",
        )
    } else {
        (
            "Your library is empty",
            "Drop files here or choose Import files to start your collection.",
        )
    };
    empty(heading, help).into_element()
}

fn group_page(app: App, ids: Vec<i64>, grid: bool, selection: Selection) -> AnyElement {
    let offset = signal(0usize);
    dynamic(move || {
        let start = offset.get();
        let end = (start + PAGE_SIZE as usize).min(ids.len());
        let mut body = view().gap_4().child(app.load(
            file::listing::file_list_by_ids(app.db(), ids[start..end].to_vec()),
            move |files| file_rows(app, files, grid, selection),
        ));
        if ids.len() > PAGE_SIZE as usize {
            body = body.child(pager(
                format!("{}–{} of {}", start + 1, end, ids.len()),
                (start > 0).then_some(move || offset.set(start.saturating_sub(PAGE_SIZE as usize))),
                (end < ids.len()).then_some(move || offset.set(end)),
                ("Previous in group", "Next in group"),
            ));
        }
        body
    })
    .into_element()
}

type CardFields = Rc<HashMap<i64, Vec<(String, String)>>>;

fn file_rows(app: App, files: &[FileListItem], grid: bool, selection: Selection) -> AnyElement {
    if files.is_empty() {
        return empty(
            "No matching files",
            "Try another search or remove a filter.",
        )
        .into_element();
    }
    let extra = signal(CardFields::default());
    let metadata = app.load(
        file::listing::card_metadata(app.db(), files.iter().map(|f| f.id).collect()),
        move |values| {
            extra.set(Rc::new(values.clone()));
            view().hidden().into_element()
        },
    );
    let list = if grid {
        let mut shelf = view()
            .grid()
            .grid_template_columns("repeat(auto-fill, minmax(156px, 1fr))")
            .gap_x(px(20.))
            .gap_y(px(28.));
        for file in files {
            shelf = shelf.child(slip(app, file, selection, extra));
        }
        shelf
    } else {
        let mut rows = view().border_t_1().border_color(rule);
        for file in files {
            rows = rows.child(list_row(app, file, selection, extra));
        }
        rows
    };
    view().child(metadata).child(list).into_element()
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

fn favorite(app: App, id: i64, on: bool) -> impl IntoElement {
    let name = if on { "Unfavorite" } else { "Favorite" };
    nsh_base::button()
        .accessibility_label(name)
        .selected(on)
        .w(px(28.))
        .h(px(28.))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .rounded(px(RADIUS))
        .transition()
        .duration(tween(INSTANT))
        .hover(|s| s.bg(wash))
        .active(|s| s.bg(rule).scale(press_scale(0.95)))
        .focus(|s| s.outline(px(2.)).outline_color(accent))
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

fn fields(id: i64, extra: Signal<CardFields>) -> AnyElement {
    dynamic(move || {
        let values = extra.get();
        let mut lines = view().gap(px(2.));
        if let Some(items) = values.get(&id).filter(|v| !v.is_empty()) {
            for (name, value) in items {
                lines = lines.child(caption(format!("{name} · {value}")).number_of_lines(1));
            }
            lines
        } else {
            view().hidden()
        }
    })
    .into_element()
}

/// The commands about one file, for its context menu.
fn file_menu(app: App, file: &FileListItem, selection: Selection, about: View) -> impl IntoElement {
    let id = file.id;
    let on = file.is_favorite;
    let path = file.path.clone();
    context_menu(about)
        .child(menu_item("Open", Some(Icon::Open), move || {
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

fn slip(
    app: App,
    file: &FileListItem,
    selection: Selection,
    extra: Signal<CardFields>,
) -> impl IntoElement {
    let id = file.id;
    let authors = authors_of(file);
    let mut body = view()
        .w_full()
        .min_w_0()
        .gap(px(6.))
        .child(detail::card_face(app, file))
        .child(selection.control(id, &file.display_name));
    if !authors.is_empty() {
        body = body.child(note(authors).text_size(px(13.)).number_of_lines(2));
    }
    body = body.child(fields(id, extra)).child(
        row()
            .justify_between()
            .gap_1()
            .child(caption(status_of(file)).number_of_lines(1).flex_1())
            .child(favorite(app, id, file.is_favorite)),
    );
    file_menu(app, file, selection, body)
}

fn list_row(
    app: App,
    file: &FileListItem,
    selection: Selection,
    extra: Signal<CardFields>,
) -> impl IntoElement {
    let id = file.id;
    let authors = authors_of(file);
    let mut who = view().gap(px(2.)).flex_1().min_w_0().child(
        nsh_base::button()
            .accessibility_label(file.display_name.clone())
            .on_press(move || app.page.set(Page::File(id)))
            .self_start()
            .rounded(px(RADIUS_SM))
            .hover(|s| s.opacity(0.72))
            .focus(|s| s.outline(px(2.)).outline_color(accent))
            .child(
                label(file.display_name.clone())
                    .font_weight(FontWeight::MEDIUM)
                    .number_of_lines(1),
            ),
    );
    if !authors.is_empty() {
        who = who.child(caption(authors).number_of_lines(1));
    }
    who = who.child(fields(id, extra));
    let line = row()
        .gap_3()
        .min_h(px(48.))
        .py_2()
        .px_1()
        .border_b_1()
        .border_color(rule)
        .child(selection.control(id, &file.display_name))
        .child(
            caption(format_of(file))
                .letter_spacing(px(0.6))
                .w(px(44.))
                .flex_shrink_0()
                .number_of_lines(1),
        )
        .child(who)
        .child(
            caption(status_of(file))
                .w(px(128.))
                .flex_shrink_0()
                .number_of_lines(1)
                .style_with(|s| if compact() { s.hidden() } else { s }),
        )
        .child(favorite(app, id, file.is_favorite));
    file_menu(app, file, selection, line)
}
