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
            .child(button("Name", move || sort.set("name".into())))
            .child(button("Created", move || sort.set("created".into())))
            .child(button("Updated", move || sort.set("updated".into())))
            .child(button("Reverse sort", move || desc.update(|v| *v = !*v)))
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
            .child(button("Grid / list", move || grid.update(|v| *v = !*v)))
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
    stack()
        .flex_1()
        .min_h_0()
        .child(title(title_text))
        .child(search::bar(app, query, chips))
        .child(
            wrap()
                .items_start()
                .child(button("Import files", move || app.page.set(Page::Import)))
                .child(button("View options", move || options.update(|v| *v = !*v)))
                .child(crate::filters::editor(app, chips))
                .child(selection.bar(app)),
        )
        .child(when(move || options.get(), display_tools))
        .on_file_drop(move |e: &FileDropEvent| imports::drop_urls(app, &e.uris))
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
                        scroll().flex_1().min_h_0().child(body).into_element()
                    },
                );
            }
            let pos = offset.get();
            let key = sort.get();
            let reverse = desc.get();
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
                    let mut pager = row()
                        .child(label(format!(
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
                    stack()
                        .flex_1()
                        .min_h_0()
                        .child(pager)
                        .child(scroll().flex_1().min_h_0().child(file_rows(
                            app,
                            &page.files,
                            is_grid,
                            selection,
                        )))
                        .into_element()
                },
            )
        }))
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
    let mut list = if grid { wrap().items_start() } else { stack() };
    if files.is_empty() {
        return label("No matching files").into_element();
    }
    let extra = signal(std::rc::Rc::new(std::collections::HashMap::<
        i64,
        Vec<(String, String)>,
    >::new()));
    list = list.child(app.load(
        file::listing::card_metadata(app.db(), files.iter().map(|f| f.id).collect()),
        move |values| {
            extra.set(std::rc::Rc::new(values.clone()));
            view().hidden().into_element()
        },
    ));
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
                .w(px(218.))
                .p_3()
                .bg(surface)
                .rounded_md()
                .child(detail::card_face(app, file))
        } else {
            row().p_2()
        };
        card = card.child(selection.control(id));
        if !grid {
            card = card.child(button(&file.display_name, move || {
                app.page.set(Page::File(id))
            }));
        }
        if !authors.is_empty() {
            card = card.child(label(authors));
        }
        card = card.child(dynamic(move || {
            let values = extra.get();
            let mut fields = stack().gap_1();
            if let Some(items) = values.get(&id) {
                for (name, value) in items {
                    fields = fields.child(label(format!("{name} · {value}")));
                }
            }
            fields
        }));
        card = card
            .child(label(format!(
                "{} · {}",
                file.storage_kind.as_deref().unwrap_or("local"),
                file.file_status
            )))
            .child(button(
                if favorite { "Unfavorite" } else { "Favorite" },
                move || {
                    app.save(file::update_favorite::file_set_favorite(
                        app.db(),
                        id,
                        !favorite,
                    ))
                },
            ));
        list = list.child(card);
    }
    list.into_element()
}
