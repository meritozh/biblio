//! The library: a collection's header (search, a random pick, Add), its
//! toolbar (grouping, sort, favourites, filters, layout, selection) and the
//! shelf of results. The shelf is virtualized: only the slips or rows in
//! view exist, and the next files load as it nears the last.
use crate::{
    app::{App, Page},
    detail, filters, imports,
    paged::{self, Paged},
    search,
    selection::Selection,
    ui::{self, *},
};
use biblio_core::commands::{
    Category, FileListItem, category,
    file::{self, FilterCondition, collections::Collection},
};
use nsh_core::prelude::*;
use serde_json::Value;
use shelf::{file_grid, file_table, group_shelf, lucky_picks};
use std::rc::Rc;
mod shelf;

/// Files loaded at a time.
const CHUNK: i64 = 60;
/// The narrowest a slip gets before the grid drops a column.
const SLIP_MIN: f32 = 156.;
const GRID_GAP_X: f32 = 28.;
const GRID_GAP_Y: f32 = 28.;

/// A file on the shelf with the schema fields its card shows.
pub struct Shelved {
    pub file: FileListItem,
    pub fields: Vec<(String, String)>,
}

/// A group the grouped view opened: its title and its files.
#[derive(Clone, PartialEq)]
pub struct Opened {
    title: String,
    ids: Rc<Vec<i64>>,
}

#[derive(Clone, Copy)]
pub struct BrowseState {
    category: Stored<Option<Option<i64>>>,
    query: Signal<String>,
    chips: Signal<Vec<search::Chip>>,
    sort: Signal<String>,
    desc: Signal<bool>,
    mode: Signal<String>,
    /// The group open in a grouped view.
    opened: Signal<Option<Opened>>,
    selection: Selection,
    grid: Signal<bool>,
    /// The first file in view, so the shelf comes back to it from a file.
    anchor: Stored<usize>,
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
            opened: signal(None),
            selection: Selection::new(),
            grid: signal(true),
            anchor: stored(0),
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

/// What the shelf shows.
#[derive(Clone, PartialEq)]
enum Shelf {
    /// Files answering the search, filters and sort.
    Files,
    /// The groups of a grouped view.
    Groups(String),
    /// The files of one group.
    Group(Opened),
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
        opened,
        selection,
        grid,
        anchor,
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
        opened.set(None);
        selection.clear();
        anchor.with(|a| *a = 0);
    }
    let previous = stored((
        query.get_untracked(),
        chips.get_untracked(),
        sort.get_untracked(),
        desc.get_untracked(),
        mode.get_untracked(),
        opened.get_untracked(),
    ));
    effect(move || {
        let next = (
            query.get(),
            chips.get(),
            sort.get(),
            desc.get(),
            mode.get(),
            opened.get(),
        );
        previous.with(|last| {
            if *last != next {
                if last.4 != next.4 {
                    opened.set(None);
                }
                *last = next;
                anchor.with(|a| *a = 0);
                selection.clear();
            }
        });
    });
    let shelf = computed(move || {
        let searching = !query.with(|q| q.trim().is_empty()) || !chips.with(Vec::is_empty);
        let group = mode.get();
        match opened.get() {
            _ if group == "flat" || searching => Shelf::Files,
            Some(open) => Shelf::Group(open),
            None => Shelf::Groups(group),
        }
    });
    let files = Paged::<Shelved>::new(app, CHUNK, anchor.with(|a| *a) as i64, move || {
        app.revision.get();
        let db = app.db();
        match shelf.get() {
            Shelf::Group(open) => {
                let key = (
                    open.title.clone(),
                    String::new(),
                    vec![],
                    String::new(),
                    false,
                );
                let ids = open.ids.clone();
                let fetch = paged::fetcher(move |offset, limit| {
                    let db = db.clone();
                    let start = (offset as usize).min(ids.len());
                    let end = (start + limit as usize).min(ids.len());
                    let page = ids[start..end].to_vec();
                    let total = ids.len() as i64;
                    async move {
                        let found = file::listing::file_list_by_ids(db.clone(), page).await?;
                        Ok((with_fields(db, found).await?, total))
                    }
                });
                (key, fetch)
            }
            // The groups load on their own; this list stays empty.
            Shelf::Groups(group) => {
                let key = (
                    group,
                    String::from("\0groups"),
                    vec![],
                    String::new(),
                    false,
                );
                (key, paged::fetcher(|_, _| async { Ok((Vec::new(), 0)) }))
            }
            Shelf::Files => {
                let q = query.get();
                let filters = chips.get().into_iter().map(|c| c.1).collect::<Vec<_>>();
                let key = (
                    q.clone(),
                    sort.get(),
                    filters.clone(),
                    String::new(),
                    desc.get(),
                );
                let (by, reverse) = (key.1.clone(), key.4);
                let fetch = paged::fetcher(move |offset, limit| {
                    let (db, q, filters, by) = (db.clone(), q.clone(), filters.clone(), by.clone());
                    async move {
                        let page = file::listing::file_list(
                            db.clone(),
                            cat_id,
                            None,
                            None,
                            Some(by),
                            Some(reverse),
                            Some(filters),
                            Some(q),
                            Some(limit as i32),
                            Some(offset as i32),
                        )
                        .await?;
                        Ok((with_fields(db, page.files).await?, page.total))
                    }
                });
                (key, fetch)
            }
        }
    });
    let groups = app.resource(move || {
        app.revision.get();
        let wanted = match shelf.get() {
            Shelf::Groups(group) => Some(group),
            _ => None,
        };
        let (db, schema) = (app.db(), schema.clone());
        async move {
            match wanted {
                Some(group) => file::collections::collection_list(db, group, schema, cat_id).await,
                None => Ok(Vec::new()),
            }
        }
    });
    let lucky = signal(false);
    let count = count_caption(move || match shelf.get() {
        Shelf::Groups(_) => match groups.get().as_deref() {
            Some(Ok(list)) if !list.is_empty() => {
                let n: usize = list.iter().map(|g| g.file_ids.len()).sum();
                format!(
                    "{} in {}",
                    counted(n as i64, "file", "files"),
                    counted(list.len() as i64, "group", "groups")
                )
            }
            _ => String::new(),
        },
        // The empty state says it; a "0 files" caption would repeat it.
        _ => match files.total.get() {
            Some(n) if n > 0 => counted(n, "file", "files"),
            _ => String::new(),
        },
    });
    let header = page_header(
        Icon::Library,
        title_text,
        Some(count.into_element()),
        Some(
            row()
                .gap_2()
                .child(
                    view()
                        .w(move || px(if compact() { 200. } else { 300. }))
                        .child(search::bar(app, query, chips)),
                )
                .child(control(
                    "Lucky",
                    Variant::Ghost,
                    ButtonSize::Md,
                    Some(Icon::Shuffle),
                    move || lucky.set(true),
                ))
                .child(
                    control(
                        "Add",
                        Variant::Primary,
                        ButtonSize::Md,
                        Some(Icon::Plus),
                        move || app.page.set(Page::Import),
                    )
                    .accessibility_label("Add files"),
                )
                .into_element(),
        ),
    );
    let schema_filter = category.map(|c| c.schema_slug.clone());
    let mut tools = wrap().gap(px(6.)).min_w_0().flex_1();
    if cat_id.is_some() {
        tools = tools.child(view_menu(mode));
    }
    tools = tools
        .child(sort_menu(app, sort, schema_filter))
        .child(icon_toggle(
            move || {
                if desc.get() {
                    "Descending order"
                } else {
                    "Ascending order"
                }
            },
            move || {
                if desc.get() {
                    Icon::ArrowDown
                } else {
                    Icon::ArrowUp
                }
            },
            move || false,
            move || desc.update(|v| *v = !*v),
        ))
        .child(icon_toggle(
            move || "Favorites only",
            move || Icon::Star,
            move || chips.with(|c| c.iter().any(is_favorite_filter)),
            move || toggle_favorites(chips),
        ))
        .child(filters::editor(app, chips));
    let toolbar = row()
        .items_start()
        .gap_3()
        .px(inset)
        .pt_4()
        .pb_3()
        .z_index(3)
        .child(tools)
        .child(
            row()
                .gap_2()
                .flex_shrink_0()
                .child(segmented(
                    "Layout",
                    vec![
                        Segment::icon("grid", "Grid", Icon::Grid),
                        Segment::icon("list", "List", Icon::List),
                    ],
                    move || if grid.get() { "grid" } else { "list" },
                    move |v| grid.set(v == "grid"),
                ))
                .child(
                    control("Select", Variant::Ghost, ButtonSize::Md, None, move || {
                        selection.active.update(|v| *v = !*v);
                        selection.clear();
                    })
                    .accessibility_label("Select files")
                    .selected(move || selection.active.get())
                    .bg(move || {
                        if selection.active.get() {
                            tint()
                        } else {
                            clear()
                        }
                    }),
                ),
        );
    let breadcrumb = dynamic(move || match shelf.get() {
        Shelf::Group(open) => row()
            .gap_2()
            .px(inset)
            .pb_3()
            .child(control(
                "All groups",
                Variant::Ghost,
                ButtonSize::Sm,
                Some(Icon::Back),
                move || opened.set(None),
            ))
            .child(caption("/"))
            .child(
                label(open.title)
                    .font_weight(FontWeight::MEDIUM)
                    .number_of_lines(1),
            )
            .into_element(),
        _ => view().hidden().into_element(),
    });
    let filtered = move || !query.with(|q| q.trim().is_empty()) || !chips.with(Vec::is_empty);
    // Rebuilt only when the kind of content changes; the list or table
    // inside follows the rows as they load.
    let state = computed(move || {
        let kind = match shelf.get() {
            Shelf::Groups(_) => match groups.get().as_deref() {
                None => 0,
                Some(Ok(list)) if list.is_empty() => 1,
                Some(Err(_)) => 3,
                Some(Ok(_)) => 4,
            },
            _ => match files.total.get() {
                None => 0,
                Some(0) => 1,
                Some(_) => 2,
            },
        };
        (kind, grid.get(), matches!(shelf.get(), Shelf::Groups(_)))
    });
    let results = dynamic(move || {
        let (kind, is_grid, _) = state.get();
        untracked(|| match kind {
            0 => view().px(inset).child(caption("Loading…")).into_element(),
            1 => shelf_empty(filtered(), cat_id.is_some()),
            3 => match groups.get().as_deref() {
                Some(Err(e)) => view().px(inset).child(label(e.clone())).into_element(),
                _ => view().into_element(),
            },
            4 => group_shelf(app, groups, opened, is_grid),
            _ if is_grid => file_grid(app, files, selection, anchor),
            _ => file_table(app, files, selection),
        })
    });
    let shown = move || {
        files
            .rows
            .with(|r| r.iter().map(|s| s.file.id).collect::<Vec<_>>())
    };
    let picks = modal(
        lucky,
        || "Lucky picks".to_owned(),
        720.,
        move || lucky_picks(app, cat_id, query, chips, lucky),
    );
    ui::page(
        header,
        view()
            .flex_1()
            .min_h_0()
            .child(toolbar)
            .child(view().px(inset).child(search::chips(chips)))
            .child(view().px(inset).child(selection.bar(app, shown)))
            .child(breadcrumb)
            .child(view().flex_1().min_h_0().child(results))
            .child(picks),
    )
    .on_file_drop(move |e: &FileDropEvent| imports::drop_urls(app, &e.uris))
    .into_element()
}

/// The cards' schema fields of `files`, with them.
async fn with_fields(
    db: biblio_core::Library,
    files: Vec<FileListItem>,
) -> Result<Vec<Shelved>, String> {
    let mut values = file::listing::card_metadata(db, files.iter().map(|f| f.id).collect()).await?;
    Ok(files
        .into_iter()
        .map(|file| Shelved {
            fields: values.remove(&file.id).unwrap_or_default(),
            file,
        })
        .collect())
}

fn is_favorite_filter(c: &search::Chip) -> bool {
    c.1.field == "favorite" && c.1.op == "is" && c.1.value.as_ref() == Some(&Value::Bool(true))
}

/// Shows only favourites, or every file again.
fn toggle_favorites(chips: Signal<Vec<search::Chip>>) {
    chips.update(|list| {
        if list.iter().any(is_favorite_filter) {
            list.retain(|c| !is_favorite_filter(c));
        } else {
            list.push(search::Chip(
                "Favorited".into(),
                FilterCondition {
                    field: "favorite".into(),
                    op: "is".into(),
                    value: Some(Value::Bool(true)),
                    ..Default::default()
                },
            ));
        }
    });
}

/// A square toolbar button that stays pressed while `on`; its name and
/// icon follow its state.
fn icon_toggle(
    name: impl Fn() -> &'static str + 'static,
    glyph: impl Fn() -> Icon + 'static,
    on: impl Fn() -> bool + Copy + 'static,
    action: impl Fn() + 'static,
) -> AnyElement {
    let action = Rc::new(action);
    dynamic(move || {
        let action = action.clone();
        icon_button(name(), glyph(), None, move || action())
            .selected(on)
            .bg(move || if on() { tint() } else { clear() })
            .into_element()
    })
    .into_element()
}

fn view_menu(mode: Signal<String>) -> impl IntoElement {
    let arrange = move |name: &'static str, key: &'static str| {
        menu_radio(
            name,
            move || mode.with(|m| m == key),
            move || mode.set(key.into()),
        )
    };
    menu(tool_trigger("View", Some(Icon::Layers), move || {
        match mode.get().as_str() {
            "author" => "By author",
            "name_prefix" => "By title",
            _ => "All",
        }
        .to_owned()
    }))
    .child(arrange("All", "flat"))
    .child(arrange("By author", "author"))
    .child(arrange("By title", "name_prefix"))
}

fn sort_menu(app: App, sort: Signal<String>, schema_filter: Option<String>) -> impl IntoElement {
    let by = move |name: &'static str, key: &'static str| {
        menu_radio(
            name,
            move || sort.with(|s| s == key),
            move || sort.set(key.into()),
        )
    };
    let schema_filter = stored(schema_filter);
    // Names of the schema fields one can sort by, for the trigger.
    let labels = signal(Vec::<(String, String)>::new());
    let fields = app.load(
        biblio_core::commands::schemas::schema_list(app.db()),
        move |schemas| {
            let mut extra = view().gap(px(1.));
            let mut seen = std::collections::HashSet::new();
            let mut names = Vec::new();
            for s in schemas {
                if schema_filter.with(|f| f.as_ref().is_some_and(|slug| slug != &s.schema.id)) {
                    continue;
                }
                for f in &s.fields {
                    if f.sortable && f.semantic.is_none() && seen.insert(f.field_key.clone()) {
                        let key = stored(format!("field:{}", f.field_key));
                        names.push((key.with(|k| k.clone()), f.label.clone()));
                        extra = extra.child(menu_radio(
                            f.label.clone(),
                            move || key.with(|k| sort.with(|s| s == k)),
                            move || sort.set(key.with(|k| k.clone())),
                        ));
                    }
                }
            }
            labels.set(names);
            extra.into_element()
        },
    );
    menu(tool_trigger("Sort", None, move || {
        let key = sort.get();
        match key.as_str() {
            "name" => "Name".to_owned(),
            "created" => "Date added".to_owned(),
            "updated" => "Date modified".to_owned(),
            other => labels.with(|l| {
                l.iter()
                    .find(|(k, _)| k == other)
                    .map(|(_, name)| name.clone())
                    .unwrap_or_else(|| other.trim_start_matches("field:").to_owned())
            }),
        }
    }))
    .child(menu_heading("Sort by"))
    .child(by("Name", "name"))
    .child(by("Date added", "created"))
    .child(by("Date modified", "updated"))
    .child(fields)
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
            "Drop files here or choose Add to start your collection.",
        )
    };
    empty(heading, help).into_element()
}
