use crate::artwork::{Icon, icon, index_mark};
use crate::{admin, catalog, detail, imports, settings, ui::*};
use biblio_core::{
    Library,
    commands::{category::category_list, processing::FilePreparedImport},
};
use nsh_core::prelude::*;
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
const COMPACT: f32 = 760.;

fn workspace(app: App) -> AnyElement {
    let menu = signal(false);
    let compact = computed(|| window().width < px(COMPACT));
    let sidebar = move || navigation(app, menu);
    let main = stack()
        .style_with(move |s| {
            if compact.get() && menu.get() {
                s.hidden()
            } else {
                s
            }
        })
        .flex_1()
        .min_w_0()
        .min_h_0()
        .gap_4()
        .px(move || px(if compact.get() { 16. } else { 40. }))
        .pt(move || px(if compact.get() { 16. } else { 32. }))
        .pb(move || px(if compact.get() { 12. } else { 20. }))
        .child(dynamic(move || {
            let page = app.page.get();
            untracked(|| match page {
                Page::Library => catalog::screen(app),
                Page::File(id) => detail::screen(app, id),
                Page::Import => imports::screen(app),
                Page::Settings => settings::screen(app),
                page => admin::screen(app, page),
            })
        }))
        .child(when(
            move || !app.message.with(String::is_empty),
            move || {
                row().pt_2().border_t_1().border_color(rule).child(
                    text(move || app.message.get())
                        .text_sm()
                        .text_color(secondary),
                )
            },
        ));
    stack()
        .w_full()
        .h_full()
        .gap_0()
        .bg(paper)
        .child(when(
            move || compact.get(),
            move || {
                row()
                    .justify_between()
                    .bg(rail)
                    .border_b_1()
                    .border_color(rule)
                    .px_4()
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
                    move || !compact.get() || menu.get(),
                    move || {
                        scroll()
                            .w(move || {
                                if compact.get() {
                                    window().width
                                } else {
                                    px(220.)
                                }
                            })
                            .flex_shrink_0()
                            .h_full()
                            .bg(rail)
                            .border_r_1()
                            .border_color(rule)
                            .child(sidebar())
                    },
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
        .child(title("Biblio").text_size(px(20.)))
}

fn nav_group(name: &str) -> Text {
    caption(name.to_owned())
        .font_weight(FontWeight::SEMIBOLD)
        .px_2()
        .pb_1()
}

fn navigation(app: App, menu: Signal<bool>) -> View {
    let mut library = stack()
        .gap_0()
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
                let mut list = stack().gap_0();
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
    let mut organize = stack().gap_0().child(nav_group("Organize"));
    for (target, name, glyph) in [
        (Page::Categories, "Categories", Icon::Categories),
        (Page::Tags, "Tags", Icon::Tags),
        (Page::Authors, "Authors", Icon::Authors),
        (Page::Schemas, "Schemas", Icon::Schemas),
        (Page::Prompts, "Prompts", Icon::Prompts),
    ] {
        organize = organize.child(nav_item(app, menu, name, glyph, target, None));
    }
    stack()
        .px_3()
        .pt(move || {
            px(if window().width < px(COMPACT) {
                16.
            } else {
                28.
            })
        })
        .pb_6()
        .gap_6()
        .child(when(|| window().width >= px(COMPACT), || wordmark().pb_2()))
        .child(library)
        .child(organize)
        .child(
            stack()
                .gap_0()
                .child(nav_group("Workspace"))
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
                )),
        )
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
        .rounded_md()
        .px_2()
        .min_h(px(32.))
        .min_w_0()
        .justify_center()
        .bg(move || if selected() { wash() } else { clear() })
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
