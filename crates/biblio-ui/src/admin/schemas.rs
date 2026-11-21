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
    stack()
        .child(
            wrap()
                .child(button("New schema", move || {
                    imported.set(None);
                    id.set(None);
                }))
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
                let mut nav = wrap();
                for s in schemas {
                    let key = s.schema.id.clone();
                    nav = nav.child(button(&s.schema.name, move || {
                        imported.set(None);
                        selection.set(Some(key.clone()));
                    }));
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
                stack()
                    .child(nav)
                    .child(dynamic(move || {
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
                    }))
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
    let mut body = stack()
        .child(if existing {
            label(format!("Schema · {}", slug.get_untracked())).into_element()
        } else {
            field("Schema ID", slug).into_element()
        })
        .child(field("Schema name", name))
        .child(field("Description", description))
        .child(field("Icon", icon))
        .child(field("Pipeline template (novel, comic, galgame)", template))
        .child(field("Extensions", extensions));
    body = body
        .child(dynamic(move || {
            changed.get();
            let data = fields.with(|v| v.clone());
            let mut list = stack();
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
                    .child(if semantic {
                        label(&key).into_element()
                    } else {
                        editor("Field key", key, "key").into_element()
                    })
                    .child(editor("Field label", f.label, "label"));
                if !semantic {
                    line = line
                        .child(editor("Field type", f.field_type, "type"))
                        .child(editor(
                            "Enum options (JSON array)",
                            f.options.unwrap_or_default(),
                            "options",
                        ));
                }
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
                    line = line.child(box_view);
                }
                if !semantic {
                    line = line.child(confirm(
                        "Remove field and stored values on save",
                        move || {
                            fields.with(|v| v.remove(i));
                            changed.update(|v| *v += 1);
                        },
                    ));
                }
                if i > 0 {
                    line = line.child(button("Move up", move || {
                        fields.with(|v| v.swap(i, i - 1));
                        changed.update(|v| *v += 1);
                    }));
                }
                list = list.child(line);
            }
            list
        }))
        .child(button("Add field", move || {
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
    body = body.child(dynamic(move || {
        changed.get();
        let mut list = stack();
        for (i, step) in steps.with(|v| v.clone()).iter().enumerate() {
            let (v, check) = check(&step.label, step.enabled);
            effect(move || {
                let enabled = v.get();
                steps.with(|steps| steps[i].enabled = enabled);
            });
            let mut line = row().child(check);
            if i > 0 {
                line = line.child(button("Move step up", move || {
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
    body = body.child(primary("Save schema", save));
    if existing {
        body = body.child(button("Export schema", move || {
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
            body = body.child(confirm("Delete schema", move || {
                app.local_job(
                    admin::schema_delete(app.db(), slug.get_untracked()),
                    move |_| {
                        selection.set(None);
                        app.refresh();
                    },
                )
            }));
        }
    }
    body.into_element()
}
