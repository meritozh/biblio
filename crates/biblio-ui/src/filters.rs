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
/// The Filters button and its popover: build a condition (field,
/// operator, value), add it, and keep named presets.
pub fn editor(app: App, chips: Signal<Vec<Chip>>) -> AnyElement {
    let opener = trigger("Filters", Some(Icon::Filter), true, false).child(dynamic(move || {
        let n = chips.with(Vec::len);
        if n == 0 {
            return view().hidden().into_element();
        }
        view()
            .min_w(px(18.))
            .h(px(18.))
            .px(px(5.))
            .items_center()
            .justify_center()
            .rounded(px(9.))
            .bg(accent)
            .child(
                text(n.to_string())
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(on_accent),
            )
            .into_element()
    }));
    popover(opener, move || {
        let field = signal(0usize);
        let op = signal("includes".to_owned());
        let value = signal(String::new());
        let key = signal(String::new());
        let fields = OPS
            .iter()
            .enumerate()
            .map(|(i, (name, _, _))| (i.to_string(), name.to_string()))
            .collect::<Vec<_>>();
        view()
            .gap_3()
            .child(subheading("Add a condition"))
            .child(view().gap_1().child(field_label("Field")).child(select(
                "Field",
                fields,
                move || Some(field.get().to_string()),
                move |v| {
                    let i = v.parse().unwrap_or(0);
                    field.set(i);
                    op.set(OPS[i].2[0].into());
                    value.set(String::new());
                },
            )))
            .child(dynamic(move || {
                let mut ops = wrap().gap(px(6.));
                for o in OPS[field.get()].2 {
                    ops = ops.child(choice(
                        op_caption(o),
                        move || op.with(|v| v == o),
                        move || op.set((*o).into()),
                    ));
                }
                view().gap_1().child(field_label("Condition")).child(ops)
            }))
            .child(when(
                move || OPS[field.get()].1 == "custom",
                move || crate::ui::field("Metadata key", key),
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
                let name = match (f, op.as_str()) {
                    ("authors", "includes") => "Author ID",
                    ("tags", "includes" | "excludes") => "Tag ID",
                    ("tags", "includes_any" | "excludes_any") => "Tag IDs, comma-separated",
                    _ => "Value",
                };
                if options.is_empty() {
                    return crate::ui::field(name, value).into_element();
                }
                let mut line = wrap().gap(px(6.));
                for opt in options {
                    line = line.child(choice(
                        *opt,
                        move || value.with(|v| v == opt),
                        move || value.set((*opt).into()),
                    ));
                }
                view()
                    .gap_1()
                    .child(field_label(name))
                    .child(line)
                    .into_element()
            }))
            .child(control(
                "Add condition",
                Variant::Primary,
                ButtonSize::Sm,
                Some(Icon::Plus),
                move || match condition(
                    OPS[field.get_untracked()].1,
                    op.get_untracked(),
                    value.get_untracked(),
                    key.get_untracked(),
                ) {
                    Ok(c) => {
                        chips.update(|v| v.push(chip(c)));
                        value.set(String::new());
                    }
                    Err(e) => app.message.set(e),
                },
            ))
            .child(separator())
            .child(presets(app, chips))
            .into_element()
    })
    .into_element()
}
/// How an operator reads in the condition builder.
fn op_caption(op: &str) -> &'static str {
    match op {
        "includes" => "Includes",
        "excludes" => "Excludes",
        "includes_any" => "Includes any",
        "excludes_any" => "Excludes any",
        "empty" => "Is empty",
        "not_empty" => "Is not empty",
        "count_gte" => "At least",
        "count_lt" => "Fewer than",
        "length_gte" => "At least",
        "length_lt" => "Shorter than",
        "contains" => "Contains",
        "is" => "Is",
        _ => "Matches",
    }
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
                let mut list = view().gap(px(1.));
                for (preset, conditions) in data.with(|v| v.clone()) {
                    let doomed = preset.clone();
                    list = list.child(
                        row()
                            .gap_1()
                            .child(
                                control(
                                    preset.clone(),
                                    Variant::Ghost,
                                    ButtonSize::Sm,
                                    None,
                                    move || {
                                        chips.set(conditions.iter().cloned().map(chip).collect())
                                    },
                                )
                                .flex_1()
                                .justify_start(),
                            )
                            .child(icon_button(
                                format!("Delete preset {preset}"),
                                Icon::Trash,
                                None,
                                move || {
                                    let doomed = doomed.clone();
                                    ask(
                                        format!("Delete the preset “{doomed}”?"),
                                        "The filter conditions it saved are forgotten.",
                                        "Delete",
                                        true,
                                        move || {
                                            let mut all = data.with(|v| v.clone());
                                            all.remove(&doomed);
                                            write(all);
                                        },
                                    )
                                },
                            )),
                    );
                }
                view()
                    .gap_2()
                    .child(subheading("Presets"))
                    .child(list)
                    .child(
                        row()
                            .gap_2()
                            .child(input("Preset name", name).flex_1())
                            .child(small("Save current filter", move || {
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
                            })),
                    )
                    .into_element()
            },
        )
    })
    .into_element()
}
