use crate::{app::App, ui::*};
use biblio_core::commands::{
    schema_admin::{self as admin, SchemaFieldInput, SchemaStepInput, SchemaUpsertPayload},
    schema_transfer,
    schemas::{self, SchemaDefinition},
};
use nsh_core::prelude::*;
use std::rc::Rc;

pub fn screen(app: App) -> AnyElement {
    let selection = signal(None::<String>);
    let imported = signal_with(None::<Rc<SchemaDefinition>>, SignalEquals::Never);
    let id = selection;
    view()
        .gap_4()
        .child(
            wrap()
                .gap_2()
                .child(control(
                    "New schema",
                    Variant::Default,
                    ButtonSize::Md,
                    Some(Icon::Plus),
                    move || {
                        imported.set(None);
                        id.set(None);
                    },
                ))
                .child(button("Import schema", move || {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Schema", &["json"])
                        .pick_file()
                    {
                        app.local_job(
                            schema_transfer::schema_import_read(path.to_string_lossy().into()),
                            move |s| {
                                selection.set(None);
                                imported.set(Some(Rc::new(s)));
                            },
                        );
                    }
                })),
        )
        .child(dynamic(move || {
            app.revision.get();
            app.load(schemas::schema_list(app.db()), move |schemas| {
                let mut nav = view().gap(px(1.)).w(px(200.)).flex_shrink_0().child(
                    caption("Schemas")
                        .font_weight(FontWeight::SEMIBOLD)
                        .px(px(8.))
                        .pb_1(),
                );
                for s in schemas {
                    let key = s.schema.id.clone();
                    let mine = stored(key.clone());
                    let on = move || mine.with(|k| selection.with(|v| v.as_ref() == Some(k)));
                    nav = nav.child(
                        nsh_base::button()
                            .accessibility_label(s.schema.name.clone())
                            .selected(on)
                            .on_press(move || {
                                imported.set(None);
                                selection.set(Some(key.clone()));
                            })
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .min_h(px(30.))
                            .px(px(8.))
                            .rounded(px(RADIUS))
                            .bg(move || if on() { wash() } else { clear() })
                            .hover(|s| s.bg(wash))
                            .focus(|s| s.outline(px(2.)).outline_color(accent))
                            .child(label(s.schema.name.clone()).number_of_lines(1).style_with(
                                move |st| {
                                    if on() {
                                        st.font_weight(FontWeight::SEMIBOLD)
                                    } else {
                                        st
                                    }
                                },
                            ))
                            .child(caption(if s.schema.is_builtin { "Built-in" } else { "" })),
                    );
                }
                let schemas = stored(
                    schemas
                        .iter()
                        .map(|s| SchemaDefinition {
                            schema: s.schema.clone(),
                            fields: s.fields.clone(),
                            pipeline_steps: s.pipeline_steps.clone(),
                        })
                        .collect::<Vec<_>>(),
                );
                wrap()
                    .items_start()
                    .gap_x(px(32.))
                    .gap_y(px(16.))
                    .child(nav)
                    .child(view().flex_1().min_w(px(320.)).child(dynamic(move || {
                        let key = selection.get();
                        let import = imported.get();
                        untracked(|| {
                            schemas.with(|v| {
                                let def = import.as_deref().or_else(|| {
                                    v.iter().find(|s| Some(&s.schema.id) == key.as_ref())
                                });
                                editor(app, def, key.is_some(), selection, imported)
                            })
                        })
                    })))
                    .into_element()
            })
        }))
        .into_element()
}
fn editor(
    app: App,
    def: Option<&SchemaDefinition>,
    existing: bool,
    selection: Signal<Option<String>>,
    imported: Signal<Option<Rc<SchemaDefinition>>>,
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
            view()
                .gap_1()
                .child(subheading(name.get_untracked()))
                .child(caption(format!("Schema · {}", slug.get_untracked())))
                .into_element()
        } else {
            view()
                .gap_3()
                .child(subheading("New schema"))
                .child(field("Schema ID", slug))
                .into_element()
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
            move |id| {
                imported.set(None);
                selection.set(Some(id));
                app.refresh();
            },
        );
    };
    let mut actions = wrap().gap_2().pt_4().child(primary("Save schema", save));
    if existing {
        actions = actions.child(button("Export schema", move || {
            if let Some(path) = rfd::FileDialog::new()
                .set_file_name(format!("{}.json", slug.get_untracked()))
                .save_file()
            {
                app.save(schema_transfer::schema_export(
                    app.db(),
                    slug.get_untracked(),
                    path.to_string_lossy().into(),
                ));
            }
        }));
        if def.is_some_and(|s| !s.schema.is_builtin) {
            actions = actions.child(confirm(
                "Delete schema",
                "Delete this schema?",
                "Categories that use it must be moved to another schema first. This cannot be undone.",
                true,
                move || {
                    app.local_job(
                        admin::schema_delete(app.db(), slug.get_untracked()),
                        move |_| {
                            selection.set(None);
                            app.refresh();
                        },
                    )
                },
            ));
        }
    }
    body.child(actions).into_element()
}
