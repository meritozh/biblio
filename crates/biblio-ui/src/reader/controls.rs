//! Shared reader preferences. Both entry points publish committed snapshots
//! from the window's serial writer. The book viewport keeps its own owner.
use super::{fonts, rule_manager, session::ReaderSession};
use crate::{app::App, ui::*};
use biblio_core::text_reader::{
    book::{Encoding, TextBook},
    rules::{Contents, SavedRule},
    store::{
        BookEdit, BookRule, Preference, ReaderState, ReaderStore, RuleEdit, TxtOpener, Typography,
    },
};
use nsh_core::prelude::*;
use std::{rc::Rc, sync::Arc};

#[derive(Clone, Copy)]
pub(super) enum Metric {
    Size,
    Leading,
    Spacing,
}

impl Metric {
    fn name(self) -> &'static str {
        match self {
            Self::Size => "Font size",
            Self::Leading => "Line height",
            Self::Spacing => "Letter spacing",
        }
    }
    fn value(self, ty: &mut Typography) -> &mut f32 {
        match self {
            Self::Size => &mut ty.size,
            Self::Leading => &mut ty.leading,
            Self::Spacing => &mut ty.spacing,
        }
    }
    fn adjust(self, value: f32, direction: f32) -> f32 {
        let (step, min, max) = match self {
            Self::Size => (1., 8., 72.),
            Self::Leading => (0.1, 1., 3.),
            Self::Spacing => (0.25, -1., 8.),
        };
        (((value + step * direction) * 100.).round() / 100.).clamp(min, max)
    }
}

pub(super) enum Change {
    Preference(Preference),
    Family(Option<String>),
    Metric(Metric, f32),
    Rule(RuleEdit),
    Apply {
        id: i64,
        selection: BookRule,
        expected: Option<SavedRule>,
        encoding: Option<Encoding>,
        book: Arc<TextBook>,
    },
    Encoding(i64, Option<Encoding>, Arc<TextBook>),
}

impl Change {
    async fn save(self, store: &ReaderStore) -> Result<Option<String>, String> {
        let r = match self {
            Self::Preference(p) => store.preference(p).await,
            Self::Family(_) | Self::Metric(_, _) => {
                let mut ty = store
                    .load(None)
                    .await
                    .map_err(|e| e.to_string())?
                    .typography;
                match self {
                    Self::Family(name) => ty.family = name,
                    Self::Metric(metric, value) => *metric.value(&mut ty) = value,
                    _ => unreachable!(),
                }
                store.preference(Preference::Type(ty)).await
            }
            Self::Rule(edit) => return store.edit_rule(edit).await.map_err(|e| e.to_string()),
            Self::Apply {
                id,
                selection,
                expected,
                encoding,
                book,
            } => {
                book.verify(&Default::default())
                    .map_err(|e| e.to_string())?;
                let state = store.load(Some(id)).await.map_err(|e| e.to_string())?;
                if state.book.as_ref().and_then(|b| b.encoding) != encoding
                    || expected.is_some_and(|rule| !state.rules.contains(&rule))
                {
                    return Err("The book encoding or rule has changed. Preview it again.".into());
                }
                store.edit_book(id, BookEdit::Rule(selection)).await
            }
            Self::Encoding(id, codec, source) => {
                source
                    .verify(&Default::default())
                    .map_err(|e| e.to_string())?;
                store.edit_book(id, BookEdit::Encoding(codec)).await
            }
        };
        r.map(|_| None).map_err(|e| e.to_string())
    }
}

pub(super) fn resolve_font(state: &mut ReaderState) {
    if let Some(name) = &state.typography.family
        && !fonts::installed().is_ok_and(|names| names.contains(name))
    {
        state.typography.family = None;
    }
}

#[derive(Clone, Copy)]
pub(super) struct Controls {
    pub session: Option<ReaderSession>,
    pub state: Signal<Rc<ReaderState>>,
    pub error: Signal<String>,
    app: App,
    owner: Owner,
    desired: Stored<Typography>,
    collapse: Signal<bool>,
    sent: Stored<u64>,
    published: Stored<u64>,
}

impl Controls {
    pub(super) fn new(
        app: App,
        state: Signal<Rc<ReaderState>>,
        session: Option<ReaderSession>,
    ) -> Self {
        Self {
            app,
            session,
            state,
            owner: Owner::current(),
            desired: stored(state.get_untracked().typography.clone()),
            collapse: signal(state.get_untracked().collapse_blanks),
            sent: stored(0),
            published: stored(0),
            error: signal(String::new()),
        }
    }

    pub(super) fn commit(
        self,
        change: Change,
        preview: Option<Rc<Contents>>,
        done: impl FnOnce(Option<String>) + 'static,
    ) {
        let coalesce = match &change {
            Change::Family(_) => Some("Font family"),
            Change::Metric(m, _) => Some(m.name()),
            _ => None,
        };
        let owner = Owner::current();
        let seq = self.sent.with(|n| {
            *n += 1;
            *n
        });
        let file = self.session.map(|s| s.id);
        let store = ReaderStore::new(self.app.db());
        let opener = matches!(&change, Change::Preference(Preference::Opener(_)))
            .then(|| self.app.reader.saving_opener());
        let edit = async move {
            let _opener = opener;
            let id = change.save(&store).await?;
            store
                .load(file)
                .await
                .map(|state| (state, id))
                .map_err(|e| e.to_string())
        };
        let rx = self.app.writes.with(|w| match coalesce {
            Some(field) => w.typography(field, edit),
            None => w.ordered(edit),
        });
        self.error.set(String::new());
        self.owner.run(|| {
            spawn(async move {
                let result = match rx.await {
                    Err(_) if coalesce.is_some() => return,
                    other => other.map_err(|e| e.to_string()).and_then(|v| v),
                };
                if self.published.get() > seq {
                    return;
                }
                self.published.replace(seq);
                match result {
                    Err(e) => {
                        if self.sent.get() == seq {
                            self.collapse
                                .set(self.state.get_untracked().collapse_blanks);
                            self.desired
                                .replace(self.state.get_untracked().typography.clone());
                        }
                        self.error.set(e);
                    }
                    Ok((mut state, id)) => {
                        resolve_font(&mut state);
                        let old = self.state.get_untracked();
                        let new = Rc::new(state);
                        if self.sent.get() == seq {
                            self.desired.replace(new.typography.clone());
                            self.collapse.set(new.collapse_blanks);
                        }
                        self.state.set(new.clone());
                        if let Some(s) = self.session {
                            if old.book.as_ref().map(|b| b.encoding)
                                != new.book.as_ref().map(|b| b.encoding)
                            {
                                s.reopen.set(Some(s.book.get()));
                                return;
                            }
                            if old.collapse_blanks != new.collapse_blanks {
                                s.reveal(s.anchor.get_untracked().unwrap_or_default().into());
                            }
                            if old.rules != new.rules {
                                s.scan();
                            } else if let Some(contents) = preview {
                                s.publish(contents);
                            } else if old.effective_rule() != new.effective_rule() {
                                s.rebuild_contents();
                            }
                        }
                        if owner.is_live() {
                            owner.run(|| done(id));
                        }
                    }
                }
            })
        });
    }

    fn prefer(self, p: Preference) {
        self.commit(Change::Preference(p), None, |_| {});
    }

    fn layout_setting(self, name: &str, control: impl IntoElement) -> View {
        if self.session.is_some() {
            view().gap_1().py_1().child(label(name)).child(control)
        } else {
            setting(name, None, control)
        }
    }

    fn metric(self, metric: Metric) -> AnyElement {
        let adjust = move |dir| {
            let value = self.desired.with(|ty| {
                let v = metric.value(ty);
                *v = metric.adjust(*v, dir);
                *v
            });
            self.commit(Change::Metric(metric, value), None, |_| {});
        };
        let name = metric.name();
        let stepper = row()
            .gap_2()
            .child(icon_button(
                format!("Decrease {}", name.to_lowercase()),
                Icon::ArrowDown,
                None,
                move || adjust(-1.),
            ))
            .child(
                text(move || {
                    metric
                        .value(&mut self.state.get().typography.clone())
                        .to_string()
                })
                .min_w(px(36.))
                .text_center(),
            )
            .child(icon_button(
                format!("Increase {}", name.to_lowercase()),
                Icon::ArrowUp,
                None,
                move || adjust(1.),
            ));
        if self.session.is_some() {
            row()
                .py_1()
                .justify_between()
                .child(label(name))
                .child(stepper)
        } else {
            setting(name, None, stepper)
        }
        .into_element()
    }

    pub(super) fn panel(self) -> AnyElement {
        let fonts = fonts::installed();
        let mut items = vec![(String::new(), "System font".into())];
        if let Ok(families) = &fonts {
            items.extend(families.iter().map(|f| (f.clone(), f.clone())));
        }
        let manager = signal(false);
        let mut panel = view().gap_2();
        if self.session.is_none() {
            panel = panel.child(self.opening());
        }
        panel = panel.child(self.layout_setting(
            "Font family",
            select(
                "Font family",
                items,
                move || {
                    Some(
                        self.state
                            .get()
                            .typography
                            .family
                            .clone()
                            .unwrap_or_default(),
                    )
                },
                move |value| {
                    self.commit(
                        Change::Family((!value.is_empty()).then_some(value)),
                        None,
                        |_| {},
                    )
                },
            ),
        ));
        if let Err(e) = fonts {
            panel = panel.child(note(e));
        }
        panel
            .child(self.metric(Metric::Size))
            .child(self.metric(Metric::Leading))
            .child(self.metric(Metric::Spacing))
            .child(button("Reset typography", move || {
                self.desired.replace(Typography::default());
                self.prefer(Preference::ResetType);
            }))
            .child(
                tick(
                    "Collapse blank lines".into(),
                    move || self.collapse.get(),
                    move || {
                        let next = !self.collapse.get_untracked();
                        self.collapse.set(next);
                        self.prefer(Preference::Collapse(next))
                    },
                )
                .child(label("Collapse blank lines")),
            )
            .child(button("Chapter rules", move || manager.set(true)))
            .child(text(move || self.error.get()).text_color(danger))
            .child(modal(
                manager,
                || "Chapter rules".into(),
                680.,
                move || rule_manager::manager(self),
            ))
            .into_element()
    }

    fn opening(self) -> AnyElement {
        let saved = match &self.state.get_untracked().opener {
            TxtOpener::BuiltIn => None,
            TxtOpener::External(s) => Some(s.clone()),
        };
        let program = signal(saved.clone().unwrap_or_default());
        let external = signal(saved.is_some());
        section("Open TXT files")
            .child(segmented(
                "TXT opening preference",
                vec![
                    Segment::text("reader", "Text Reader"),
                    Segment::text("external", "External application"),
                ],
                move || if external.get() { "external" } else { "reader" },
                move |key| {
                    external.set(key == "external");
                    if key == "reader" {
                        self.prefer(Preference::Opener(TxtOpener::BuiltIn));
                    }
                },
            ))
            .child(when(
                move || external.get(),
                move || {
                    view()
                        .gap_2()
                        .child(field("Application name or path", program))
                        .child(button("Save external application", move || {
                            self.prefer(Preference::Opener(TxtOpener::External(
                                program.get_untracked(),
                            )))
                        }))
                },
            ))
            .into_element()
    }
}

pub(crate) fn settings_tab(app: App) -> AnyElement {
    let store = ReaderStore::new(app.db());
    let rx = app
        .writes
        .with(|w| w.ordered(async move { store.load(None).await.map_err(|e| e.to_string()) }));
    app.load(
        async move { rx.await.map_err(|e| e.to_string())? },
        move |state| {
            let mut state = state.clone();
            resolve_font(&mut state);
            Controls::new(app, signal(Rc::new(state)), None).panel()
        },
    )
}
