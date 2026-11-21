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
pub fn screen(app: App) -> AnyElement {
    scroll()
        .flex_1()
        .min_h_0()
        .child(stack().child(heading("Settings")).child(dynamic(move || {
            app.revision.get();
            let db = app.db();
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
                move |(path, mode, scheme, config, remote, stats)| {
                    let root = signal(path.clone());
                    let mut modes = wrap().child(label(format!("Import mode · {mode}")));
                    for mode in ["copy", "move"] {
                        modes = modes.child(button(mode, move || {
                            app.save(prefs::settings_set(
                                app.db(),
                                "import_mode".into(),
                                mode.into(),
                            ))
                        }));
                    }
                    let scheme = signal(scheme.clone());
                    let mut themes = wrap().child(label("Mode"));
                    for mode in ["system", "light", "dark"] {
                        themes = themes.child(choice(
                            mode,
                            move || scheme.with(|s| s == mode),
                            move || {
                                app.local_job(
                                    prefs::settings_set(app.db(), "theme".into(), mode.into()),
                                    move |_| {
                                        set_color_scheme(appearance(mode));
                                        scheme.set(mode.into());
                                    },
                                )
                            },
                        ));
                    }
                    let mut palettes = wrap().child(label("Palette"));
                    let palette = use_context::<Palette>().expect("workspace palette");
                    for (key, name) in [("porcelain", "Porcelain"), ("graphite", "Graphite")] {
                        palettes = palettes.child(choice(
                            name,
                            move || palette.0.get() == (key == "graphite"),
                            move || {
                                app.job(
                                    prefs::settings_set(
                                        app.db(),
                                        "design-theme".into(),
                                        key.into(),
                                    ),
                                    move |_| palette.0.set(key == "graphite"),
                                )
                            },
                        ));
                    }
                    let (enabled, on) = check("Use LLM on import", config.enabled);
                    let (analyze, content) = check("Analyze file contents", config.analyze_content);
                    let url = signal(config.base_url.clone());
                    let model = signal(config.model.clone());
                    let secret = signal(config.api_key.clone());
                    stack()
                        .max_w(px(760.))
                        .child(section("Storage"))
                        .child(field("Storage folder", root))
                        .child(
                            wrap()
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
                        )
                        .child(modes)
                        .child(section("Appearance"))
                        .child(themes)
                        .child(palettes)
                        .child(section("Language model"))
                        .child(on)
                        .child(content)
                        .child(field("Base URL", url))
                        .child(field("Model", model))
                        .child(input("API key", secret).masked(true))
                        .child(
                            wrap()
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
                                    app.local_job(
                                        llm::llm_test_connection(app.db()),
                                        move |result| app.message.set(result),
                                    )
                                })),
                        )
                        .child(remote_form(app, remote))
                        .child(section("Database"))
                        .child(label(format!(
                            "{} files · {} tags · {} categories · {} bytes",
                            stats.file_count,
                            stats.tag_count,
                            stats.category_count,
                            stats.size_bytes
                        )))
                        .child(
                            wrap()
                                .child(button("Check integrity", move || {
                                    app.local_job(
                                        recovery::db_check_integrity(app.db()),
                                        move |r| app.message.set(r.message),
                                    )
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
                },
            )
        })))
        .into_element()
}
fn remote_form(app: App, config: &remote::RemoteConfig) -> AnyElement {
    let mut s = stack().child(section("Baidu Netdisk"));
    if config.enabled {
        let key = signal(None::<String>);
        s = s
            .child(label(format!("Connected · {}", config.app_root)))
            .child(confirm("Disconnect", move || {
                app.save(remote::remote_logout(app.db()))
            }))
            .child(button("Show / hide recovery key", move || {
                if key.get_untracked().is_some() {
                    key.set(None);
                } else {
                    app.local_job(remote::remote_recovery_key(app.db()), move |value| {
                        key.set(Some(value))
                    });
                }
            }))
            .child(dynamic(move || {
                key.get().map(|k| {
                    nsh_base::text_view()
                        .markdown(k)
                        .selectable(true)
                        .text_color(ink)
                })
            }))
            .child(app.load(remote::remote_legacy_count(app.db()), move |n| {
                if *n > 0 {
                    confirm(format!("Encrypt {n} legacy remote files"), move || {
                        app.local_job(remote::file_reencrypt_legacy(app.db()), move |n| {
                            app.message.set(format!("Queued {n} files"))
                        })
                    })
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
            .child(input("Access token or redirect URL", token).masked(true))
            .child(field("Remote folder", root))
            .child(button("Connect", move || {
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
