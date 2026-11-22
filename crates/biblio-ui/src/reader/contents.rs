//! Book surveys, committed outlines and virtualized outline rows.
use super::session::{Lane, ReaderSession};
use crate::ui::*;
use biblio_core::text_reader::{
    book::{ReadingPosition, TextBook},
    rules::{self, ChapterRule, Contents, RuleDraft, Title},
    store::DefaultRule,
};
use nsh_core::prelude::*;
use std::{rc::Rc, sync::Arc};
use tokio_util::sync::CancellationToken;

type OutlineFn =
    dyn Fn(&TextBook, &RuleDraft, &CancellationToken) -> Result<Contents, String> + Send + Sync;
#[derive(Clone)]
pub(super) struct Outliner(pub Arc<OutlineFn>);
impl Default for Outliner {
    fn default() -> Self {
        Self(Arc::new(|book, draft, stop| {
            let compiled = ChapterRule::compile(draft).map_err(|e| e.to_string())?;
            rules::outline(book, &compiled, stop).map_err(|e| e.to_string())
        }))
    }
}

impl ReaderSession {
    pub(super) fn scan(self) {
        self.scanning.cancel();
        self.survey.set(None);
        self.scan_status.set("Scanning chapters…".into());
        let book = self.book.get();
        let rules = self.state.get_untracked().rules.clone();
        let scanner = self.scanner.get();
        self.scanning_rules.run(
            move |stop| (scanner.0)(&book, &rules, stop),
            move |result| match result {
                Ok(survey) => {
                    self.survey.set(Some(Rc::new(survey)));
                    self.rebuild_contents();
                }
                Err(e) => self.scan_status.set(e),
            },
        );
    }

    pub(super) fn rebuild_contents(self) {
        self.scanning.cancel();
        let state = self.state.get_untracked();
        let Some(survey) = self.survey.get_untracked() else {
            return;
        };
        let id = match state.effective_rule() {
            DefaultRule::Auto => survey.detected.clone(),
            DefaultRule::Fixed(id) => Some(id),
        };
        let Some(rule) = state.rules.iter().find(|r| Some(&r.id) == id.as_ref()) else {
            self.publish(Rc::new(Contents::default()));
            return;
        };
        let book = self.book.get();
        let draft = rule.draft.clone();
        let scan = self.outliner.get();
        self.scan_status.set("Scanning chapters…".into());
        self.scanning.run(
            move |stop| (scan.0)(&book, &draft, stop),
            move |r| match r {
                Ok(contents) => self.publish(Rc::new(contents)),
                Err(e) => self.scan_status.set(e),
            },
        );
    }

    pub(super) fn publish(self, contents: Rc<Contents>) {
        self.scanning.cancel();
        self.scan_status
            .set(counted(contents.chapters as i64, "chapter", "chapters"));
        self.contents.set(Some(contents));
    }
}

pub(super) fn title_text(
    session: ReaderSession,
    title: Title,
    display: impl Fn(String) -> AnyElement + 'static,
) -> AnyElement {
    match title {
        Title::Text(s) => display(s),
        title => {
            let book = session.book.get();
            let name = signal("Loading title…".to_string());
            Lane::new().run(
                move |stop| {
                    title
                        .resolve(&book, stop)
                        .map(|s| s.into_owned())
                        .map_err(|e| e.to_string())
                },
                move |r| name.set(r.unwrap_or_else(|e| e)),
            );
            dynamic(move || display(name.get())).into_element()
        }
    }
}

pub(super) fn list(
    session: ReaderSession,
    contents: Signal<Option<Rc<Contents>>>,
    navigate: Option<Signal<bool>>,
) -> VirtualList {
    let current = computed(move || {
        let at = session.anchor.get().unwrap_or_default().paragraph;
        contents.with(|c| {
            c.as_ref()?
                .entries
                .partition_point(|entry| entry.anchor.paragraph <= at)
                .checked_sub(1)
        })
    });
    virtual_list(
        move || contents.with(|c| c.as_ref().map_or(0, |c| c.entries.len())),
        move |pos| {
            dynamic(move || {
                let item = contents.with(|c| c.as_ref()?.entries.get(pos.index()).cloned());
                let Some(item) = item else {
                    return view().into_element();
                };
                let at: ReadingPosition = item.anchor;
                untracked(|| {
                    title_text(session, item.title, move |name| {
                        let label = format!("{name} · {}", item.words);
                        if let Some(open) = navigate {
                            let active = move || current.get() == Some(pos.index());
                            nsh_base::button()
                                .selected(active)
                                .bg(move || if active() { tint() } else { clear() })
                                .accessibility_label(label)
                                .on_press(move || {
                                    open.set(false);
                                    session.reveal(at.into());
                                })
                                .px_4()
                                .py_2()
                                .w_full()
                                .items_start()
                                .hover(|s| s.bg(wash))
                                .focus(|s| s.bg(tint))
                                .child(crate::ui::label(name).number_of_lines(2))
                                .into_element()
                        } else {
                            text(label).text_color(ink).py_1().into_element()
                        }
                    })
                })
            })
        },
    )
    .initial_scroll_index(if navigate.is_some() {
        current.get_untracked().unwrap_or(0)
    } else {
        0
    })
    .label(if navigate.is_some() {
        "Table of contents"
    } else {
        "Preview contents"
    })
}
