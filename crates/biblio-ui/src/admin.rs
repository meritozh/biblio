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
    stack()
        .flex_1()
        .min_h_0()
        .gap_4()
        .child(heading(name))
        .child(
            scroll()
                .flex_1()
                .min_h_0()
                .child(stack().max_w(px(MEASURE)).pb_6().child(body)),
        )
        .into_element()
}

fn names(app: App, tags: bool) -> AnyElement {
    let q = signal(String::new());
    let new = signal(String::new());
    let pos = signal(0i64);
    effect(move || {
        q.get();
        pos.set(0);
    });
    stack()
        .child(input("Search names", q))
        .child(
            row()
                .child(input("New name", new))
                .child(primary("Create", move || {
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
                            Some(30),
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
                            Some(30),
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
                    let mut body = stack().child(label(format!("{total} names")));
                    for (id, name, count, color) in rows {
                        let id = *id;
                        let value = signal(name.clone());
                        let color = signal(color.clone().unwrap_or_default());
                        let mut line = row()
                            .child(label(format!("#{id} · {count} files")))
                            .child(input("Name", value));
                        if tags {
                            line = line.child(input("Color", color));
                        }
                        body = body.child(
                            line.child(button("Save", move || {
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
                            .child(confirm("Delete", move || {
                                if tags {
                                    app.save(tag::tag_delete(app.db(), id));
                                } else {
                                    app.save(author::author_delete(app.db(), id));
                                }
                            })),
                        );
                    }
                    let mut pager = row();
                    if p > 0 {
                        pager = pager.child(button("Previous", move || pos.set(p - 30)));
                    }
                    if p + 30 < *total {
                        pager = pager.child(button("Next", move || pos.set(p + 30)));
                    }
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
                let mut s = stack().child(category_form(app, None, schemas));
                for cat in cats {
                    s = s.child(category_form(app, Some(cat), schemas));
                }
                s.into_element()
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
    let mut options = wrap();
    for s in schemas {
        let id = s.schema.id.clone();
        let key = stored(id.clone());
        options = options.child(choice(
            s.schema.name.clone(),
            move || key.with(|k| slug.with(|v| v == k)),
            move || slug.set(id.clone()),
        ));
    }
    stack()
        .child(section(if id.is_some() {
            "Edit category"
        } else {
            "New category"
        }))
        .child(field("Category name", name))
        .child(field("Description", desc))
        .child(field_label("Schema"))
        .child(options)
        .child(field("View defaults (JSON)", config))
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
            let mut s = stack().child(prompt_form(app, None));
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
                s = s
                    .child(button(
                        p.with(|p| format!("{} · {} / {}", p.1, p.3, p.4)),
                        move || opened.update(|b| *b = !*b),
                    ))
                    .child(when(
                        move || opened.get(),
                        move || prompt_form(app, Some(p.with(|p| p.clone()))),
                    ));
            }
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
    let mut actions = wrap().child(primary(
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
            .child(confirm("Delete prompt", move || {
                app.save(prompts::prompt_delete(app.db(), id))
            }));
    }
    stack()
        .child(field("Prompt name", name))
        .child(
            row()
                .child(input("Schema ID", slug))
                .child(input("Pipeline step", step)),
        )
        .child(
            input("Prompt text", content)
                .multiline(true)
                .min_h(px(110.)),
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
    stack()
        .child(app.load(
            async move {
                Ok((
                    file::reanalyze::file_count_novels_missing_tags(db.clone()).await?,
                    file::reanalyze::file_count_comics_missing_covers(db).await?,
                ))
            },
            move |(n, c)| {
                wrap()
                    .child(button(
                        format!("Analyze {n} novels missing tags"),
                        move || {
                            app.local_job(
                                file::reanalyze::file_reanalyze_missing_tags(app.db()),
                                move |r| {
                                    app.message.set(format!(
                                        "{} succeeded · {} failed",
                                        r.succeeded, r.failed
                                    ))
                                },
                            )
                        },
                    ))
                    .child(button(
                        format!("Regenerate {c} missing covers"),
                        move || {
                            app.local_job(
                                file::reanalyze::file_regenerate_missing_covers(app.db()),
                                move |r| {
                                    app.message.set(format!(
                                        "{} regenerated · {} skipped · {} failed",
                                        r.regenerated, r.skipped, r.failed
                                    ))
                                },
                            )
                        },
                    ))
                    .into_element()
            },
        ))
        .child(
            wrap()
                .child(confirm("Delete unused tags", move || {
                    app.local_job(tag::tag_delete_unused(app.db()), move |r| {
                        app.message.set(format!("{} tags deleted", r.deleted))
                    })
                }))
                .child(confirm("Delete unused authors", move || {
                    app.local_job(author::author_delete_unused(app.db()), move |r| {
                        app.message.set(format!("{} authors deleted", r.deleted))
                    })
                })),
        )
        .child(
            row()
                .child(input("Source author ID", from))
                .child(input("Destination author ID", to))
                .child(confirm("Merge authors", move || {
                    if let (Ok(a), Ok(b)) =
                        (from.get_untracked().parse(), to.get_untracked().parse())
                    {
                        app.save(author::author_merge(app.db(), a, b));
                    } else {
                        app.message.set("Enter both author IDs".into());
                    }
                })),
        )
        .child(crate::detail::category_picker(app, cat))
        .child(row().child(input("Author ID", assign)).child(confirm(
            "Assign to authorless files",
            move || {
                if let (Some(cat), Ok(id)) = (cat.get_untracked(), assign.get_untracked().parse()) {
                    app.local_job(
                        file::reanalyze::file_assign_author_to_authorless(app.db(), Some(cat), id),
                        move |r| app.message.set(format!("{} files updated", r.assigned)),
                    );
                } else {
                    app.message
                        .set("Choose a category and enter an author ID".into());
                }
            },
        )))
        .child(button("Check file locations", move || {
            app.save(file::search_lucky_status::file_check_status(app.db(), None))
        }))
        .child(duplicate_groups(app))
        .into_element()
}
fn duplicate_groups(app: App) -> AnyElement {
    let show = signal(false);
    stack()
        .child(button("Find possible duplicates", move || show.set(true)))
        .child(when(
            move || show.get(),
            move || {
                app.load(
                    file::duplicates::file_duplicate_groups(app.db(), None, None, None),
                    move |groups| {
                        let mut r = stack()
                            .child(label(format!("{} possible duplicate groups", groups.len())));
                        for g in groups {
                            r = r.child(label(&g.prefix));
                            for f in &g.files {
                                let id = f.id;
                                r = r.child(button(&f.display_name, move || {
                                    app.page.set(Page::File(id))
                                }));
                            }
                        }
                        r.into_element()
                    },
                )
            },
        ))
        .into_element()
}
