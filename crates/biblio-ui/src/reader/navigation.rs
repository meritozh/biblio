//! Owns a single open request from routing through the last external side effect.
use super::session;
use crate::{
    app::{App, Page, run_service},
    ui::*,
};
use biblio_core::{
    Library,
    commands::{
        cache,
        file::create_replace::file_get,
        text_open::{self, CopyState, Entry, ExternalOpener, OpenTarget, SystemOpener},
    },
};
use nsh_core::prelude::*;
use std::{future::Future, path::PathBuf, pin::Pin, rc::Rc};
use tokio::sync::watch;

type CopyFuture = Pin<Box<dyn Future<Output = Result<watch::Receiver<CopyState>, String>> + Send>>;
#[derive(Clone)]
pub(super) struct OpenServices {
    pub readable: Rc<dyn Fn(Library, i64) -> CopyFuture>,
    pub launcher: Rc<dyn ExternalOpener>,
}

impl Default for OpenServices {
    fn default() -> Self {
        Self {
            readable: Rc::new(|db, id| {
                Box::pin(async move { cache::readable_copy(&db, id).await })
            }),
            launcher: Rc::new(SystemOpener),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Navigation {
    pub find: Signal<u64>,
    // Opening a file observes a pending global opener edit, even when the
    // Settings tab that submitted it has already unmounted.
    opener_write: Stored<Option<watch::Receiver<()>>>,
    serial: Stored<u64>,
    route: Signal<(u64, Entry, Option<String>)>,
    pub(crate) page_epoch: Stored<u64>,
}

impl Navigation {
    pub(super) fn saving_opener(self) -> watch::Sender<()> {
        let (done, rx) = watch::channel(());
        self.opener_write.replace(Some(rx));
        done
    }

    pub fn new() -> Self {
        Self {
            find: signal(0),
            opener_write: stored(None),
            serial: stored(0),
            route: signal((0, Entry::Shelf, None)),
            page_epoch: stored(0),
        }
    }
}

pub fn activate(app: App, id: i64, entry: Entry) {
    let nav = app.reader;
    let serial = nav.serial.with(|n| {
        *n += 1;
        *n
    });
    let source = app.page.get_untracked();
    let epoch = nav.page_epoch.with(|n| *n);
    let db = app.db();
    let opener = nav.opener_write.get();
    let task = run_service(async move {
        if let Some(mut write) = opener {
            let _ = write.changed().await;
        }
        text_open::target(&db, id, entry).await
    });
    app.owner.run(|| {
        spawn(async move {
            let result = task.await.map_err(|e| e.to_string()).and_then(|r| r);
            if nav.serial.with(|n| *n == serial)
                && app.page.get_untracked() == source
                && nav.page_epoch.with(|n| *n == epoch)
            {
                match result {
                    Ok(OpenTarget::Details(id)) => app.page.set(Page::File(id)),
                    Ok(OpenTarget::CategoryFile(id)) => {
                        app.job(cache::cache_open(app.db(), id), |_| {})
                    }
                    Ok(target) => {
                        let program = match target {
                            OpenTarget::ExternalTxt { program, .. } => Some(program),
                            _ => None,
                        };
                        nav.find.set(0);
                        nav.route.set((serial, entry, program));
                        app.page.set(Page::Reader(id));
                    }
                    Err(error) => app.message.set(error),
                }
            }
        })
    });
}

pub fn back(app: App) {
    if matches!(app.page.get_untracked(), Page::File(_) | Page::Reader(_)) {
        app.reader.serial.with(|n| *n += 1);
        app.page.set(Page::Library);
    }
}

pub fn screen(app: App, id: i64) -> AnyElement {
    let retry = signal(0_u64);
    let title = app.resource(move || file_get(app.db(), id));
    let header = dynamic(move || {
        let name = title
            .get()
            .as_deref()
            .and_then(|r| r.as_ref().ok())
            .map(|file| file.display_name.clone())
            .unwrap_or_else(|| "Text Reader".into());
        header_bar(
            icon_button("Back to library", Icon::Back, Some("⌘["), move || {
                back(app)
            })
            .into_element(),
            name,
            None,
            None,
        )
    });
    stack()
        .gap_0()
        .flex_1()
        .min_h_0()
        .child(header)
        .child(dynamic(move || {
            retry.get();
            let serial = app.reader.route.with(|r| r.0);
            untracked(move || copy_screen(app, id, serial, retry))
        }))
        .into_element()
}

enum OpenStatus {
    Waiting(&'static str),
    Ready(PathBuf),
    Failed(String),
}

fn copy_screen(app: App, id: i64, serial: u64, retry: Signal<u64>) -> AnyElement {
    let (_, entry, program) = app.reader.route.get_untracked();
    let services = use_context::<OpenServices>().unwrap_or_default();
    let status = signal_with(
        Rc::new(OpenStatus::Waiting("Opening file…")),
        SignalEquals::Never,
    );
    let attempt = retry.get_untracked();
    let active = move || {
        app.page.get_untracked() == Page::Reader(id)
            && app.reader.serial.with(|n| *n == serial)
            && retry.get_untracked() == attempt
    };
    let task = run_service((services.readable)(app.db(), id));
    let abort = task.abort_handle();
    cleanup(move || abort.abort());
    spawn(async move {
        let result = async {
            let mut watch = task.await.map_err(|e| e.to_string())??;
            loop {
                if !active() {
                    return Ok(());
                }
                let state = watch.borrow_and_update().clone();
                match state {
                    CopyState::Ready(path) => {
                        if let Some(program) = &program {
                            services.launcher.launch(&path, Some(program))?;
                            app.page.set(match entry {
                                Entry::Shelf => Page::Library,
                                Entry::DetailOpen => Page::File(id),
                            });
                        } else {
                            status.set(Rc::new(OpenStatus::Ready(path)));
                        }
                        return Ok(());
                    }
                    CopyState::Failed(error) => return Err(error),
                    CopyState::Queued => {
                        status.set(Rc::new(OpenStatus::Waiting("Download queued…")))
                    }
                    CopyState::Downloading => {
                        status.set(Rc::new(OpenStatus::Waiting("Downloading file…")))
                    }
                }
                watch
                    .changed()
                    .await
                    .map_err(|_| "Download stopped".to_string())?;
            }
        }
        .await;
        if let Err(error) = result
            && active()
        {
            status.set(Rc::new(OpenStatus::Failed(error)));
        }
    });
    dynamic(move || match &*status.get() {
        OpenStatus::Waiting(stage) => note(*stage).into_element(),
        OpenStatus::Failed(error) => stack()
            .child(notice(error.clone()))
            .child(button("Retry", move || retry.update(|n| *n += 1)))
            .into_element(),
        OpenStatus::Ready(path) => untracked(|| session::screen(app, id, path.clone(), retry)),
    })
    .into_element()
}
