use crate::{
    app::{App, Page},
    paged::{self, Paged},
    ui::{self, *},
};
use biblio_core::commands::{self as api, author, category, file, prompts, tag};
use nsh_core::prelude::*;
mod names;
mod prompt_page;
mod schemas;
use names::names;
use prompt_page::prompt_list;

pub fn screen(app: App, page: Page) -> AnyElement {
    match page {
        Page::Categories => categories(app),
        Page::Tags => names(app, true),
        Page::Authors => names(app, false),
        Page::Prompts => prompt_list(app),
        Page::Schemas => schemas::screen(app),
        Page::Cleanup => ui::page(
            heading(Icon::Cleanup, "Cleanup"),
            page_scroll(MEASURE, cleanup_tools(app)),
        )
        .into_element(),
        _ => unreachable!(),
    }
}

/// The primary command of a page header: plus icon, caption.
fn create(caption_text: &str, action: impl Fn() + 'static) -> AnyElement {
    control(
        caption_text,
        Variant::Primary,
        ButtonSize::Md,
        Some(Icon::Plus),
        action,
    )
    .into_element()
}

/// A dialog's commands at its foot: Cancel, then the default submit.
fn dialog_actions(open: Signal<bool>, submit: impl IntoElement) -> View {
    row()
        .justify_end()
        .gap_2()
        .pt_2()
        .child(button("Cancel", move || open.set(false)))
        .child(submit)
}

fn categories(app: App) -> AnyElement {
    let data = app.resource(move || {
        app.revision.get();
        let db = app.db();
        async move {
            Ok((
                category::category_list(db.clone()).await?,
                api::schemas::schema_list(db).await?,
            ))
        }
    });
    let editing = signal(None::<i64>);
    let open = signal(false);
    let header = page_header(
        Icon::Categories,
        "Categories",
        Some(
            count_caption(move || match data.get().as_deref() {
                Some(Ok((cats, _))) if !cats.is_empty() => {
                    counted(cats.len() as i64, "category", "categories")
                }
                _ => String::new(),
            })
            .into_element(),
        ),
        Some(create("New category", move || {
            editing.set(None);
            open.set(true);
        })),
    );
    let cols = vec![
        Col::text("name", "Name", Some(180.)),
        Col::muted("description", "Description", None),
        Col::custom("schema", "Schema", Some(140.)),
        Col::mono("folder", "Folder", Some(150.)),
        Col::muted("defaults", "View defaults", Some(150.)),
        Col::command("edit", Icon::Edit),
    ];
    let keys: Vec<&'static str> = cols.iter().map(|c| c.key).collect();
    let shown = dynamic(move || {
        match data.get().as_deref() {
        None => caption("Loading…").into_element(),
        Some(Err(e)) => label(e.clone()).into_element(),
        Some(Ok((cats, _))) if cats.is_empty() => empty(
            "No categories yet",
            "A category is a shelf with its own schema and storage folder. Create one to start importing.",
        )
        .into_element(),
        Some(Ok((cats, schemas))) => {
            let schema_name = |slug: &str| {
                schemas
                    .iter()
                    .find(|s| s.schema.id == slug)
                    .map(|s| s.schema.name.clone())
                    .unwrap_or_else(|| slug.to_owned())
            };
            let n = cats.len();
            let data_rows = nsh_base::DataTableRows::new(n)
                .texts("name", cats.iter().map(|c| c.name.as_str()))
                .texts(
                    "description",
                    cats.iter().map(|c| c.description.clone().unwrap_or_default()),
                )
                .texts("schema", cats.iter().map(|c| schema_name(&c.schema_slug)))
                .texts("folder", cats.iter().map(|c| c.folder_name.clone().unwrap_or_default()))
                .texts("defaults", cats.iter().map(|c| view_defaults(c.view_config.as_deref())))
                .texts("edit", cats.iter().map(|c| format!("Edit {}", c.name)));
            let ids: Vec<i64> = cats.iter().map(|c| c.id).collect();
            let keys = std::rc::Rc::new(keys.clone());
            let open_row = std::rc::Rc::new(move |row: u32| {
                if let Some(&id) = ids.get(row as usize) {
                    editing.set(Some(id));
                    open.set(true);
                }
            });
            let double = open_row.clone();
            table(
                "Categories",
                cols.clone(),
                move || n,
                move || data_rows.clone(),
                |c: &nsh_base::DataTableCell| row().child(badge(c.text.to_string())).into_element(),
                move |t| {
                    t.on_row_press(move |p: &nsh_base::DataTablePress| {
                        if pressed_key(&keys, p) == Some("edit") {
                            open_row(p.row);
                        }
                    })
                    .on_double_press(move |t: &nsh_base::DataTableTarget| {
                        if let Some(row) = t.row {
                            double(row);
                        }
                    })
                },
            )
            .into_element()
        }
    }
    });
    let dialog = modal(
        open,
        move || {
            if editing.get_untracked().is_some() {
                "Edit category"
            } else {
                "New category"
            }
            .to_owned()
        },
        560.,
        move || {
            // Made once the records are here; a later refresh leaves what
            // the user is typing alone.
            let ready = computed(move || data.with(Option::is_some));
            dynamic(move || {
                if !ready.get() {
                    return caption("Loading…").into_element();
                }
                untracked(|| match data.get().as_deref() {
                    Some(Ok((cats, schemas))) => {
                        let cat = editing
                            .get()
                            .and_then(|id| cats.iter().find(|c| c.id == id));
                        category_form(app, cat, schemas_of(schemas).as_slice(), open)
                    }
                    Some(Err(e)) => label(e.clone()).into_element(),
                    None => view().into_element(),
                })
            })
            .into_element()
        },
    );
    ui::page(
        header,
        page_body()
            .child(about(
                "A category is a shelf of files with its own schema and storage folder. Files inherit the schema of their category.",
            ))
            .child(shown)
            .child(dialog),
    )
    .into_element()
}

/// The schema definitions of a schema list.
fn schemas_of(list: &[api::schemas::SchemaDefinition]) -> Vec<api::schemas::SchemaDefinition> {
    list.iter()
        .map(|s| api::schemas::SchemaDefinition {
            schema: s.schema.clone(),
            fields: s.fields.clone(),
            pipeline_steps: s.pipeline_steps.clone(),
        })
        .collect()
}

/// A category's default sort, as its table shows it ("Date added ↓").
fn view_defaults(config: Option<&str>) -> String {
    let value = config
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .unwrap_or_default();
    let by = value["sort"]["by"].as_str().unwrap_or("name");
    let desc = value["sort"]["desc"].as_bool().unwrap_or(false);
    let name = match by {
        "name" => "Name".to_owned(),
        "created" => "Date added".to_owned(),
        "updated" => "Date modified".to_owned(),
        other => other.strip_prefix("field:").unwrap_or(other).to_owned(),
    };
    format!("{name} {}", if desc { "↓" } else { "↑" })
}

fn category_form(
    app: App,
    cat: Option<&api::Category>,
    schemas: &[api::schemas::SchemaDefinition],
    open: Signal<bool>,
) -> AnyElement {
    let id = cat.map(|c| c.id);
    let name = signal(cat.map(|c| c.name.clone()).unwrap_or_default());
    let desc = signal(cat.and_then(|c| c.description.clone()).unwrap_or_default());
    let slug = signal(
        cat.map(|c| c.schema_slug.clone())
            .unwrap_or_else(|| "novel".into()),
    );
    let config = signal(cat.and_then(|c| c.view_config.clone()).unwrap_or_default());
    let mut options = wrap().gap(px(6.));
    for s in schemas {
        let id = s.schema.id.clone();
        let key = stored(id.clone());
        options = options.child(choice(
            s.schema.name.clone(),
            move || key.with(|k| slug.with(|v| v == k)),
            move || slug.set(id.clone()),
        ));
    }
    let advanced = signal(false);
    let submit = primary(
        if id.is_some() {
            "Update category"
        } else {
            "Create category"
        },
        move || {
            let name = name.get_untracked();
            let desc = Some(desc.get_untracked());
            let slug = slug.get_untracked();
            let cfg = Some(config.get_untracked()).filter(|s| !s.trim().is_empty());
            let clear = cfg.is_none();
            let close = move || open.set(false);
            if let Some(id) = id {
                app.save_then(
                    category::category_update(
                        app.db(),
                        id,
                        Some(name),
                        None,
                        desc,
                        Some(slug),
                        cfg,
                        Some(clear),
                    ),
                    close,
                );
            } else {
                app.save_then(
                    category::category_create(app.db(), name, desc, slug, cfg),
                    close,
                );
            }
        },
    );
    view()
        .gap_4()
        .child(field("Category name", name))
        .child(field("Description", desc))
        .child(view().gap_1().child(field_label("Schema")).child(options))
        .child(reveal("Advanced", None, advanced, move || {
            view()
                .pl(px(28.))
                .child(field_help(
                    "View defaults (JSON)",
                    "Default sort, filters and grouping for this category.",
                    config,
                ))
                .into_element()
        }))
        .child(dialog_actions(open, submit))
        .into_element()
}

fn cleanup_tools(app: App) -> AnyElement {
    let from = signal(String::new());
    let to = signal(String::new());
    let cat = signal(None);
    let assign = signal(String::new());
    let db = app.db();
    view()
        .gap_2()
        .child(subheading("Analysis"))
        .child(app.load(
            async move {
                Ok((
                    file::reanalyze::file_count_novels_missing_tags(db.clone()).await?,
                    file::reanalyze::file_count_comics_missing_covers(db).await?,
                ))
            },
            move |(n, c)| {
                view()
                    .child(setting(
                        "Missing tags",
                        Some("Run analysis again on novels that have no tags."),
                        button(format!("Analyze {n} novels missing tags"), move || {
                            app.local_job(
                                file::reanalyze::file_reanalyze_missing_tags(app.db()),
                                move |r| {
                                    app.message.set(format!(
                                        "{} succeeded · {} failed",
                                        r.succeeded, r.failed
                                    ))
                                },
                            )
                        }),
                    ))
                    .child(setting(
                        "Missing covers",
                        Some("Extract covers again for comics without one."),
                        button(format!("Regenerate {c} missing covers"), move || {
                            app.local_job(
                                file::reanalyze::file_regenerate_missing_covers(app.db()),
                                move |r| {
                                    app.message.set(format!(
                                        "{} regenerated · {} skipped · {} failed",
                                        r.regenerated, r.skipped, r.failed
                                    ))
                                },
                            )
                        }),
                    ))
                    .child(setting(
                        "File locations",
                        Some("Mark files that were moved or deleted outside Biblio."),
                        button("Check file locations", move || {
                            app.save(file::search_lucky_status::file_check_status(app.db(), None))
                        }),
                    ))
                    .into_element()
            },
        ))
        .child(section("Names"))
        .child(setting(
            "Unused names",
            Some("Tags and authors that no file uses."),
            wrap()
                .gap_2()
                .child(confirm(
                    "Delete unused tags",
                    "Delete every unused tag?",
                    "Tags that no file uses are deleted. This cannot be undone.",
                    true,
                    move || {
                        app.local_job(tag::tag_delete_unused(app.db()), move |r| {
                            app.message.set(format!("{} tags deleted", r.deleted))
                        })
                    },
                ))
                .child(confirm(
                    "Delete unused authors",
                    "Delete every unused author?",
                    "Authors that no file uses are deleted. This cannot be undone.",
                    true,
                    move || {
                        app.local_job(author::author_delete_unused(app.db()), move |r| {
                            app.message.set(format!("{} authors deleted", r.deleted))
                        })
                    },
                )),
        ))
        .child(setting(
            "Merge authors",
            Some(
                "Move every file of the source author to the destination, then delete the source.",
            ),
            view()
                .gap_2()
                .child(
                    row()
                        .gap_2()
                        .child(input("Source author ID", from).flex_1())
                        .child(input("Destination author ID", to).flex_1()),
                )
                .child(confirm(
                    "Merge authors",
                    "Merge these authors?",
                    "The source author's files move to the destination and the source is deleted.",
                    true,
                    move || {
                        if let (Ok(a), Ok(b)) =
                            (from.get_untracked().parse(), to.get_untracked().parse())
                        {
                            app.save(author::author_merge(app.db(), a, b));
                        } else {
                            app.message.set("Enter both author IDs".into());
                        }
                    },
                )),
        ))
        .child(setting(
            "Authorless files",
            Some("Give every file without an author in a category the same author."),
            view()
                .gap_2()
                .child(crate::detail::category_picker(app, cat))
                .child(input("Author ID", assign))
                .child(confirm(
                    "Assign to authorless files",
                    "Assign this author to every authorless file?",
                    "Only files in the chosen category without any author change.",
                    false,
                    move || {
                        if let (Some(cat), Ok(id)) =
                            (cat.get_untracked(), assign.get_untracked().parse())
                        {
                            app.local_job(
                                file::reanalyze::file_assign_author_to_authorless(
                                    app.db(),
                                    Some(cat),
                                    id,
                                ),
                                move |r| app.message.set(format!("{} files updated", r.assigned)),
                            );
                        } else {
                            app.message
                                .set("Choose a category and enter an author ID".into());
                        }
                    },
                )),
        ))
        .child(section("Duplicates"))
        .child(duplicate_groups(app))
        .into_element()
}
fn duplicate_groups(app: App) -> AnyElement {
    let show = signal(false);
    view()
        .gap_3()
        .child(button("Find possible duplicates", move || show.set(true)))
        .child(when(
            move || show.get(),
            move || {
                app.load(
                    file::duplicates::file_duplicate_groups(app.db(), None, None, None),
                    move |groups| {
                        let mut r = view().gap_3().child(caption(format!(
                            "{} possible duplicate groups",
                            groups.len()
                        )));
                        for g in groups {
                            let mut group = view()
                                .gap(px(2.))
                                .pt_2()
                                .border_t_1()
                                .border_color(rule)
                                .child(field_label(g.prefix.clone()));
                            for f in &g.files {
                                let id = f.id;
                                group = group.child(
                                    control(
                                        &f.display_name,
                                        Variant::Ghost,
                                        ButtonSize::Sm,
                                        Some(Icon::Open),
                                        move || app.page.set(Page::File(id)),
                                    )
                                    .justify_start(),
                                );
                            }
                            r = r.child(group);
                        }
                        r.into_element()
                    },
                )
            },
        ))
        .into_element()
}
