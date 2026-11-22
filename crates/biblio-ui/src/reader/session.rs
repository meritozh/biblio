//! One document owner and independent, cancellable blocking readers. A slow
//! directory scan never occupies the viewport or search lane.
use super::{
    controls, encoding,
    toolbar::{Toolbar, side_panel},
    viewport::{self, Pages},
};
use crate::{
    app::{App, run_service},
    ui::*,
};
use biblio_core::text_reader::{
    book::{Cursor, Direction, Hit, ReadFrom, ReadingPosition, TextBook, TextPoint, Window},
    rules::{self, Contents, RuleSurvey},
    store::{ReaderState, ReaderStore},
};
use nsh_core::prelude::*;
use std::{fs::File, path::PathBuf, rc::Rc, sync::Arc};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

struct CurrentRequest(CancellationToken);
impl Drop for CurrentRequest {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

#[derive(Clone, Copy)]
pub(super) struct Lane {
    owner: Owner,
    token: Stored<CurrentRequest>,
    permit: Stored<Arc<Semaphore>>,
}

impl Lane {
    pub(super) fn new() -> Self {
        Self::with_parallelism(1)
    }

    pub(super) fn with_parallelism(n: usize) -> Self {
        Self {
            owner: Owner::current(),
            token: stored(CurrentRequest(CancellationToken::new())),
            permit: stored(Arc::new(Semaphore::new(n))),
        }
    }

    pub(super) fn cancel(self) {
        self.token.with(|s| s.0.cancel());
    }

    pub(super) fn run<T: Send + 'static>(
        self,
        work: impl FnOnce(&CancellationToken) -> Result<T, String> + Send + 'static,
        done: impl FnOnce(Result<T, String>) + 'static,
    ) {
        self.cancel();
        let stop = CancellationToken::new();
        self.token.with(|s| s.0 = stop.clone());
        let permit = self.permit.with(|s| s.clone());
        let worker_stop = stop.clone();
        let task = run_service(async move {
            let permit = tokio::select! {
                _ = worker_stop.cancelled() => return Err("Cancelled".into()),
                p = permit.acquire_owned() => p.map_err(|e| e.to_string())?,
            };
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                work(&worker_stop)
            })
            .await
            .map_err(|e| e.to_string())?
        });
        self.owner.run(|| {
            spawn(async move {
                let result = task.await.map_err(|e| e.to_string()).and_then(|r| r);
                if !stop.is_cancelled() {
                    done(result);
                }
            })
        });
    }
}

fn visible_window(
    book: &TextBook,
    mut from: ReadFrom,
    collapse: bool,
    stop: &CancellationToken,
) -> Result<Window, String> {
    let backwards = matches!(from, ReadFrom::Before(_));
    let mut first = None;
    loop {
        let mut window = book
            .window(from, collapse, stop)
            .map_err(|e| e.to_string())?;
        let next = if backwards {
            window.before.map(ReadFrom::Before)
        } else {
            window.after.map(ReadFrom::After)
        };
        let leading = if backwards {
            &mut window.after
        } else {
            &mut window.before
        };
        let original = *first.get_or_insert(*leading);
        if window.rows.is_empty()
            && let Some(next) = next
        {
            from = next;
            continue;
        }
        *leading = original;
        return Ok(window);
    }
}

pub(super) fn screen(
    app: App,
    id: i64,
    path: PathBuf,
    retry: Signal<u64>,
    toolbar: Toolbar,
) -> AnyElement {
    // Re-decode the same held source after a confirmed preview, including
    // when a cache path has been replaced while the book was open.
    let reopen = signal_with(None::<Arc<TextBook>>, SignalEquals::Never);
    dynamic(move || {
        let previous = reopen.get();
        untracked(|| load_document(app, id, path.clone(), previous, reopen, retry, toolbar))
    })
    .into_element()
}

fn load_document(
    app: App,
    id: i64,
    path: PathBuf,
    previous: Option<Arc<TextBook>>,
    reopen: Signal<Option<Arc<TextBook>>>,
    retry: Signal<u64>,
    toolbar: Toolbar,
) -> AnyElement {
    let store = ReaderStore::new(app.db());
    let saved = app
        .writes
        .with(|w| w.ordered(async move { store.load(Some(id)).await.map_err(|e| e.to_string()) }));
    let loaded = signal_with(None, SignalEquals::Never);
    let open = Lane::new();
    spawn(async move {
        match saved.await.map_err(|e| e.to_string()).and_then(|r| r) {
            Err(e) => loaded.set(Some(Err(e))),
            Ok(mut state) => {
                controls::resolve_font(&mut state);
                let saved = state.book.clone().unwrap_or_default();
                let resume = saved.checkpoint.as_ref().map(|c| c.at.into());
                open.run(
                    move |cancel| {
                        let book = match previous {
                            Some(source) => source.with_encoding(saved.encoding, cancel),
                            None => TextBook::open(
                                File::open(path).map_err(|e| e.to_string())?,
                                saved.encoding,
                                cancel,
                            ),
                        }
                        .map_err(|e| e.to_string())?;
                        let window = visible_window(
                            &book,
                            ReadFrom::Resume(saved.checkpoint),
                            state.collapse_blanks,
                            cancel,
                        )?;
                        Ok((Arc::new(book), state, window, resume))
                    },
                    move |r| {
                        loaded.set(Some(r.map(|(book, state, window, resume)| {
                            ReaderSession::new(
                                app,
                                id,
                                book,
                                state,
                                Pages::new(window),
                                resume,
                                reopen,
                                toolbar.status,
                            )
                        })))
                    },
                );
            }
        }
    });
    dynamic(move || {
        loaded.with(|r| match r {
            None => note("Loading text…").into_element(),
            Some(Err(e)) => stack()
                .child(notice(e.clone()))
                .child(button("Retry", move || retry.update(|n| *n += 1)))
                .into_element(),
            Some(Ok(session)) => untracked(|| session.body(toolbar)),
        })
    })
    .into_element()
}

#[derive(Clone, Copy)]
pub struct ReaderSession {
    pub app: App,
    pub id: i64,
    pub book: Stored<Arc<TextBook>>,
    pub state: Signal<Rc<ReaderState>>,
    pub(super) pages: Signal<Pages>,
    pub anchor: Signal<Option<ReadingPosition>>,
    pub destination: Signal<Option<TextPoint>>,
    pub hit: Signal<Option<Hit>>,
    pub read_error: Signal<String>,
    reading: Lane,
    looking: Lane,
    positioning_match: Stored<bool>,
    pub(super) scanning: Lane,
    pub loading: Signal<bool>,
    last_query: Stored<String>,
    find_status: Signal<String>,
    pub contents: Signal<Option<Rc<Contents>>>,
    pub survey: Signal<Option<Rc<RuleSurvey>>>,
    pub(super) scan_status: Signal<String>,
    pub(super) scanning_rules: Lane,
    pub(super) scanner: Stored<Scanner>,
    pub(super) outliner: Stored<super::contents::Outliner>,
    pub(super) reopen: Signal<Option<Arc<TextBook>>>,
}

impl ReaderSession {
    fn new(
        app: App,
        id: i64,
        book: Arc<TextBook>,
        state: ReaderState,
        pages: Pages,
        resume: Option<TextPoint>,
        reopen: Signal<Option<Arc<TextBook>>>,
        scan_status: Signal<String>,
    ) -> Self {
        let session = Self {
            app,
            id,
            book: stored(book),
            state: signal(Rc::new(state)),
            pages: signal_with(pages, SignalEquals::Never),
            anchor: signal(None),
            destination: signal(resume),
            hit: signal(None),
            read_error: signal(String::new()),
            reading: Lane::new(),
            looking: Lane::new(),
            positioning_match: stored(false),
            scanning: Lane::new(),
            loading: signal(false),
            last_query: stored(String::new()),
            find_status: signal(String::new()),
            contents: signal_with(None, SignalEquals::Never),
            survey: signal_with(None, SignalEquals::Never),
            scanning_rules: Lane::new(),
            scanner: stored(use_context::<Scanner>().unwrap_or_default()),
            outliner: stored(use_context::<super::contents::Outliner>().unwrap_or_default()),
            reopen,
            scan_status,
        };
        let writes = app.writes.with(|w| w.clone());
        cleanup(move || {
            writes.finish_session();
        });
        session.scan();
        session
    }

    pub fn visible(self, at: ReadingPosition) {
        if self.anchor.get_untracked() != Some(at) {
            self.anchor.set(Some(at));
            let checkpoint = self.book.with(|b| b.checkpoint(at));
            self.app
                .writes
                .with(|w| w.position(ReaderStore::new(self.app.db()), self.id, checkpoint));
        }
    }

    pub fn extend(self, cursor: Cursor, backwards: bool) {
        if !self.loading.get_untracked() && self.read_error.get_untracked().is_empty() {
            let from = if backwards {
                ReadFrom::Before(cursor)
            } else {
                ReadFrom::After(cursor)
            };
            self.read(from, None);
        }
    }

    pub fn reveal(self, point: TextPoint) {
        self.positioning_match.with(|pending| *pending = false);
        self.read(ReadFrom::Around(point), Some(point));
    }

    fn read(self, from: ReadFrom, destination: Option<TextPoint>) {
        let book = self.book.with(|b| b.clone());
        let before = matches!(from, ReadFrom::Before(_));
        let collapse = self.state.get_untracked().collapse_blanks;
        self.loading.set(true);
        self.read_error.set(String::new());
        self.reading.run(
            move |stop| visible_window(&book, from, collapse, stop),
            move |r| {
                self.loading.set(false);
                self.positioning_match.with(|pending| *pending = false);
                match r {
                    Ok(window) => {
                        if let Some(point) = destination {
                            self.destination.set(Some(point));
                            self.pages.set(Pages::new(window));
                        } else {
                            let anchor = self.anchor.get_untracked();
                            self.pages.update(|p| {
                                let changed = p.extend(window, before);
                                if self.destination.get_untracked().is_none()
                                    && let Some(a) = anchor
                                    && (before || changed == Some(a.paragraph))
                                {
                                    self.destination.set(Some(a.into()));
                                }
                            });
                        }
                    }
                    Err(e) => self.read_error.set(e),
                }
            },
        );
    }

    pub fn find(self, query: String, direction: Direction) {
        self.looking.cancel();
        if self.positioning_match.with(std::mem::take) {
            self.reading.cancel();
            self.loading.set(false);
        }
        let same = self.last_query.with(|q| *q == query);
        self.last_query.with(|q| *q = query.clone());
        let old = self.hit.get_untracked().filter(|_| same);
        if !same || query.is_empty() {
            self.hit.set(None);
        }
        if query.is_empty() {
            self.find_status.set(String::new());
            return;
        }
        let from = old
            .map(|h| match direction {
                Direction::Previous => h.start,
                Direction::Next => h.end,
            })
            .unwrap_or_else(|| self.anchor.get_untracked().unwrap_or_default().into());
        let book = self.book.with(|b| b.clone());
        self.find_status.set("Searching…".into());
        self.looking.run(
            move |stop| {
                book.find(&query, &from, direction, stop)
                    .map_err(|e| e.to_string())
            },
            move |r| match r {
                Ok(hit) => {
                    self.hit.set(hit);
                    self.find_status.set(if hit.is_none() {
                        "No matches".into()
                    } else {
                        String::new()
                    });
                    if let Some(hit) = hit {
                        self.positioning_match.with(|p| *p = true);
                        self.read(ReadFrom::Around(hit.start), Some(hit.start));
                    }
                }
                Err(e) => self.find_status.set(e),
            },
        );
    }

    fn body(self, toolbar: Toolbar) -> AnyElement {
        let query = signal(String::new());
        let input_ref = element_ref();
        let open_find = signal(false);
        let open_contents = toolbar.contents;
        let open_settings = toolbar.settings;
        toolbar.ready.set(true);
        cleanup(move || toolbar.ready.set(false));
        let open_encoding = signal(false);
        let book_text = element_ref();
        spawn(async move {
            next_frame().await;
            if !open_find.get_untracked()
                && !open_contents.get_untracked()
                && !open_settings.get_untracked()
                && !open_encoding.get_untracked()
            {
                book_text.focus();
            }
        });
        let controls = controls::Controls::new(self.app, self.state, Some(self));
        let nav = self.app.reader;
        let focus = move || {
            open_contents.set(false);
            open_settings.set(false);
            if open_find.get_untracked() {
                input_ref.focus();
            } else {
                open_find.set(true);
            }
        };
        let seen = stored(0);
        effect(move || {
            let n = nav.find.get();
            if seen.with(|v| std::mem::replace(v, n) != n) {
                focus();
            }
        });
        let search = move |dir| self.find(query.get_untracked(), dir);
        let settings = move || {
            view()
                .gap_3()
                .child(controls.panel())
                .child(button("Book encoding", move || {
                    if compact() {
                        open_settings.set(false);
                    }
                    open_encoding.set(true);
                }))
        };
        let outline = move || super::contents::list(self, self.contents, Some(open_contents));
        stack()
            .flex_1()
            .min_h_0()
            .gap_0()
            .child(modal(
                open_encoding,
                || "Book encoding".into(),
                560.,
                move || encoding::panel(controls, self, open_encoding),
            ))
            .child(when(
                move || self.book.with(|b| b.encoding_uncertain()),
                move || {
                    row()
                        .px(inset)
                        .child(note("Encoding uncertain."))
                        .child(ghost("Preview book encoding", move || {
                            open_encoding.set(true)
                        }))
                },
            ))
            .child(when(
                move || open_find.get(),
                move || {
                    let newly_opened = stored(true);
                    row()
                        .gap_1()
                        .px(inset)
                        .py_1()
                        .border_b_1()
                        .border_color(rule)
                        .child(
                            input("Find text", query)
                                .element_ref(&input_ref)
                                .auto_focus(true)
                                .on_layout(move |_: &Bounds<Points>| {
                                    if newly_opened.with(|v| std::mem::take(v)) {
                                        input_ref.focus();
                                    }
                                })
                                .multiline(true)
                                .on_change(move |s: &str| {
                                    query.set(s.into());
                                    self.find(s.into(), Direction::Next);
                                })
                                .on_submit(move |_: &str| search(Direction::Next))
                                .flex_1(),
                        )
                        .child(icon_button(
                            "Previous match",
                            Icon::ArrowUp,
                            None,
                            move || search(Direction::Previous),
                        ))
                        .child(icon_button(
                            "Next match",
                            Icon::ArrowDown,
                            None,
                            move || search(Direction::Next),
                        ))
                        .child(icon_button("Close find", Icon::Close, None, move || {
                            open_find.set(false);
                            query.set(String::new());
                            self.find(String::new(), Direction::Next);
                        }))
                        .child(
                            text(move || self.find_status.get())
                                .text_size(px(12.))
                                .text_color(secondary)
                                .max_w(move || px(window().width.0 * 0.25))
                                .number_of_lines(1),
                        )
                },
            ))
            .child(when(
                move || !self.read_error.get().is_empty(),
                move || {
                    wrap()
                        .child(text(move || self.read_error.get()))
                        .child(small("Retry reading", move || {
                            self.reveal(self.anchor.get_untracked().unwrap_or_default().into());
                        }))
                },
            ))
            .child(
                row()
                    .gap_0()
                    .items_stretch()
                    .flex_1()
                    .min_h_0()
                    .child(when(
                        move || open_contents.get() && !compact(),
                        move || {
                            side_panel(
                                "Contents",
                                open_contents,
                                260.,
                                outline().flex_1().min_h_0(),
                            )
                            .border_r_1()
                            .border_color(rule)
                        },
                    ))
                    .child(viewport::viewport(self, book_text))
                    .child(when(
                        move || open_settings.get() && !compact(),
                        move || {
                            side_panel(
                                "Reading settings",
                                open_settings,
                                312.,
                                scroll()
                                    .flex_1()
                                    .min_h_0()
                                    .child(view().px_4().pb_5().child(settings())),
                            )
                            .border_l_1()
                            .border_color(rule)
                        },
                    )),
            )
            .child(when(compact, move || {
                view()
                    .child(modal(
                        open_contents,
                        || "Contents".into(),
                        440.,
                        move || {
                            stack()
                                .child(text(move || self.scan_status.get()).text_color(secondary))
                                .child(outline().h(move || px(window().height.0 * 0.52)))
                                .into_element()
                        },
                    ))
                    .child(modal(
                        open_settings,
                        || "Reading settings".into(),
                        400.,
                        move || settings().into_element(),
                    ))
            }))
            .into_element()
    }
}

type SurveyFn = dyn Fn(&TextBook, &[rules::SavedRule], &CancellationToken) -> Result<RuleSurvey, String>
    + Send
    + Sync;
#[derive(Clone)]
pub(super) struct Scanner(pub Arc<SurveyFn>);
impl Default for Scanner {
    fn default() -> Self {
        Self(Arc::new(|book, rules, stop| {
            rules::survey(book, rules, stop).map_err(|e| e.to_string())
        }))
    }
}
