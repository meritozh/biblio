use crate::{app::App, search::Chip, ui::*};
use biblio_core::commands::{file::FilterCondition, settings as prefs};
use nsh_core::prelude::*;
use std::collections::BTreeMap;

const OPS: &[(&str, &str, &[&str])] = &[
    (
        "Authors",
        "authors",
        &["includes", "empty", "not_empty", "count_gte", "count_lt"],
    ),
    (
        "Tags",
        "tags",
        &[
            "includes",
            "excludes",
            "includes_any",
            "excludes_any",
            "empty",
            "not_empty",
            "count_gte",
            "count_lt",
        ],
    ),
    ("Progress", "progress", &["contains", "empty", "not_empty"]),
    ("Name length", "display_name", &["length_gte", "length_lt"]),
    ("Status", "file_status", &["is"]),
    ("Storage", "storage_kind", &["is"]),
    ("Cached", "local_cache", &["empty", "not_empty"]),
    ("Favorite", "favorite", &["is"]),
    (
        "Custom field",
        "custom",
        &["contains", "is", "empty", "not_empty"],
    ),
];
type Presets = BTreeMap<String, Vec<FilterCondition>>;
pub fn editor(app: App, chips: Signal<Vec<Chip>>) -> AnyElement {
    let opened = signal(false);
    stack()
        .child(button("Filters", move || opened.update(|v| *v = !*v)))
        .child(when(
            move || opened.get(),
            move || {
                let field = signal(0usize);
                let op = signal("includes".to_owned());
                let value = signal(String::new());
                let key = signal(String::new());
                let mut fields = wrap();
                for (i, (name, _, ops)) in OPS.iter().enumerate() {
                    fields = fields.child(button(*name, move || {
                        field.set(i);
                        op.set(ops[0].into());
                        value.set(String::new());
                    }));
                }
                stack()
                    .child(fields)
                    .child(dynamic(move || {
                        let mut row = wrap();
                        for o in OPS[field.get()].2 {
                            row =
                                row.child(button(o.replace('_', " "), move || op.set((*o).into())));
                        }
                        row
                    }))
                    .child(
                        text(move || {
                            format!("{} · {}", OPS[field.get()].0, op.get().replace('_', " "))
                        })
                        .text_color(ink),
                    )
                    .child(when(
                        move || OPS[field.get()].1 == "custom",
                        move || input("Metadata key", key),
                    ))
                    .child(dynamic(move || {
                        let f = OPS[field.get()].1;
                        let op = op.get();
                        if matches!(op.as_str(), "empty" | "not_empty") {
                            return view().into_element();
                        }
                        let options: &[&str] = match f {
                            "file_status" => &["available", "missing", "moved"],
                            "storage_kind" => &["local", "remote"],
                            "favorite" => &["true", "false"],
                            _ => &[],
                        };
                        let mut line = wrap().child(input(
                            match (f, op.as_str()) {
                                ("authors", "includes") => "Author ID",
                                ("tags", "includes" | "excludes") => "Tag ID",
                                ("tags", "includes_any" | "excludes_any") => {
                                    "Tag IDs, comma-separated"
                                }
                                _ => "Value",
                            },
                            value,
                        ));
                        for opt in options {
                            line = line.child(button(*opt, move || value.set((*opt).into())));
                        }
                        line.into_element()
                    }))
                    .child(button("Add condition", move || {
                        match condition(
                            OPS[field.get_untracked()].1,
                            op.get_untracked(),
                            value.get_untracked(),
                            key.get_untracked(),
                        ) {
                            Ok(c) => chips.update(|v| v.push(chip(c))),
                            Err(e) => app.message.set(e),
                        }
                    }))
                    .child(presets(app, chips))
            },
        ))
        .into_element()
}
pub fn chip(c: FilterCondition) -> Chip {
    let v = c
        .text
        .clone()
        .or_else(|| {
            c.value.as_ref().map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| v.to_string())
            })
        })
        .or_else(|| c.tag_id.or(c.author_id).or(c.n).map(|n| n.to_string()))
        .or_else(|| {
            c.tag_ids.as_ref().map(|ids| {
                ids.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
        })
        .unwrap_or_default();
    Chip(
        format!(
            "{} {} {}",
            c.key.as_ref().unwrap_or(&c.field),
            c.op.replace('_', " "),
            v
        )
        .trim()
        .to_owned(),
        c,
    )
}
fn condition(field: &str, op: String, raw: String, key: String) -> Result<FilterCondition, String> {
    let mut c = FilterCondition {
        field: field.into(),
        op,
        ..Default::default()
    };
    let value = raw.trim();
    if field == "custom" {
        if key.trim().is_empty() {
            return Err("Enter a metadata key".into());
        }
        c.key = Some(key.trim().into());
    }
    match c.op.as_str() {
        "empty" | "not_empty" => {}
        "count_gte" | "count_lt" | "length_gte" | "length_lt" => {
            c.n = Some(
                value
                    .parse::<i64>()
                    .ok()
                    .filter(|v| *v >= 0)
                    .ok_or("Enter a nonnegative count")?,
            )
        }
        "includes" | "excludes" => {
            let id = value
                .parse::<i64>()
                .ok()
                .filter(|v| *v > 0)
                .ok_or("Enter a positive ID")?;
            if field == "authors" {
                c.author_id = Some(id)
            } else {
                c.tag_id = Some(id)
            }
        }
        "includes_any" | "excludes_any" => {
            c.tag_ids = Some(
                value
                    .split(',')
                    .map(|s| {
                        s.trim()
                            .parse::<i64>()
                            .ok()
                            .filter(|v| *v > 0)
                            .ok_or("Enter tag IDs separated by commas")
                    })
                    .collect::<Result<_, _>>()?,
            )
        }
        _ if value.is_empty() => return Err("Enter a filter value".into()),
        _ if field == "favorite" => {
            c.value = Some(serde_json::json!(
                value.parse::<bool>().map_err(|_| "Choose true or false")?
            ))
        }
        _ if field == "file_status" && !["available", "missing", "moved"].contains(&value) => {
            return Err("Choose a status".into());
        }
        _ if field == "storage_kind" && !["local", "remote"].contains(&value) => {
            return Err("Choose a storage kind".into());
        }
        _ if matches!(field, "file_status" | "storage_kind") => c.value = Some(value.into()),
        _ => c.text = Some(value.into()),
    }
    Ok(c)
}
fn presets(app: App, chips: Signal<Vec<Chip>>) -> AnyElement {
    let name = signal(String::new());
    let revision = signal(0u64);
    dynamic(move || {
        revision.get();
        let db = app.db();
        app.load(
            async move {
                prefs::settings_get(db, "native_saved_filters".into())
                    .await?
                    .map(|v| serde_json::from_str::<Presets>(&v).map_err(|e| e.to_string()))
                    .unwrap_or_else(|| Ok(Presets::new()))
            },
            move |data| {
                let data = stored(data.clone());
                let write = move |presets: Presets| {
                    let value = serde_json::to_string(&presets).expect("conditions serialize");
                    app.local_job(
                        prefs::settings_set(app.db(), "native_saved_filters".into(), value),
                        move |_| revision.update(|n| *n += 1),
                    );
                };
                let mut s = stack().child(row().child(input("Preset name", name)).child(button(
                    "Save current filter",
                    move || {
                        let name = name.get_untracked().trim().to_owned();
                        let conditions = chips
                            .get_untracked()
                            .into_iter()
                            .map(|c| c.1)
                            .collect::<Vec<_>>();
                        if name.is_empty() || conditions.is_empty() {
                            app.message
                                .set("Choose a name and at least one condition".into());
                            return;
                        }
                        let mut all = data.with(|v| v.clone());
                        all.insert(name, conditions);
                        write(all);
                    },
                )));
                for (name, conditions) in data.with(|v| v.clone()) {
                    s = s.child(
                        row()
                            .child(button(&name, move || {
                                chips.set(conditions.iter().cloned().map(chip).collect())
                            }))
                            .child(confirm("Delete preset", move || {
                                let mut all = data.with(|v| v.clone());
                                all.remove(&name);
                                write(all);
                            })),
                    );
                }
                s.into_element()
            },
        )
    })
    .into_element()
}
