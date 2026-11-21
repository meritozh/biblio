use crate::{admin, catalog, detail, imports, settings, ui::*};
use biblio_core::{
    Library,
    commands::{category::category_list, processing::FilePreparedImport},
};
use nsh_core::{
    desktop::{MenuRole, TitleBarStyle},
    element::desktop::{MenuItem, set_application_menu, set_title_bar, title_bar},
    platform::Os,
    prelude::*,
};
use std::{future::Future, path::PathBuf, rc::Rc};
use tokio::sync::mpsc;

// The native engine and the headless renderer invoke components on a UI
// thread. SQLx, reqwest and the long-lived transfer queues share this runtime.
pub fn run_service<T: Send + 'static>(
    future: impl Future<Output = T> + Send + 'static,
) -> tokio::task::JoinHandle<T> {
    static RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_name("biblio-service")
                .build()
                .expect("start service runtime")
        })
        .spawn(future)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Library,
    File(i64),
    Import,
    Categories,
    Tags,
    Authors,
    Schemas,
    Prompts,
    Cleanup,
    Settings,
}
#[derive(Clone, Copy)]
pub struct App {
    pub service: Stored<Library>,

    pub page: Signal<Page>,
    pub category: Signal<Option<i64>>,
    pub revision: Signal<u64>,
    pub message: Signal<String>,
    pub busy: Signal<bool>,
    pub pending: Signal<Vec<FilePreparedImport>>,
    pub analyzing: Signal<Vec<String>>,
    pub sources: Signal<Vec<String>>,
    pub roots: Stored<std::collections::HashMap<String, String>>,
    pub browse: catalog::BrowseState,
    /// Bumped to ask the library's search field for the keyboard focus.
    pub find: Signal<u64>,
    /// The last request a search field took, so one made before the field
    /// existed (from another page) is taken when it appears.
    pub found: Stored<u64>,
    owner: Owner,
}
impl App {
    pub fn db(self) -> Library {
        self.service.with(|s| s.clone())
    }
    pub fn refresh(self) {
        self.revision.update(|n| *n += 1);
    }
    pub fn job<T: Send + 'static>(
        self,
        f: impl Future<Output = Result<T, String>> + Send + 'static,
        done: impl FnOnce(T) + 'static,
    ) {
        if self.busy.get_untracked() {
            return;
        }
        self.busy.set(true);
        self.message.set("Working…".into());
        let task = run_service(f);
        self.owner.run(|| {
            spawn(async move {
                let result = task.await.map_err(|e| e.to_string()).and_then(|r| r);
                self.busy.set(false);
                match result {
                    Ok(value) => {
                        self.message.set(String::new());
                        done(value);
                    }
                    Err(e) => self.message.set(e),
                }
            })
        });
    }
    pub fn local_job<T: Send + 'static>(
        self,
        f: impl Future<Output = Result<T, String>> + Send + 'static,
        done: impl FnOnce(T) + 'static,
    ) {
        let owner = Owner::current();
        self.job(f, move |value| {
            if owner.is_live() {
                owner.run(|| done(value))
            } else {
                self.refresh()
            }
        });
    }
    pub fn save<T: Send + 'static>(
        self,
        f: impl Future<Output = Result<T, String>> + Send + 'static,
    ) {
        self.job(f, move |_| {
            self.message.set("Saved".into());
            self.refresh();
        });
    }
    pub fn load<T: Send + 'static>(
        self,
        f: impl Future<Output = Result<T, String>> + Send + 'static,
        render: impl Fn(&T) -> AnyElement + 'static,
    ) -> AnyElement {
        let result = signal_with(None::<Rc<Result<T, String>>>, SignalEquals::Never);
        let task = run_service(f);
        let abort = task.abort_handle();
        cleanup(move || abort.abort());
        spawn(async move {
            result.set(Some(Rc::new(
                task.await.map_err(|e| e.to_string()).and_then(|r| r),
            )));
        });
        dynamic(move || match result.get().as_deref() {
            None => label("Loading…").into_element(),
            Some(Err(e)) => label(e.clone()).into_element(),
            Some(Ok(data)) => untracked(|| render(data)),
        })
        .into_element()
    }
}

pub fn start(dir: PathBuf) -> AnyElement {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let open = run_service(async move {
        use biblio_core::commands::settings::settings_get;
        let db = Library::open(dir, move |name, data| {
            let _ = tx.send((name, data));
        })
        .await?;
        let scheme = settings_get(db.clone(), "theme".into())
            .await?
            .unwrap_or_default();
        let palette = settings_get(db.clone(), "design-theme".into())
            .await?
            .unwrap_or_default();
        Ok((db, scheme, palette))
    });
    let ready = signal_with(
        None::<Result<(Library, String, String), String>>,
        SignalEquals::Never,
    );
    spawn(async move {
        ready.set(Some(open.await.map_err(|e| e.to_string()).and_then(|r| r)));
    });
    let pending = signal_with(Vec::<FilePreparedImport>::new(), SignalEquals::Never);
    let analyzing = signal(Vec::<String>::new());
    let sources = signal(Vec::<String>::new());
    let revision = signal(0u64);
    let message = signal(String::new());
    spawn(async move {
        while let Some((name, data)) = rx.recv().await {
            if name == "file-prepared" {
                match serde_json::from_value::<FilePreparedImport>(data) {
                    Ok(file) if analyzing.get_untracked().contains(&file.path) => {
                        analyzing.update(|paths| paths.retain(|p| p != &file.path));
                        pending.update(|items| items.push(file));
                    }
                    Err(e) => message.set(e.to_string()),
                    _ => {}
                }
            } else if name == "import-analysis-failed" {
                let status = data["status"].as_str().unwrap_or_default();
                if let Some(path) = data["current_file"].as_str() {
                    if analyzing.get_untracked().iter().any(|p| p == path) {
                        analyzing.update(|paths| paths.retain(|p| p != path));
                        message.set(format!("{path} · {status}"));
                    }
                }
            } else if name.starts_with("remote-") {
                let status = data["status"].as_str().unwrap_or_default();
                message.set(format!(
                    "{} · {}",
                    data["file_name"].as_str().unwrap_or_default(),
                    data["error"].as_str().unwrap_or(status)
                ));
                if status == "success" {
                    revision.update(|n| *n += 1);
                }
            }
        }
    });
    view()
        .w_full()
        .h(move || window().height)
        .child(dynamic(move || match ready.get() {
            None => title("Opening Biblio…").into_element(),
            Some(Err(e)) => stack()
                .p_4()
                .child(title("Could not open the library"))
                .child(label(e))
                .into_element(),
            Some(Ok((db, scheme, palette))) => untracked(|| {
                set_color_scheme(settings::appearance(&scheme));
                provide_context(Palette(signal(palette == "graphite")));
                workspace(App {
                    service: stored(db),
                    browse: catalog::BrowseState::new(),
                    find: signal(0),
                    found: stored(0),
                    page: signal(Page::Library),
                    category: signal(None),
                    revision,
                    message,
                    busy: signal(false),
                    pending,
                    analyzing,
                    sources,
                    roots: stored(std::collections::HashMap::new()),
                    owner: Owner::current(),
                })
            }),
        }))
        .into_element()
}
/// The window chrome (macOS): the system title bar folds into the app so
/// the rail runs to the top edge under the window controls, and the menu
/// bar carries the app's commands with their shortcuts.
fn chrome(app: App, menu: Signal<bool>) {
    if os() != Os::MacOs {
        return;
    }
    set_title_bar(TitleBarStyle::Hidden, Some(px(52.)));
    let go = move |page: Page| {
        menu.set(false);
        app.page.set(page);
    };
    set_application_menu(vec![
        MenuItem::submenu(
            "Biblio",
            [
                MenuItem::role(MenuRole::About),
                MenuItem::separator(),
                MenuItem::new("Settings…")
                    .shortcut("mod+,")
                    .on_select(move || go(Page::Settings)),
                MenuItem::separator(),
                MenuItem::role(MenuRole::Hide),
                MenuItem::role(MenuRole::HideOthers),
                MenuItem::role(MenuRole::ShowAll),
                MenuItem::separator(),
                MenuItem::role(MenuRole::Quit),
            ],
        ),
        MenuItem::submenu(
            "File",
            [
                MenuItem::new("Import Files…")
                    .shortcut("mod+i")
                    .on_select(move || go(Page::Import)),
                MenuItem::separator(),
                MenuItem::role(MenuRole::Close),
            ],
        ),
        MenuItem::submenu(
            "Edit",
            [
                MenuItem::role(MenuRole::Undo),
                MenuItem::role(MenuRole::Redo),
                MenuItem::separator(),
                MenuItem::role(MenuRole::Cut),
                MenuItem::role(MenuRole::Copy),
                MenuItem::role(MenuRole::Paste),
                MenuItem::role(MenuRole::SelectAll),
                MenuItem::separator(),
                MenuItem::new("Find in Library")
                    .shortcut("mod+f")
                    .on_select(move || find(app, menu)),
            ],
        ),
        MenuItem::submenu(
            "Go",
            [
                MenuItem::new("All Files")
                    .shortcut("mod+1")
                    .on_select(move || {
                        app.category.set(None);
                        go(Page::Library);
                    }),
                MenuItem::new("Back to Library")
                    .shortcut("mod+[")
                    .on_select(move || {
                        if matches!(app.page.get_untracked(), Page::File(_)) {
                            go(Page::Library);
                        }
                    }),
                MenuItem::separator(),
                MenuItem::new("Categories").on_select(move || go(Page::Categories)),
                MenuItem::new("Tags").on_select(move || go(Page::Tags)),
                MenuItem::new("Authors").on_select(move || go(Page::Authors)),
                MenuItem::new("Cleanup").on_select(move || go(Page::Cleanup)),
            ],
        ),
        MenuItem::submenu(
            "Window",
            [
                MenuItem::role(MenuRole::Minimize),
                MenuItem::role(MenuRole::Zoom),
                MenuItem::separator(),
                MenuItem::role(MenuRole::ToggleFullScreen),
            ],
        ),
    ]);
}

/// The height of the band the hidden title bar leaves at the top of the
/// window: what the host reports, else the height asked for on macOS.
fn band() -> Points {
    let reported = title_bar().top;
    if reported > px(0.) {
        reported
    } else if os() == Os::MacOs {
        px(52.)
    } else {
        px(0.)
    }
}
/// The width the window controls take at the band's leading end.
fn controls() -> Points {
    let reported = title_bar().left;
    if reported > px(0.) {
        reported
    } else if os() == Os::MacOs {
        px(78.)
    } else {
        px(0.)
    }
}

/// Opens the library (if elsewhere) and focuses its search field.
fn find(app: App, menu: Signal<bool>) {
    menu.set(false);
    if !matches!(app.page.get_untracked(), Page::Library) {
        app.page.set(Page::Library);
    }
    app.find.update(|n| *n += 1);
}

fn shortcuts(app: App, menu: Signal<bool>) {
    let go = move |page: Page| {
        menu.set(false);
        app.page.set(page);
    };
    on_key("mod+k", move |_| find(app, menu));
    on_key("mod+f", move |_| find(app, menu));
    on_key("mod+i", move |_| go(Page::Import));
    on_key("mod+,", move |_| go(Page::Settings));
    on_key("mod+1", move |_| {
        app.category.set(None);
        go(Page::Library);
    });
    on_key("mod+[", move |_| {
        if matches!(app.page.get_untracked(), Page::File(_)) {
            go(Page::Library);
        }
    });
}

/// The foot of the workspace: what is running, then the latest result.
fn status_bar(app: App) -> View {
    row()
        .min_h(px(28.))
        .px(move || px(if compact() { 16. } else { 40. }))
        .border_t_1()
        .border_color(rule)
        .bg(paper)
        .child(
            text(move || {
                if app.busy.get() {
                    "Working…".to_owned()
                } else {
                    app.message.get()
                }
            })
            .text_size(px(12.))
            .text_color(secondary)
            .number_of_lines(1)
            .flex_1(),
        )
}

fn workspace(app: App) -> AnyElement {
    let menu = signal(false);
    shortcuts(app, menu);
    chrome(app, menu);
    let narrow = computed(compact);
    let page = stack()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .gap_0()
        .px(move || px(if narrow.get() { 16. } else { 40. }))
        .pt(move || {
            px(if narrow.get() {
                16.
            } else if band() > px(0.) {
                4.
            } else {
                28.
            })
        })
        .child(dynamic(move || {
            let page = app.page.get();
            untracked(|| match page {
                Page::Library => catalog::screen(app),
                Page::File(id) => detail::screen(app, id),
                Page::Import => imports::screen(app),
                Page::Settings => settings::screen(app),
                page => admin::screen(app, page),
            })
        }));
    let main = view()
        .style_with(move |s| {
            if narrow.get() && menu.get() {
                s.hidden()
            } else {
                s
            }
        })
        .flex_1()
        .min_w_0()
        .min_h_0()
        .child(when(
            move || !narrow.get() && band() > px(0.),
            move || view().h(band()).flex_shrink_0().window_drag(true),
        ))
        .child(page)
        .child(when(
            move || app.busy.get() || !app.message.with(String::is_empty),
            move || status_bar(app),
        ));
    stack()
        .w_full()
        .h_full()
        .gap_0()
        .bg(paper)
        .child(when(
            move || narrow.get(),
            move || {
                row()
                    .justify_between()
                    .bg(rail)
                    .border_b_1()
                    .border_color(rule)
                    .window_drag(true)
                    .min_h(move || band().max(px(44.)))
                    .pl(move || controls() + px(4.))
                    .pr(move || title_bar().right + px(12.))
                    .py_2()
                    .child(wordmark())
                    .child(nav_button(
                        "Navigation",
                        Icon::Menu,
                        move || menu.get(),
                        move || menu.update(|v| *v = !*v),
                    ))
            },
        ))
        .child(
            row()
                .flex_1()
                .min_h_0()
                .w_full()
                .items_stretch()
                .gap_0()
                .child(when(
                    move || !narrow.get() || menu.get(),
                    move || navigation(app, menu, narrow),
                ))
                .child(main),
        )
        .into_element()
}

fn wordmark() -> View {
    row()
        .gap_2()
        .px_2()
        .child(index_mark(true))
        .child(subheading("Biblio").text_size(px(19.)))
}

fn nav_group(name: &str) -> Text {
    caption(name.to_owned())
        .font_weight(FontWeight::SEMIBOLD)
        .px_2()
        .pb_1()
}

fn navigation(app: App, menu: Signal<bool>, narrow: Memo<bool>) -> View {
    let mut library = view()
        .gap(px(1.))
        .child(nav_group("Library"))
        .child(nav_item(
            app,
            menu,
            "All files",
            Icon::Library,
            Page::Library,
            None,
        ))
        .child(dynamic(move || {
            app.revision.get();
            app.load(category_list(app.db()), move |categories| {
                let mut list = view().gap(px(1.));
                for cat in categories {
                    list = list.child(nav_item(
                        app,
                        menu,
                        &cat.name,
                        Icon::Category,
                        Page::Library,
                        Some(cat.id),
                    ));
                }
                list.into_element()
            })
        }));
    library = library.child(nav_item(
        app,
        menu,
        "Import",
        Icon::Import,
        Page::Import,
        None,
    ));
    let mut organize = view().gap(px(1.)).child(nav_group("Organize"));
    for (target, name, glyph) in [
        (Page::Categories, "Categories", Icon::Categories),
        (Page::Tags, "Tags", Icon::Tags),
        (Page::Authors, "Authors", Icon::Authors),
        (Page::Schemas, "Schemas", Icon::Schemas),
        (Page::Prompts, "Prompts", Icon::Prompts),
    ] {
        organize = organize.child(nav_item(app, menu, name, glyph, target, None));
    }
    let footer = view()
        .gap(px(1.))
        .px_3()
        .py_3()
        .border_t_1()
        .border_color(rule)
        .child(nav_item(
            app,
            menu,
            "Cleanup",
            Icon::Cleanup,
            Page::Cleanup,
            None,
        ))
        .child(nav_item(
            app,
            menu,
            "Settings",
            Icon::Settings,
            Page::Settings,
            None,
        ));
    view()
        .test_id("navigation")
        .w(move || {
            if narrow.get() {
                window().width
            } else {
                px(232.)
            }
        })
        .flex_shrink_0()
        .h_full()
        .bg(rail)
        .border_r_1()
        .border_color(rule)
        .child(when(
            move || !narrow.get() && band() > px(0.),
            move || view().h(band()).flex_shrink_0().window_drag(true),
        ))
        .child(when(
            move || !narrow.get(),
            move || {
                wordmark()
                    .px_5()
                    .pt(move || if band() > px(0.) { px(2.) } else { px(26.) })
                    .pb_4()
            },
        ))
        .child(
            scroll().flex_1().min_h_0().child(
                view()
                    .px_3()
                    .pt(move || px(if narrow.get() { 12. } else { 4. }))
                    .pb_4()
                    .gap_5()
                    .child(library)
                    .child(organize),
            ),
        )
        .child(footer)
}

fn nav_button(
    name: &str,
    glyph: Icon,
    selected: impl Fn() -> bool + Copy + 'static,
    action: impl Fn() + 'static,
) -> nsh_base::ButtonElement {
    nsh_base::button()
        .accessibility_label(name.to_owned())
        .selected(selected)
        .on_press(action)
        .rounded(px(RADIUS))
        .px_2()
        .min_h(px(30.))
        .min_w_0()
        .justify_center()
        .bg(move || if selected() { wash() } else { clear() })
        .transition_colors()
        .duration(tween(INSTANT))
        .hover(|s| s.bg(wash))
        .active(|s| s.bg(rule))
        .focus(|s| s.outline(px(2.)).outline_color(accent))
        .child(
            row()
                .gap_3()
                .min_w_0()
                .child(icon(
                    glyph,
                    move || {
                        if selected() { accent() } else { secondary() }
                    },
                ))
                .child(label(name).number_of_lines(1).style_with(move |s| {
                    s.font_weight(if selected() {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                })),
        )
}

fn nav_item(
    app: App,
    menu: Signal<bool>,
    name: &str,
    glyph: Icon,
    target: Page,
    category: Option<i64>,
) -> impl IntoElement {
    let selected = move || match (target, app.page.get()) {
        (Page::Library, Page::Library | Page::File(_)) => app.category.get() == category,
        (_, page) => target == page,
    };
    nav_button(name, glyph, selected, move || {
        if target == Page::Library {
            app.category.set(category);
        }
        app.page.set(target);
        menu.set(false);
    })
}
