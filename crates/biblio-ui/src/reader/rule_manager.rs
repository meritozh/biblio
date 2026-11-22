//! Selecting a card owns a disposable outline. Only Apply publishes that
//! exact outline, after the book selection has reached the ordered writer.
mod editor;

use super::{
    contents,
    controls::{Change, Controls},
    session::{Lane, ReaderSession},
};
use crate::ui::*;
use biblio_core::text_reader::{
    rules::{AUTO_ID, Contents, RuleDraft, SavedRule, preset_example},
    store::{BookRule, DefaultRule, Preference, RuleEdit},
};
use nsh_core::prelude::*;
use std::rc::Rc;

const AUTO_NAME: &str = "自动检测";

fn default_for(id: &str) -> DefaultRule {
    if id == AUTO_ID {
        DefaultRule::Auto
    } else {
        DefaultRule::Fixed(id.into())
    }
}

#[derive(Clone, Copy)]
struct Manager {
    controls: Controls,
    selected: Signal<Option<String>>,
    editing: Signal<Option<(Option<String>, RuleDraft)>>,
    owner: Owner,
    previews: Lane,
}

pub(super) fn manager(controls: Controls) -> AnyElement {
    let m = Manager {
        controls,
        selected: signal(None),
        editing: signal(None),
        owner: Owner::current(),
        previews: Lane::with_parallelism(2),
    };
    dynamic(move || {
        if let Some((id, draft)) = m.editing.get() {
            untracked(|| editor::form(m, id, draft))
        } else {
            untracked(|| m.browse())
        }
    })
    .into_element()
}

impl Manager {
    fn commit(
        self,
        change: Change,
        preview: Option<Rc<Contents>>,
        done: impl FnOnce(Option<String>) + 'static,
    ) {
        self.owner
            .run(|| self.controls.commit(change, preview, done));
    }

    fn browse(self) -> AnyElement {
        let state = self.controls.state;
        let selected_rule = computed(move || {
            let id = self.selected.get()?;
            if id == AUTO_ID {
                Some((id, None))
            } else {
                state
                    .with(|s| s.rules.iter().find(|r| r.id == id).cloned())
                    .map(|r| (id, Some(r)))
            }
        });
        let rules = computed(move || state.get().rules.clone());
        view()
            .gap_3()
            .child(
                text(move || {
                    let s = state.get();
                    let name = |r: &DefaultRule| match r {
                        DefaultRule::Auto => AUTO_NAME.into(),
                        DefaultRule::Fixed(id) => s
                            .rules
                            .iter()
                            .find(|r| &r.id == id)
                            .map(|r| r.draft.name.clone())
                            .unwrap_or_default(),
                    };
                    if self.controls.session.is_some() {
                        format!(
                            "Default · {}   This book · {}",
                            name(&s.default_rule),
                            name(&s.effective_rule())
                        )
                    } else {
                        format!("Default · {}", name(&s.default_rule))
                    }
                })
                .text_color(secondary),
            )
            .child(button("New rule", move || {
                self.editing.set(Some((None, editor::blank())))
            }))
            .child(dynamic(move || {
                selected_rule.get().map_or_else(
                    || view().into_element(),
                    |(id, rule)| untracked(|| self.inspector(id, rule)),
                )
            }))
            .child(text(move || self.controls.error.get()).text_color(danger))
            .child(dynamic(move || {
                let rules = rules.get();
                untracked(|| {
                    let mut cards = view().gap_2().child(self.card(AUTO_ID, AUTO_NAME));
                    for rule in rules {
                        cards = cards.child(self.card(&rule.id, &rule.draft.name));
                    }
                    cards
                })
            }))
            .into_element()
    }

    fn card(self, id: &str, name: &str) -> AnyElement {
        let id = stored(id.to_string());
        let on = move || self.selected.get().as_ref() == Some(&id.get());
        let mut card = nsh_base::button()
            .accessibility_label(name.to_string())
            .selected(on)
            .on_press(move || self.selected.set(Some(id.get())))
            .items_start()
            .border_1()
            .border_color(move || if on() { accent() } else { rule() })
            .bg(move || if on() { tint() } else { sheet() })
            .focus(|s| s.outline(px(2.)).outline_color(accent))
            .rounded(px(RADIUS))
            .p_2()
            .gap_1()
            .child(label(name.to_string()));
        if let Some(s) = self.controls.session {
            card = card.child(dynamic(move || {
                let Some(survey) = s.survey.get() else {
                    return text(move || s.scan_status.get()).into_element();
                };
                let Some(card) = survey.cards.iter().find(|c| c.id == id.get()) else {
                    return note("Scanning chapters…").into_element();
                };
                let stats = match &card.stats {
                    Ok(stats) => stats,
                    Err(e) => return notice(e.to_string()).into_element(),
                };
                let mut details = view().gap_1().child(note(counted(
                    stats.chapters as i64,
                    "chapter",
                    "chapters",
                )));
                if card.id == AUTO_ID {
                    let detected = survey
                        .cards
                        .iter()
                        .find(|r| Some(&r.id) == survey.detected.as_ref())
                        .map_or("No matching preset", |r| r.name.as_str());
                    details = details.child(note(format!("Detected · {detected}")));
                }
                if let Some(example) = stats.example.clone() {
                    details = details.child(untracked(|| {
                        contents::title_text(s, example, |s| {
                            caption(s).number_of_lines(1).into_element()
                        })
                    }));
                } else {
                    details = details.child(caption("No matching example"));
                }
                details.into_element()
            }));
        } else if let Some(example) = preset_example(&id.get()) {
            card = card.child(caption(example));
        }
        card.into_element()
    }

    fn inspector(self, id: String, rule: Option<SavedRule>) -> AnyElement {
        let name = rule
            .as_ref()
            .map_or(AUTO_NAME, |r| r.draft.name.as_str())
            .to_string();
        let selected = stored(id);
        let mut actions = wrap().child(button("Set as default", move || {
            self.commit(
                Change::Preference(Preference::Default(default_for(&selected.get()))),
                None,
                |_| {},
            )
        }));
        if let Some(rule) = rule {
            let custom = preset_example(&rule.id).is_none();
            let draft = stored(rule.draft);
            actions = actions.child(button("Duplicate rule", move || {
                let mut copy = draft.get();
                copy.name = format!("{} copy", copy.name);
                self.editing.set(Some((None, copy)));
            }));
            if custom {
                actions = actions
                    .child(button("Edit rule", move || {
                        self.editing.set(Some((Some(selected.get()), draft.get())))
                    }))
                    .child(button("Delete rule", move || {
                        self.commit(
                            Change::Rule(RuleEdit::Remove(selected.get())),
                            None,
                            move |_| self.selected.set(None),
                        )
                    }));
            }
        }
        let mut panel = view().gap_2().child(subheading(name)).child(actions);
        if let Some(s) = self.controls.session {
            panel = panel.child(self.preview(s, selected.get()));
        }
        panel
            .child(button("Cancel selection", move || self.selected.set(None)))
            .into_element()
    }

    fn preview(self, session: ReaderSession, id: String) -> AnyElement {
        let result = signal_with(None::<Rc<Contents>>, SignalEquals::Never);
        let status = signal(String::new());
        let expected = stored(None::<SavedRule>);
        let lane = self.previews;
        cleanup(move || lane.cancel());
        let key = stored(id);
        let candidate = computed(move || {
            let id = key.get();
            let state = self.controls.state.get();
            let selected = if id == AUTO_ID {
                let Some(survey) = session.survey.get() else {
                    return Err(session.scan_status.get());
                };
                survey.detected.clone()
            } else {
                Some(id)
            };
            Ok(selected.and_then(|id| state.rules.iter().find(|r| r.id == id).cloned()))
        });
        effect(move || {
            let draft = match candidate.get() {
                Ok(rule) => rule,
                Err(message) => {
                    lane.cancel();
                    result.set(None);
                    status.set(message);
                    return;
                }
            };
            expected.replace(draft.clone());
            lane.cancel();
            result.set(None);
            let Some(rule) = draft else {
                status.set("0 chapters".into());
                result.set(Some(Rc::new(Contents::default())));
                return;
            };
            status.set("Preparing preview…".into());
            let book = session.book.get();
            let scan = session.outliner.get();
            lane.run(
                move |stop| (scan.0)(&book, &rule.draft, stop),
                move |r| match r {
                    Err(e) => status.set(e),
                    Ok(c) => {
                        status.set(counted(c.chapters as i64, "chapter", "chapters"));
                        result.set(Some(Rc::new(c)));
                    }
                },
            );
        });
        view()
            .gap_2()
            .child(text(move || format!("Preview · {}", status.get())))
            .child(when(
                move || result.with(|r| r.is_some()),
                move || {
                    view()
                        .gap_2()
                        .child(contents::list(session, result, None).h(px(168.)))
                        .child(primary("Apply to this book", move || {
                            let id = key.get();
                            let submitted = id.clone();
                            let selection = if id == AUTO_ID {
                                BookRule::Auto
                            } else {
                                BookRule::Fixed(id)
                            };
                            self.commit(
                                Change::Apply {
                                    id: session.id,
                                    selection,
                                    expected: expected.get(),
                                    encoding: self
                                        .controls
                                        .state
                                        .get_untracked()
                                        .book
                                        .as_ref()
                                        .and_then(|b| b.encoding),
                                    book: session.book.get(),
                                },
                                result.get_untracked(),
                                move |_| {
                                    if self.selected.get_untracked().as_ref() == Some(&submitted) {
                                        self.selected.set(None);
                                    }
                                },
                            );
                        }))
                },
            ))
            .into_element()
    }
}
