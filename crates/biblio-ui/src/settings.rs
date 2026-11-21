use crate::{app::App, ui::*};
use biblio_core::{
    commands::{llm, remote, settings as prefs},
    database::recovery,
};
use nsh_core::prelude::*;

pub fn appearance(value: &str) -> Appearance {
    match value {
        "dark" => Appearance::Dark,
        "light" => Appearance::Light,
        _ => Appearance::System,
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    General,
    Model,
    Remote,
    Database,
}

fn tab_strip(current: Signal<Tab>) -> impl IntoElement {
    let mut strip = nsh_base::tabs()
        .flex_row()
        .gap_1()
        .border_b_1()
        .border_color(rule)
        .indicator_style(|s| {
            s.bottom(px(-1.))
                .h(px(2.))
                .bg(ink)
                .transition()
                .duration(tween(BASE))
                .ease_in_out()
        });
    for (tab, name) in [
        (Tab::General, "General"),
        (Tab::Model, "Language model"),
        (Tab::Remote, "Baidu Netdisk"),
        (Tab::Database, "Database"),
    ] {
        let on = move || current.get() == tab;
        strip = strip.child(
            nsh_base::tab()
                .selected(on)
                .accessibility_label(name)
                .on_press(move || current.set(tab))
                .px(px(10.))
                .py(px(8.))
                .rounded(px(RADIUS_SM))
                .hover(|s| s.bg(wash))
                .focus(|s| s.outline(px(2.)).outline_color(accent))
                .child(label(name).style_with(move |s| {
                    if on() {
                        s.text_color(ink).font_weight(FontWeight::SEMIBOLD)
                    } else {
                        s.text_color(secondary).font_weight(FontWeight::MEDIUM)
                    }
                })),
        );
    }
    strip
}

pub fn screen(app: App) -> AnyElement {
    let tab = signal(Tab::General);
    let body = dynamic(move || {
        app.revision.get();
        let db = app.db();
        let current = tab.get();
        app.load(
            async move {
                Ok((
                    prefs::storage_get_path(db.clone())
                        .await?
                        .unwrap_or_default(),
                    prefs::settings_get(db.clone(), "import_mode".into())
                        .await?
                        .unwrap_or_else(|| "move".into()),
                    prefs::settings_get(db.clone(), "theme".into())
                        .await?
                        .unwrap_or_else(|| "system".into()),
                    llm::llm_config_get(db.clone()).await?,
                    remote::remote_config_get(db.clone()).await?,
                    recovery::db_get_stats(db).await?,
                ))
            },
            move |(path, mode, scheme, config, remote, stats)| match current {
                Tab::General => general(app, path, mode, scheme),
                Tab::Model => model(app, config),
                Tab::Remote => remote_form(app, remote),
                Tab::Database => database(app, stats),
            },
        )
    });
    page(
        heading(Icon::Settings, "Settings"),
        page_scroll(MEASURE, view().gap_5().child(tab_strip(tab)).child(body)),
    )
    .into_element()
}

fn general(app: App, path: &str, mode: &str, scheme: &str) -> AnyElement {
    let root = signal(path.to_owned());
    let copy = mode == "copy";
    let scheme = signal(scheme.to_owned());
    let palette = use_context::<Palette>().expect("workspace palette");
    view()
        .gap_2()
        .child(subheading("Storage"))
        .child(
            view().gap_2().child(field("Storage folder", root)).child(
                row()
                    .gap_2()
                    .child(button("Browse", move || {
                        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                            root.set(dir.to_string_lossy().into_owned());
                        }
                    }))
                    .child(primary("Save storage folder", move || {
                        app.save(prefs::settings_set(
                            app.db(),
                            "storage_path".into(),
                            root.get_untracked(),
                        ))
                    })),
            ),
        )
        .child(setting(
            "Import mode",
            Some(if copy {
                "Copies imported files into the storage folder and leaves the originals in place."
            } else {
                "Moves imported files into the storage folder."
            }),
            segmented(
                "Import mode",
                vec![Segment::text("copy", "Copy"), Segment::text("move", "Move")],
                move || if copy { "copy" } else { "move" },
                move |key| {
                    app.save(prefs::settings_set(
                        app.db(),
                        "import_mode".into(),
                        key.into(),
                    ))
                },
            ),
        ))
        .child(section("Appearance"))
        .child(setting(
            "Mode",
            Some("System follows the computer's light or dark appearance."),
            segmented(
                "Mode",
                vec![
                    Segment::text("system", "System"),
                    Segment::text("light", "Light"),
                    Segment::text("dark", "Dark"),
                ],
                move || match scheme.get().as_str() {
                    "light" => "light",
                    "dark" => "dark",
                    _ => "system",
                },
                move |mode| {
                    app.local_job(
                        prefs::settings_set(app.db(), "theme".into(), mode.into()),
                        move |_| {
                            set_color_scheme(appearance(mode));
                            scheme.set(mode.into());
                        },
                    )
                },
            ),
        ))
        .child(setting(
            "Palette",
            Some("Porcelain is cool with an ink-blue accent; Graphite is warm and monochrome."),
            segmented(
                "Palette",
                vec![
                    Segment::text("porcelain", "Porcelain"),
                    Segment::text("graphite", "Graphite"),
                ],
                move || {
                    if palette.0.get() {
                        "graphite"
                    } else {
                        "porcelain"
                    }
                },
                move |key| {
                    app.job(
                        prefs::settings_set(app.db(), "design-theme".into(), key.into()),
                        move |_| palette.0.set(key == "graphite"),
                    )
                },
            ),
        ))
        .into_element()
}

fn model(app: App, config: &llm::LlmConfig) -> AnyElement {
    let (enabled, on) = check("Use LLM on import", config.enabled);
    let (analyze, content) = check("Analyze file contents", config.analyze_content);
    let url = signal(config.base_url.clone());
    let model = signal(config.model.clone());
    let secret = signal(config.api_key.clone());
    view()
        .gap_3()
        .child(note("An OpenAI-compatible endpoint suggests titles, authors and tags while files are analyzed."))
        .child(view().gap_1().child(on).child(content))
        .child(field("Base URL", url))
        .child(field("Model", model))
        .child(
            view()
                .gap_1()
                .child(field_label("API key"))
                .child(input("API key", secret).masked(true)),
        )
        .child(
            row()
                .gap_2()
                .child(primary("Save LLM settings", move || {
                    app.save(llm::llm_config_set(
                        app.db(),
                        llm::LlmConfig {
                            enabled: enabled.get_untracked(),
                            base_url: url.get_untracked(),
                            api_key: secret.get_untracked(),
                            model: model.get_untracked(),
                            analyze_content: analyze.get_untracked(),
                        },
                    ))
                }))
                .child(button("Test saved connection", move || {
                    app.local_job(llm::llm_test_connection(app.db()), move |result| {
                        app.message.set(result)
                    })
                })),
        )
        .into_element()
}

fn database(app: App, stats: &recovery::DatabaseStats) -> AnyElement {
    let figure = |n: String, what: &str| {
        view()
            .gap(px(2.))
            .min_w(px(120.))
            .child(
                text(n)
                    .font_family(DISPLAY_FACE)
                    .text_size(px(22.))
                    .text_color(ink),
            )
            .child(caption(what.to_owned()))
    };
    view()
        .gap_3()
        .child(
            wrap()
                .gap_x(px(32.))
                .gap_y(px(12.))
                .py_2()
                .child(figure(stats.file_count.to_string(), "files"))
                .child(figure(stats.tag_count.to_string(), "tags"))
                .child(figure(stats.category_count.to_string(), "categories"))
                .child(figure(format_bytes(stats.size_bytes), "on disk")),
        )
        .child(
            wrap()
                .gap_2()
                .child(button("Check integrity", move || {
                    app.local_job(recovery::db_check_integrity(app.db()), move |r| {
                        app.message.set(r.message)
                    })
                }))
                .child(button("Create backup", move || {
                    app.local_job(recovery::db_create_backup(app.db()), move |r| {
                        app.message.set(r.message)
                    })
                }))
                .child(button("Optimize database", move || {
                    app.local_job(recovery::db_optimize(app.db()), move |r| {
                        app.message.set(r.message)
                    })
                })),
        )
        .into_element()
}

fn format_bytes(n: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1024. && unit < UNITS.len() - 1 {
        value /= 1024.;
        unit += 1;
    }
    if unit == 0 {
        format!("{n} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn remote_form(app: App, config: &remote::RemoteConfig) -> AnyElement {
    let mut s = view().gap_3().child(note(
        "Files moved to remote storage are encrypted on this computer before upload.",
    ));
    if config.enabled {
        let key = signal(None::<String>);
        s = s
            .child(setting(
                "Connected",
                Some("Remote folder"),
                label(config.app_root.clone()),
            ))
            .child(
                row()
                    .gap_2()
                    .child(button("Show / hide recovery key", move || {
                        if key.get_untracked().is_some() {
                            key.set(None);
                        } else {
                            app.local_job(remote::remote_recovery_key(app.db()), move |value| {
                                key.set(Some(value))
                            });
                        }
                    }))
                    .child(confirm(
                        "Disconnect",
                        "Disconnect Baidu Netdisk?",
                        "Remote files stay on Baidu Netdisk, but Biblio cannot open them until you connect again.",
                        true,
                        move || app.save(remote::remote_logout(app.db())),
                    )),
            )
            .child(dynamic(move || {
                key.get().map(|k| {
                    view()
                        .gap_1()
                        .p_3()
                        .rounded(px(RADIUS))
                        .bg(rail)
                        .child(caption("Recovery key · keep it safe; without it remote files cannot be decrypted."))
                        .child(
                            nsh_base::text_view()
                                .markdown(k)
                                .selectable(true)
                                .text_color(ink),
                        )
                })
            }))
            .child(app.load(remote::remote_legacy_count(app.db()), move |n| {
                let n = *n;
                if n > 0 {
                    confirm(
                        format!("Encrypt {n} legacy remote files"),
                        format!("Encrypt {n} legacy remote files?"),
                        "Each file is downloaded, encrypted on this computer and uploaded again.",
                        false,
                        move || {
                            app.local_job(remote::file_reencrypt_legacy(app.db()), move |n| {
                                app.message.set(format!("Queued {n} files"))
                            })
                        },
                    )
                    .into_element()
                } else {
                    view().into_element()
                }
            }));
    } else {
        let key = signal(config.app_key.clone());
        let token = signal(String::new());
        let root = signal(config.app_root.clone());
        s = s
            .child(field("App key", key))
            .child(button("Authorize in browser", move || {
                app.local_job(
                    remote::remote_get_authorize_url(key.get_untracked()),
                    move |url| {
                        if let Err(e) = open::that_detached(url) {
                            app.message.set(e.to_string());
                        }
                    },
                )
            }))
            .child(
                view()
                    .gap_1()
                    .child(field_label("Access token or redirect URL"))
                    .child(input("Access token or redirect URL", token).masked(true)),
            )
            .child(field("Remote folder", root))
            .child(primary("Connect", move || {
                match parse_token(&token.get_untracked()) {
                    Ok((t, expiry)) => app.save(remote::remote_login(
                        app.db(),
                        key.get_untracked(),
                        t,
                        expiry,
                        Some(root.get_untracked()),
                    )),
                    Err(e) => app.message.set(e),
                }
            }));
    }
    s.into_element()
}
fn parse_token(raw: &str) -> Result<(String, i64), String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err("Enter a token or redirect URL".into());
    }
    if let Ok(url) = url::Url::parse(value) {
        let fields = url::form_urlencoded::parse(url.fragment().unwrap_or_default().as_bytes())
            .collect::<std::collections::HashMap<_, _>>();
        let t = fields
            .get("access_token")
            .filter(|s| !s.is_empty())
            .ok_or("Redirect URL has no access token")?;
        let expiry = fields
            .get("expires_in")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(2592000);
        Ok((t.to_string(), expiry))
    } else {
        Ok((value.into(), 2592000))
    }
}
