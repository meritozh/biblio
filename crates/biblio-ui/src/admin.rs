use crate::{
    app::{App, Page},
    ui::*,
};
use biblio_core::commands::{self as api, author, category, file, prompts, tag};
use nsh_core::prelude::*;
mod schemas;

pub fn screen(app: App, page: Page) -> AnyElement {
    let (name, body) = match page {
        Page::Categories => ("Categories", categories(app)),
        Page::Tags => ("Tags", names(app, true)),
        Page::Authors => ("Authors", names(app, false)),
        Page::Prompts => ("Prompts", prompt_list(app)),
        Page::Schemas => ("Schemas", schemas::screen(app)),
        Page::Cleanup => ("Cleanup", cleanup_tools(app)),
        _ => unreachable!(),
    };
    let measure = if page == Page::Schemas { 960. } else { MEASURE };
    view()
        .flex_1()
        .min_h_0()
        .gap_4()
        .child(heading(name))
        .child(
            scroll()
                .flex_1()
                .min_h_0()
                .child(view().max_w(px(measure)).pb_8().child(body)),
        )
        .into_element()
}

fn names(app: App, tags: bool) -> AnyElement {
    const NAME_PAGE: i64 = 30;
    let q = signal(String::new());
    let new = signal(String::new());
    let pos = signal(0i64);
    effect(move || {
        q.get();
        pos.set(0);
    });
    let kind = if tags { "tag" } else { "author" };
    view()
        .gap_4()
        .child(
            wrap()
                .gap_2()
                .child(input("Search names", q).flex_1().min_w(px(200.)))
                .child(view().w(px(1.)).h(px(24.)).bg(rule))
                .child(input("New name", new).w(px(200.)))
                .child(control("Create", Variant::Primary, ButtonSize::Md, Some(Icon::Plus), move || {
                    let name = new.get_untracked();
                    let db = app.db();
                    app.local_job(
                        async move {
                            if tags {
                                tag::tag_create(db, name, None).await?;
                            } else {
                                author::author_create(db, name).await?;
                            }
                            Ok(())
                        },
                        move |_| {
                            new.set(String::new());
                            app.refresh();
                        },
                    );
                })),
        )
        .child(dynamic(move || {
            let q = q.get();
            let p = pos.get();
            app.revision.get();
            let db = app.db();
            app.load(
                async move {
                    if tags {
                        let rows = tag::tag_list(
                            db.clone(),
                            Some(true),
                            Some(NAME_PAGE),
                            Some(p),
                            Some(q.clone()),
                        )
                        .await?
                        .tags;
                        Ok((
                            tag::tag_count(db, Some(q)).await?,
                            rows.into_iter()
                                .map(|t| (t.id, t.name, t.usage_count, t.color))
                                .collect::<Vec<_>>(),
                        ))
                    } else {
                        let rows = author::author_list(
                            db.clone(),
                            Some(true),
                            Some(NAME_PAGE),
                            Some(p),
                            Some(q.clone()),
                        )
                        .await?
                        .authors;
                        Ok((
                            author::author_count(db, Some(q)).await?,
                            rows.into_iter()
                                .map(|a| (a.id, a.name, a.usage_count, None))
                                .collect(),
                        ))
                    }
                },
                move |(total, rows)| {
                    if p > 0 && p >= *total {
                        let last = (total - 1).max(0) / NAME_PAGE * NAME_PAGE;
                        pos.set(last);
                        return label("Loading…").into_element();
                    }
                    if *total == 0 {
                        return empty(
                            if tags { "No tags" } else { "No authors" },
                            "Create one above, or let analysis add them while importing.",
                        )
                        .into_element();
                    }
                    let mut body = view().child(
                        row()
                            .gap_3()
                            .pb_2()
                            .border_b_1()
                            .border_color(rule)
                            .child(caption("ID").w(px(44.)))
                            .child(caption("Name").flex_1())
                            .child(caption("Files").w(px(56.)))
                            .child(view().w(px(if tags { 252. } else { 124. }))),
                    );
                    for (id, name, count, color) in rows {
                        let id = *id;
                        let value = signal(name.clone());
                        let color = signal(color.clone().unwrap_or_default());
                        let shown = name.clone();
                        let count = *count;
                        let mut line = row()
                            .gap_3()
                            .py(px(6.))
                            .border_b_1()
                            .border_color(rule)
                            .child(caption(format!("#{id}")).w(px(44.)))
                            .child(input("Name", value).flex_1());
                        line = line.child(caption(count.to_string()).w(px(56.)));
                        if tags {
                            line = line.child(input("Color", color).w(px(120.)));
                        }
                        body = body.child(
                            line.child(small("Save", move || {
                                if tags {
                                    app.save(tag::tag_update(
                                        app.db(),
                                        id,
                                        Some(value.get_untracked()),
                                        Some(color.get_untracked()).filter(|s| !s.is_empty()),
                                    ));
                                } else {
                                    app.save(author::author_update(
                                        app.db(),
                                        id,
                                        value.get_untracked(),
                                    ));
                                }
                            }))
                            .child(icon_button(
                                format!("Delete {shown}"),
                                Icon::Trash,
                                None,
                                move || {
                                    let shown = shown.clone();
                                    ask(
                                        format!("Delete the {kind} “{shown}”?"),
                                        format!("It is removed from {count} files. This cannot be undone."),
                                        "Delete",
                                        true,
                                        move || {
                                            if tags {
                                                app.save(tag::tag_delete(app.db(), id));
                                            } else {
                                                app.save(author::author_delete(app.db(), id));
                                            }
                                        },
                                    )
                                },
                            )),
                        );
                    }
                    let mut pager = row().justify_between().pt_3().child(caption(format!(
                        "{total} {}",
                        if tags { "tags" } else { "authors" }
                    )));
                    let mut buttons = row().gap_1();
                    if p > 0 {
                        buttons = buttons.child(icon_button("Previous", Icon::Back, None, move || pos.set(p - NAME_PAGE)));
                    }
                    if p + NAME_PAGE < *total {
                        buttons = buttons.child(icon_button("Next", Icon::Forward, None, move || pos.set(p + NAME_PAGE)));
                    }
                    pager = pager.child(buttons);
                    body.child(pager).into_element()
                },
            )
        }))
        .into_element()
}
fn categories(app: App) -> AnyElement {
    dynamic(move || {
        app.revision.get();
        let db = app.db();
        app.load(
            async move {
                Ok((
                    category::category_list(db.clone()).await?,
                    api::schemas::schema_list(db).await?,
                ))
            },
            move |(cats, schemas)| {
                let schemas = std::rc::Rc::new(
                    schemas
                        .iter()
                        .map(|s| api::schemas::SchemaDefinition {
                            schema: s.schema.clone(),
                            fields: s.fields.clone(),
                            pipeline_steps: s.pipeline_steps.clone(),
                        })
                        .collect::<Vec<_>>(),
                );
                let mut list = view().gap(px(2.));
                for cat in cats {
                    let open = signal(false);
                    let cat = std::rc::Rc::new(copy_category(cat));
                    let defs = schemas.clone();
                    let schema_name = schemas
                        .iter()
                        .find(|s| s.schema.id == cat.schema_slug)
                        .map(|s| s.schema.name.clone())
                        .unwrap_or_else(|| cat.schema_slug.clone());
                    list = list.child(reveal(
                        cat.name.clone(),
                        Some(schema_name),
                        open,
                        move || {
                            view()
                                .pl(px(28.))
                                .pb_4()
                                .child(category_form(app, Some(&cat), &defs))
                                .into_element()
                        },
                    ));
                }
                view()
                    .gap_4()
                    .child(
                        group(
                            "New category",
                            "A category is a shelf with its own schema and storage folder.",
                        )
                        .child(category_form(
                            app,
                            None,
                            schemas.as_slice(),
                        )),
                    )
                    .child(section("Categories"))
                    .child(if cats.is_empty() {
                        caption("No categories yet.").into_element()
                    } else {
                        list.into_element()
                    })
                    .into_element()
            },
        )
    })
    .into_element()
}
fn category_form(
    app: App,
    cat: Option<&api::Category>,
    schemas: &[api::schemas::SchemaDefinition],
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
    view()
        .gap_3()
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
        .child(primary(
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
                if let Some(id) = id {
                    app.save(category::category_update(
                        app.db(),
                        id,
                        Some(name),
                        None,
                        desc,
                        Some(slug),
                        cfg,
                        Some(clear),
                    ));
                } else {
                    app.save(category::category_create(app.db(), name, desc, slug, cfg));
                }
            },
        ))
        .into_element()
}
fn prompt_list(app: App) -> AnyElement {
    dynamic(move || {
        app.revision.get();
        app.load(prompts::prompt_list(app.db(), None, None), move |list| {
            let mut s = view()
                .gap_4()
                .child(
                    group("New prompt", "Prompts steer the language model during analysis, per schema and pipeline step.")
                        .child(prompt_form(app, None)),
                )
                .child(section("Prompts"));
            let mut rows = view().gap(px(2.));
            for p in list {
                let opened = signal(false);
                let p = stored((
                    p.id,
                    p.name.clone(),
                    p.content.clone(),
                    p.schema_slug
                        .clone()
                        .unwrap_or_else(|| p.mime_group.clone()),
                    p.step.clone(),
                ));
                let (name, scope) = p.with(|p| (p.1.clone(), format!("{} · {}", p.3, p.4)));
                rows = rows.child(reveal(name, Some(scope), opened, move || {
                    view()
                        .pl(px(28.))
                        .pb_4()
                        .child(prompt_form(app, Some(p.with(|p| p.clone()))))
                        .into_element()
                }));
            }
            s = s.child(if list.is_empty() {
                caption("No prompts yet.").into_element()
            } else {
                rows.into_element()
            });
            s.into_element()
        })
    })
    .into_element()
}
type PromptDraft = (i64, String, String, String, String);
fn prompt_form(app: App, p: Option<PromptDraft>) -> AnyElement {
    let (id, n, c, s, k) = p.unwrap_or((
        0,
        String::new(),
        String::new(),
        "novel".into(),
        "filename".into(),
    ));
    let name = signal(n);
    let content = signal(c);
    let slug = signal(s);
    let step = signal(k);
    let mut actions = wrap().gap_2().child(primary(
        if id == 0 {
            "Create prompt"
        } else {
            "Save prompt"
        },
        move || {
            let payload = prompts::PromptCreate {
                name: name.get_untracked(),
                content: content.get_untracked(),
                schema_slug: slug.get_untracked(),
                step: step.get_untracked(),
            };
            if id == 0 {
                app.save(prompts::prompt_create(app.db(), payload));
            } else {
                app.save(prompts::prompt_update(app.db(), id, payload));
            }
        },
    ));
    if id != 0 {
        actions = actions
            .child(button("Set as default", move || {
                app.save(prompts::prompt_set_default(app.db(), id))
            }))
            .child(confirm(
                "Delete prompt",
                "Delete this prompt?",
                "Analysis falls back to the default prompt for its step.",
                true,
                move || app.save(prompts::prompt_delete(app.db(), id)),
            ));
    }
    view()
        .gap_3()
        .child(field("Prompt name", name))
        .child(
            wrap()
                .gap_3()
                .child(field("Schema ID", slug).flex_1().min_w(px(160.)))
                .child(field("Pipeline step", step).flex_1().min_w(px(160.))),
        )
        .child(
            view().gap_1().child(field_label("Prompt text")).child(
                input("Prompt text", content)
                    .multiline(true)
                    .min_h(px(120.)),
            ),
        )
        .child(actions)
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

fn copy_category(c: &api::Category) -> api::Category {
    api::Category {
        id: c.id,
        name: c.name.clone(),
        description: c.description.clone(),
        icon: c.icon.clone(),
        is_default: c.is_default,
        folder_name: c.folder_name.clone(),
        schema_slug: c.schema_slug.clone(),
        view_config: c.view_config.clone(),
        created_at: c.created_at.clone(),
    }
}
