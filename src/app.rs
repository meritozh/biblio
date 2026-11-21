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
fn workspace(app: App) -> AnyElement {
    let mut sidebar = stack()
        .w(px(208.))
        .h_full()
        .p_4()
        .bg(surface)
        .child(title("Biblio"))
        .child(nav_item(app, "All files", Page::Library, None))
        .child(dynamic(move || {
            app.revision.get();
            app.load(category_list(app.db()), move |categories| {
                let mut list = stack().gap_1();
                for cat in categories {
                    let id = cat.id;
                    list = list.child(nav_item(app, &cat.name, Page::Library, Some(id)));
                }
                list.into_element()
            })
        }));
    for (target, name) in [
        (Page::Import, "Import"),
        (Page::Categories, "Categories"),
        (Page::Tags, "Tags"),
        (Page::Authors, "Authors"),
        (Page::Schemas, "Schemas"),
        (Page::Prompts, "Prompts"),
        (Page::Cleanup, "Cleanup"),
        (Page::Settings, "Settings"),
    ] {
        sidebar = sidebar.child(nav_item(app, name, target, None));
    }
    view()
        .flex_1()
        .flex_row()
        .w_full()
        .h_full()
        .bg(paper)
        .child(scroll().w(px(208.)).h_full().child(sidebar))
        .child(
            stack()
                .flex_1()
                .h_full()
                .p_4()
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
                .child(text(move || app.message.get()).text_sm().text_color(ink)),
        )
        .into_element()
}

fn nav_item(app: App, name: &str, target: Page, category: Option<i64>) -> impl IntoElement {
    button(name, move || {
        if target == Page::Library {
            app.category.set(category);
        }
        app.page.set(target);
    })
    .bg(move || {
        if app.page.get() == target && (target != Page::Library || app.category.get() == category) {
            paper()
        } else {
            surface()
        }
    })
}
