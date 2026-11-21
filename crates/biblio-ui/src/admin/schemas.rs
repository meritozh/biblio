use crate::{
    app::App,
    ui::{self, *},
};
use biblio_core::commands::{
    schema_admin::{self as admin, SchemaFieldInput, SchemaStepInput, SchemaUpsertPayload},
    schema_transfer,
    schemas::{self, SchemaDefinition},
};
use nsh_core::prelude::*;
use std::rc::Rc;

pub fn screen(app: App) -> AnyElement {
    let data = app.resource(move || {
        app.revision.get();
        schemas::schema_list(app.db())
    });
    // What the dialog edits: an existing schema by slug, an imported file,
    // or (both empty) a new schema.
    let editing = signal(None::<String>);
    let imported = signal_with(None::<Rc<SchemaDefinition>>, SignalEquals::Never);
    let open = signal(false);
    let import = move || {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Schema", &["json"])
            .pick_file()
        {
            app.local_job(
                schema_transfer::schema_import_read(path.to_string_lossy().into()),
                move |s| {
                    editing.set(None);
                    imported.set(Some(Rc::new(s)));
                    open.set(true);
                },
            );
        }
    };
    let export = move |slug: String| {
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(format!("{slug}.json"))
            .save_file()
        {
            app.local_job(
                schema_transfer::schema_export(app.db(), slug, path.to_string_lossy().into()),
                move |_| app.message.set("Schema exported".into()),
            );
        }
    };
    let header = page_header(
        Icon::Schemas,
        "Schemas",
        Some(
            count_caption(move || match data.get().as_deref() {
                Some(Ok(list)) if !list.is_empty() => {
                    counted(list.len() as i64, "schema", "schemas")
                }
                _ => String::new(),
            })
            .into_element(),
        ),
        Some(
            row()
                .gap_2()
                .child(control(
                    "Import",
                    Variant::Default,
                    ButtonSize::Md,
                    Some(Icon::Upload),
                    import,
                ))
                .child(control(
                    "New schema",
                    Variant::Primary,
                    ButtonSize::Md,
                    Some(Icon::Plus),
                    move || {
                        imported.set(None);
                        editing.set(None);
                        open.set(true);
                    },
                ))
                .into_element(),
        ),
    );
    let cols = vec![
        Col::custom("name", "Name", None),
        Col::mono("slug", "Slug", Some(140.)),
        Col::custom("pipeline", "Pipeline", Some(130.)),
        Col::mono("extensions", "Extensions", Some(220.)),
        Col::muted("fields", "Fields", Some(80.)).end(),
        Col::command("export", Icon::Download),
        Col::command("edit", Icon::Edit),
    ];
    let keys: Vec<&'static str> = cols.iter().map(|c| c.key).collect();
    let shown = dynamic(move || match data.get().as_deref() {
        None => caption("Loading…").into_element(),
        Some(Err(e)) => label(e.clone()).into_element(),
        Some(Ok(list)) => {
            let n = list.len();
            let slugs: Rc<Vec<String>> =
                Rc::new(list.iter().map(|s| s.schema.id.clone()).collect());
            let builtin: Rc<Vec<bool>> =
                Rc::new(list.iter().map(|s| s.schema.is_builtin).collect());
            let rows = nsh_base::DataTableRows::new(n)
                .texts("name", list.iter().map(|s| s.schema.name.as_str()))
                .texts("slug", list.iter().map(|s| s.schema.id.as_str()))
                .texts(
                    "pipeline",
                    list.iter().map(|s| s.schema.pipeline_template.as_str()),
                )
                .texts(
                    "extensions",
                    list.iter().map(|s| {
                        serde_json::from_str::<Vec<String>>(&s.schema.accepted_extensions)
                            .unwrap_or_default()
                            .iter()
                            .map(|e| format!(".{e}"))
                            .collect::<Vec<_>>()
                            .join(" ")
                    }),
                )
                .texts("fields", list.iter().map(|s| s.fields.len().to_string()))
                .texts(
                    "export",
                    list.iter().map(|s| format!("Export {}", s.schema.name)),
                )
                .texts(
                    "edit",
                    list.iter().map(|s| format!("Edit {}", s.schema.name)),
                );
            let keys = Rc::new(keys.clone());
            let pick = slugs.clone();
            let edit = Rc::new(move |row: u32| {
                if let Some(slug) = pick.get(row as usize) {
                    imported.set(None);
                    editing.set(Some(slug.clone()));
                    open.set(true);
                }
            });
            let double = edit.clone();
            let cell = move |c: &nsh_base::DataTableCell| {
                if c.key.as_str() == "pipeline" {
                    return row().child(badge(c.text.to_string())).into_element();
                }
                let mut line = row()
                    .gap_2()
                    .min_w_0()
                    .child(label(c.text.to_string()).number_of_lines(1).flex_shrink(1.));
                if builtin.get(c.row as usize).copied().unwrap_or(false) {
                    line = line.child(badge("Built-in"));
                }
                line.into_element()
            };
            table(
                "Schemas",
                cols.clone(),
                move || n,
                move || rows.clone(),
                cell,
                move |t| {
                    t.on_row_press(move |p: &nsh_base::DataTablePress| {
                        match pressed_key(&keys, p) {
                            Some("edit") => edit(p.row),
                            Some("export") => {
                                if let Some(slug) = slugs.get(p.row as usize) {
                                    export(slug.clone());
                                }
                            }
                            _ => {}
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
    });
    let dialog = modal(
        open,
        move || {
            if editing.with_untracked(Option::is_some) {
                "Edit schema"
            } else if imported.with_untracked(Option::is_some) {
                "Import schema"
            } else {
                "New schema"
            }
            .to_owned()
        },
        760.,
        move || {
            // Made once the schemas are here; a later refresh leaves the
            // editor alone.
            let ready = computed(move || data.with(Option::is_some));
            dynamic(move || {
                if !ready.get() {
                    return caption("Loading…").into_element();
                }
                untracked(|| {
                    let key = editing.get();
                    let import = imported.get();
                    match data.get().as_deref() {
                        Some(Ok(list)) => {
                            let def = import.as_deref().or_else(|| {
                                list.iter().find(|s| Some(&s.schema.id) == key.as_ref())
                            });
                            editor(app, def, key.is_some(), open)
                        }
                        Some(Err(e)) => label(e.clone()).into_element(),
                        None => view().into_element(),
                    }
                })
            })
            .into_element()
        },
    );
    ui::page(
        header,
        page_body()
            .child(about(
                "A schema defines a resource type: which files it accepts, which fields its files carry, and which import pipeline runs. Categories bind to a schema and files inherit it from their category. Built-in schemas can be edited but not deleted. Export shares a schema as a JSON file of its fields and pipeline steps.",
            ))
            .child(shown)
            .child(dialog),
    )
    .into_element()
}

fn editor(
    app: App,
    def: Option<&SchemaDefinition>,
    existing: bool,
    open: Signal<bool>,
) -> AnyElement {
    let slug = signal(def.map(|s| s.schema.id.clone()).unwrap_or_default());
    let name = signal(def.map(|s| s.schema.name.clone()).unwrap_or_default());
    let template = signal(
        def.map(|s| s.schema.pipeline_template.clone())
            .unwrap_or_else(|| "novel".into()),
    );
    let extensions = signal(
        def.and_then(|s| serde_json::from_str::<Vec<String>>(&s.schema.accepted_extensions).ok())
            .unwrap_or_default()
            .join(", "),
    );
    let icon = signal(def.and_then(|s| s.schema.icon.clone()).unwrap_or_default());
    let description = signal(
        def.and_then(|s| s.schema.description.clone())
            .unwrap_or_default(),
    );
    let fields = stored(
        def.map(|s| {
            s.fields
                .iter()
                .map(|f| SchemaFieldInput {
                    field_key: f.field_key.clone(),
                    label: f.label.clone(),
                    semantic: f.semantic.clone(),
                    field_type: f.field_type.clone(),
                    options: f.options.clone(),
                    form_visible: f.form_visible,
                    card_visible: f.card_visible,
                    sortable: f.sortable,
                    filterable: f.filterable,
                    required: f.required,
                    order_index: f.order_index,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default(),
    );
    let steps = stored(
        def.map(|s| {
            s.pipeline_steps
                .iter()
                .map(|p| SchemaStepInput {
                    step_key: p.step_key.clone(),
                    label: p.label.clone(),
                    enabled: p.enabled,
                    order_index: p.order_index,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default(),
    );
    let changed = signal(0u64);
    let keep_steps = def.is_some();
    let mut body = view()
        .gap_3()
        .child(if existing {
            caption(format!("Schema ID · {}", slug.get_untracked())).into_element()
        } else {
            field("Schema ID", slug).into_element()
        })
        .child(field("Schema name", name))
        .child(field("Description", description))
        .child(field("Icon", icon))
        .child(field_help(
            "Pipeline template",
            "novel, comic or galgame",
            template,
        ))
        .child(field_help(
            "Extensions",
            "Comma-separated, such as epub, pdf, txt",
            extensions,
        ))
        .child(section("Fields"));
    body = body
        .child(dynamic(move || {
            changed.get();
            let data = fields.with(|v| v.clone());
            let mut list = view().gap_2();
            for (i, f) in data.into_iter().enumerate() {
                let key = f.field_key.clone();
                let semantic = f.semantic.is_some();
                let update = move |k: &'static str, v: &str| {
                    fields.with(|fs| {
                        let f = &mut fs[i];
                        match k {
                            "key" => f.field_key = v.into(),
                            "label" => f.label = v.into(),
                            "type" => f.field_type = v.into(),
                            "options" => {
                                f.options = Some(v.into()).filter(|s: &String| !s.is_empty())
                            }
                            _ => {}
                        }
                    })
                };
                let editor = |hint: &'static str, initial: String, k: &'static str| {
                    input_face()
                        .default_value(initial)
                        .label(hint)
                        .placeholder(hint)
                        .on_change(move |s: &str| update(k, s))
                };
                let mut line = wrap()
                    .gap_2()
                    .child(if semantic {
                        label(&key).font_weight(FontWeight::MEDIUM).into_element()
                    } else {
                        editor("Field key", key, "key").w(px(140.)).into_element()
                    })
                    .child(editor("Field label", f.label, "label").flex_1().min_w(px(140.)));
                if !semantic {
                    line = line
                        .child(editor("Field type", f.field_type, "type"))
                        .child(editor(
                            "Enum options (JSON array)",
                            f.options.unwrap_or_default(),
                            "options",
                        ));
                }
                let mut flags = wrap().gap_x(px(16.)).gap_y(px(4.));
                for (kind, initial) in [
                    ("Form", f.form_visible),
                    ("Card", f.card_visible),
                    ("Sort", f.sortable),
                    ("Filter", f.filterable),
                    ("Required", f.required),
                ] {
                    let (v, box_view) = check(kind, initial);
                    effect(move || {
                        let value = v.get();
                        fields.with(|fs| {
                            if let Some(f) = fs.get_mut(i) {
                                match kind {
                                    "Form" => f.form_visible = value,
                                    "Card" => f.card_visible = value,
                                    "Sort" => f.sortable = value,
                                    "Filter" => f.filterable = value,
                                    _ => f.required = value,
                                }
                            }
                        });
                    });
                    flags = flags.child(box_view);
                }
                let mut tools = row().gap_1();
                if i > 0 {
                    tools = tools.child(icon_button("Move up", Icon::Upload, None, move || {
                        fields.with(|v| v.swap(i, i - 1));
                        changed.update(|v| *v += 1);
                    }));
                }
                if !semantic {
                    tools = tools.child(icon_button(
                        "Remove field and stored values on save",
                        Icon::Trash,
                        None,
                        move || {
                            ask(
                                "Remove this field?",
                                "When you save the schema, the field and every value stored in it are deleted.",
                                "Remove field",
                                true,
                                move || {
                                    fields.with(|v| {
                                        v.remove(i);
                                    });
                                    changed.update(|v| *v += 1);
                                },
                            )
                        },
                    ));
                }
                list = list.child(
                    view()
                        .gap_2()
                        .p_3()
                        .rounded(px(RADIUS))
                        .border_1()
                        .border_color(rule)
                        .child(row().gap_2().child(line.flex_1()).child(tools))
                        .child(flags),
                );
            }
            list
        }))
        .child(control("Add field", Variant::Default, ButtonSize::Md, Some(Icon::Plus), move || {
            fields.with(|v| {
                v.push(SchemaFieldInput {
                    field_key: String::new(),
                    semantic: None,
                    field_type: "text".into(),
                    label: String::new(),
                    options: None,
                    form_visible: true,
                    card_visible: false,
                    sortable: false,
                    filterable: false,
                    required: false,
                    order_index: v.len() as i64,
                })
            });
            changed.update(|v| *v += 1);
        }));
    body = body
        .child(section("Pipeline steps"))
        .child(dynamic(move || {
            changed.get();
            let mut list = view().gap(px(2.));
            for (i, step) in steps.with(|v| v.clone()).iter().enumerate() {
                let (v, check) = check(&step.label, step.enabled);
                effect(move || {
                    let enabled = v.get();
                    steps.with(|steps| steps[i].enabled = enabled);
                });
                let mut line = row().gap_2().child(view().flex_1().child(check));
                if i > 0 {
                    line = line.child(icon_button("Move step up", Icon::Upload, None, move || {
                        steps.with(|v| v.swap(i, i - 1));
                        changed.update(|v| *v += 1);
                    }));
                }
                list = list.child(line);
            }
            list
        }));
    let save = move || {
        let id = slug.get_untracked();
        let db = app.db();
        let mut fields = fields.with(|v| v.clone());
        for (i, f) in fields.iter_mut().enumerate() {
            f.order_index = i as i64;
        }
        let p = SchemaUpsertPayload {
            name: name.get_untracked(),
            description: Some(description.get_untracked()),
            icon: Some(icon.get_untracked()),
            accepted_extensions: extensions
                .get_untracked()
                .split(',')
                .map(|s| s.trim().into())
                .collect(),
            pipeline_template: template.get_untracked(),
            fields,
        };
        let mut steps = steps.with(|v| v.clone());
        for (i, s) in steps.iter_mut().enumerate() {
            s.order_index = i as i64;
        }
        app.local_job(
            async move {
                admin::save_definition(db, id.clone(), p, existing, keep_steps.then_some(steps))
                    .await?;
                Ok(id)
            },
            move |_| {
                open.set(false);
                app.message.set("Saved".into());
                app.refresh();
            },
        );
    };
    let mut actions = row().justify_end().gap_2().pt_4();
    if existing && def.is_some_and(|s| !s.schema.is_builtin) {
        actions = actions.child(view().flex_1().child(confirm(
            "Delete schema",
            "Delete this schema?",
            "Categories that use it must be moved to another schema first. This cannot be undone.",
            true,
            move || {
                app.local_job(
                    admin::schema_delete(app.db(), slug.get_untracked()),
                    move |_| {
                        open.set(false);
                        app.refresh();
                    },
                )
            },
        )));
    }
    actions = actions
        .child(button("Cancel", move || open.set(false)))
        .child(primary("Save schema", save));
    body.child(actions).into_element()
}
