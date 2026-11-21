use crate::artwork::{favorite_mark, index_mark};
use crate::{
    app::{App, Page},
    detail, imports, search,
    selection::Selection,
    ui::*,
};
use biblio_core::commands::{
    Category, category,
    file::{self, FilterCondition},
};
use nsh_core::prelude::*;
use serde_json::Value;

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
    let options = signal(false);
    let schema_filter = category.map(|c| c.schema_slug.clone());
    let display_tools = move || {
        let schema_filter = schema_filter.clone();
        let mut tools = wrap()
            .child(choice(
                "Name",
                move || sort.with(|s| s == "name"),
                move || sort.set("name".into()),
            ))
            .child(choice(
                "Created",
                move || sort.with(|s| s == "created"),
                move || sort.set("created".into()),
            ))
            .child(choice(
                "Updated",
                move || sort.with(|s| s == "updated"),
                move || sort.set("updated".into()),
            ))
            .child(choice(
                "Reverse sort",
                move || desc.get(),
                move || desc.update(|v| *v = !*v),
            ))
            .child(app.load(
                biblio_core::commands::schemas::schema_list(app.db()),
                move |schemas| {
                    let mut extra = wrap();
                    let mut seen = std::collections::HashSet::new();
                    for s in schemas {
                        if schema_filter
                            .as_ref()
                            .is_some_and(|slug| slug != &s.schema.id)
                        {
                            continue;
                        }
                        for f in &s.fields {
                            if f.sortable
                                && f.semantic.is_none()
                                && seen.insert(f.field_key.clone())
                            {
                                let key = format!("field:{}", f.field_key);
                                extra =
                                    extra.child(button(&f.label, move || sort.set(key.clone())));
                            }
                        }
                    }
                    extra.into_element()
                },
            ))
            .child(choice("Grid", move || grid.get(), move || grid.set(true)))
            .child(choice("List", move || !grid.get(), move || grid.set(false)))
            .child(button("Pick at random", move || {
                lucky.set(true);
                app.refresh();
            }))
            .child(button("Show files", move || {
                lucky.set(false);
                mode.set("flat".into());
            }));
        if cat_id.is_some() {
            for (name, key) in [
                ("Group by author", "author"),
                ("Group by title", "name_prefix"),
            ] {
                tools = tools.child(button(name, move || {
                    lucky.set(false);
                    mode.set(key.into());
                }));
            }
        }
        tools
    };
    let tools_and_results = stack()
        .child(search::bar(app, query, chips))
        .child(
            wrap()
                .items_start()
                .child(choice(
                    "View options",
                    move || options.get(),
                    move || options.update(|v| *v = !*v),
                ))
                .child(crate::filters::editor(app, chips))
                .child(selection.bar(app)),
        )
        .child(when(move || options.get(), display_tools))
        .child(dynamic(move || {
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
                        if groups.is_empty() {
                            return empty_shelf(false, cat_id.is_some());
                        }
                        let mut body = stack();
                        for group in groups {
                            let ids = group.file_ids.clone();
                            let expanded = signal(false);
                            body = body
                                .child(button(
                                    format!("{} ({})", group.title, ids.len()),
                                    move || expanded.update(|b| *b = !*b),
                                ))
                                .child(when(
                                    move || expanded.get(),
                                    move || group_page(app, ids.clone(), is_grid, selection),
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
                    let ids = page.files.iter().map(|f| f.id).collect::<Vec<_>>();
                    let mut pager = wrap()
                        .pt_3()
                        .pb_2()
                        .border_b_1()
                        .border_color(rule)
                        .child(note(format!(
                            "{} {}",
                            page.total,
                            if page.total == 1 { "file" } else { "files" }
                        )))
                        .child(when(
                            move || selection.active.get(),
                            move || {
                                let ids = ids.clone();
                                button("Select this page", move || {
                                    selection.ids.update(|s| s.extend(&ids))
                                })
                            },
                        ));
                    if pos > 0 {
                        pager = pager.child(button("Previous", move || {
                            offset.set((pos - PAGE_SIZE).max(0))
                        }));
                    }
                    if i64::from(pos + PAGE_SIZE) < page.total {
                        pager = pager.child(button("Next", move || offset.set(pos + PAGE_SIZE)));
                    }
                    let results = if page.total == 0 {
                        empty_shelf(filtered, cat_id.is_some())
                    } else {
                        file_rows(app, &page.files, is_grid, selection)
                    };
                    stack().child(pager).child(results).into_element()
                },
            )
        }));
    stack()
        .flex_1()
        .min_h_0()
        .on_file_drop(move |e: &FileDropEvent| imports::drop_urls(app, &e.uris))
        .child(
            heading_bar()
                .child(title(title_text))
                .child(import_button(move || app.page.set(Page::Import))),
        )
        .child(scroll().flex_1().min_h_0().child(tools_and_results))
        .into_element()
}

fn empty_shelf(filtered: bool, category: bool) -> AnyElement {
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
    stack()
        .w_full()
        .items_center()
        .justify_center()
        .gap_2()
        .py_6()
        .px_4()
        .min_h(move || {
            px(if window().width < px(760.) {
                204.
            } else {
                284.
            })
        })
        .bg(sheet)
        .border_1()
        .border_color(rule)
        .rounded_lg()
        .child(index_mark(false))
        .child(subheading(heading).text_center())
        .child(note(help).text_center())
        .into_element()
}

fn group_page(app: App, ids: Vec<i64>, grid: bool, selection: Selection) -> AnyElement {
    let offset = signal(0usize);
    dynamic(move || {
        let start = offset.get();
        let end = (start + PAGE_SIZE as usize).min(ids.len());
        let mut pager = row().child(label(format!("{}–{} of {}", start + 1, end, ids.len())));
        if start > 0 {
            pager = pager.child(button("Previous in group", move || {
                offset.set(start.saturating_sub(PAGE_SIZE as usize))
            }));
        }
        if end < ids.len() {
            pager = pager.child(button("Next in group", move || offset.set(end)));
        }
        stack().child(pager).child(app.load(
            file::listing::file_list_by_ids(app.db(), ids[start..end].to_vec()),
            move |files| file_rows(app, files, grid, selection),
        ))
    })
    .into_element()
}

fn file_rows(
    app: App,
    files: &[biblio_core::commands::FileListItem],
    grid: bool,
    selection: Selection,
) -> AnyElement {
    let mut list = if grid {
        view()
            .grid()
            .grid_template_columns("repeat(auto-fill, minmax(200px, 1fr))")
            .gap_3()
    } else {
        stack().gap_0()
    };
    if files.is_empty() {
        return label("No matching files").into_element();
    }
    let extra = signal(std::rc::Rc::new(std::collections::HashMap::<
        i64,
        Vec<(String, String)>,
    >::new()));
    let metadata = app.load(
        file::listing::card_metadata(app.db(), files.iter().map(|f| f.id).collect()),
        move |values| {
            extra.set(std::rc::Rc::new(values.clone()));
            view().hidden().into_element()
        },
    );
    for file in files {
        let id = file.id;
        let favorite = file.is_favorite;
        let authors = file
            .authors
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let mut card = if grid {
            stack()
                .w_full()
                .gap_2()
                .p_3()
                .bg(sheet)
                .border_1()
                .border_color(rule)
                .rounded_lg()
                .child(detail::card_face(app, file))
        } else {
            wrap().p_2().border_b_1().border_color(rule)
        };
        card = card.child(selection.control(id));
        if !grid {
            card = card.child(button(&file.display_name, move || {
                app.page.set(Page::File(id))
            }));
        }
        if !authors.is_empty() {
            card = card.child(note(authors));
        }
        card = card.child(dynamic(move || {
            let values = extra.get();
            let mut fields = stack().gap_1();
            if let Some(items) = values.get(&id).filter(|v| !v.is_empty()) {
                for (name, value) in items {
                    fields = fields.child(note(format!("{name} · {value}")));
                }
                fields
            } else {
                view().hidden()
            }
        }));
        let caption = if favorite { "Unfavorite" } else { "Favorite" };
        let toggle = nsh_base::button()
            .accessibility_label(caption)
            .selected(favorite)
            .w(px(36.))
            .h(px(36.))
            .items_center()
            .justify_center()
            .rounded_md()
            .hover(|s| s.bg(paper))
            .active(|s| s.opacity(0.7))
            .focus(|s| s.outline(px(2.)).outline_color(accent))
            .on_press(move || {
                app.save(file::update_favorite::file_set_favorite(
                    app.db(),
                    id,
                    !favorite,
                ))
            })
            .child(
                view()
                    .tooltip(
                        view()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(cloth)
                            .child(label(caption).text_color(rail_ink)),
                    )
                    .child(favorite_mark(favorite)),
            );
        card = card.child(
            row()
                .justify_between()
                .child(note(format!(
                    "{} · {}",
                    file.storage_kind.as_deref().unwrap_or("local"),
                    file.file_status
                )))
                .child(toggle),
        );
        list = list.child(card);
    }
    stack().gap_0().child(metadata).child(list).into_element()
}
