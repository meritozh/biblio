use super::*;
use biblio_core::text_reader::rules::{ChapterRule, Heading, PatternMode};

pub(super) fn blank() -> RuleDraft {
    RuleDraft {
        name: String::new(),
        mode: PatternMode::Template,
        chapter: String::new(),
        volume: None,
    }
}

#[derive(Clone, Copy)]
struct Expression {
    value: Signal<String>,
    selection: Stored<Selection>,
    element: ElementRef,
}

impl Expression {
    fn new(value: String) -> Self {
        let end = value.encode_utf16().count() as u32;
        Self {
            value: signal(value),
            selection: stored(Selection { start: end, end }),
            element: element_ref(),
        }
    }

    fn input(self, name: &str, target: Stored<Self>) -> Input {
        input(name, self.value)
            .element_ref(&self.element)
            .on_focus(move || {
                target.replace(self);
            })
            .on_selection_change(move |range: &Selection| {
                self.selection.replace(*range);
            })
    }

    fn insert(self, token: &str) {
        use nsh_core::text_edit::{byte_to_utf16, utf16_to_byte};
        let range = self.selection.get();
        let mut text = self.value.get_untracked();
        let a = utf16_to_byte(&text, range.start.min(range.end));
        let b = utf16_to_byte(&text, range.start.max(range.end));
        text.replace_range(a..b, token);
        let at = byte_to_utf16(&text, a + token.len());
        self.value.set(text);
        self.selection.replace(Selection { start: at, end: at });
        self.element.focus();
        self.element.set_selection(at, at);
    }
}

pub(super) fn form(m: Manager, id: Option<String>, draft: RuleDraft) -> AnyElement {
    let id = stored(id);
    let name = signal(draft.name);
    let mode = signal(draft.mode);
    let chapter = Expression::new(draft.chapter);
    let volume = Expression::new(draft.volume.unwrap_or_default());
    let target = stored(chapter);
    let sample = signal(String::new());
    let result = signal(String::new());
    let draft = move || RuleDraft {
        name: name.get_untracked().trim().to_owned(),
        mode: mode.get_untracked(),
        chapter: chapter.value.get_untracked(),
        volume: Some(volume.value.get_untracked()).filter(|s| !s.is_empty()),
    };
    let saving = signal(false);
    effect(move || {
        if !m.controls.error.get().is_empty() {
            saving.set(false);
        }
    });
    let save = move |make_default| {
        if saving.get_untracked() {
            return;
        }

        let draft = draft();
        if draft.name.is_empty() {
            result.set("Enter a rule name".into());
            return;
        }
        if let Err(e) = ChapterRule::compile(&draft) {
            result.set(e.to_string());
            return;
        }
        saving.set(true);
        m.commit(
            Change::Rule(RuleEdit::Save {
                id: id.get(),
                draft,
                make_default,
            }),
            None,
            move |id| {
                m.selected.set(id);
                m.editing.set(None);
            },
        );
    };
    view()
        .gap_2()
        .child(field("Rule name", name))
        .child(segmented(
            "Pattern mode",
            vec![
                Segment::text("template", "Template"),
                Segment::text("regex", "Regular expression"),
            ],
            move || {
                if mode.get() == PatternMode::Template {
                    "template"
                } else {
                    "regex"
                }
            },
            move |key| {
                mode.set(if key == "template" {
                    PatternMode::Template
                } else {
                    PatternMode::Regex
                })
            },
        ))
        .child(field_label("Chapter expression"))
        .child(chapter.input("Chapter expression", target))
        .child(field_label("Volume expression (optional)"))
        .child(volume.input("Volume expression", target))
        .child(when(
            move || mode.get() == PatternMode::Template,
            move || {
                let mut buttons = wrap();
                for token in ["{序号}", "{标题}"] {
                    buttons = buttons.child(small(token, move || target.get().insert(token)));
                }
                buttons
            },
        ))
        .child(field("Example heading", sample))
        .child(button("Test example", move || {
            let answer =
                ChapterRule::compile(&draft()).and_then(|r| r.probe(&sample.get_untracked()));
            result.set(match answer {
                Ok(Some(Heading::Volume)) => "Example · Volume".into(),
                Ok(Some(Heading::Chapter)) => "Example · Chapter".into(),
                Ok(None) => "Example · No match".into(),
                Err(e) => e.to_string(),
            })
        }))
        .child(text(move || result.get()))
        .child(
            wrap()
                .child(primary("Save rule", move || save(false)).disabled(saving))
                .child(button("Save and set default", move || save(true)).disabled(saving))
                .child(ghost("Cancel edit", move || m.editing.set(None)).disabled(saving)),
        )
        .child(text(move || m.controls.error.get()).text_color(danger))
        .into_element()
}
