use super::{
    controls::{Change, Controls},
    session::{Lane, ReaderSession},
};
use crate::ui::*;
use biblio_core::text_reader::book::Encoding;
use nsh_core::prelude::*;

const CHOICES: [(&str, Option<Encoding>); 7] = [
    ("Automatic", None),
    ("UTF-8", Some(Encoding::Utf8)),
    ("UTF-16 LE", Some(Encoding::Utf16Le)),
    ("UTF-16 BE", Some(Encoding::Utf16Be)),
    ("GBK", Some(Encoding::Gbk)),
    ("GB18030", Some(Encoding::Gb18030)),
    ("Big5", Some(Encoding::Big5)),
];

pub(super) fn panel(controls: Controls, session: ReaderSession, open: Signal<bool>) -> AnyElement {
    let current = controls
        .state
        .get_untracked()
        .book
        .as_ref()
        .and_then(|b| b.encoding);
    let selected = signal(current);
    let lane = Lane::with_parallelism(2);
    let sample = signal(None::<Result<String, String>>);
    effect(move || {
        let choice = selected.get();
        let book = session.book.get();
        sample.set(None);
        lane.run(
            move |stop| {
                let codec = match choice {
                    Some(codec) => codec,
                    None => book
                        .with_encoding(None, stop)
                        .map_err(|e| e.to_string())?
                        .encoding(),
                };
                book.sample(codec, stop).map_err(|e| e.to_string())
            },
            move |result| sample.set(Some(result)),
        );
    });
    let mut choices = wrap();
    for (label, codec) in CHOICES {
        choices = choices.child(choice(
            label,
            move || selected.get() == codec,
            move || selected.set(codec),
        ));
    }
    let actual = session.book.with(|b| b.encoding());
    let name = CHOICES.iter().find(|(_, c)| *c == Some(actual)).unwrap().0;
    view()
        .gap_3()
        .child(note(format!("Current encoding · {name}")))
        .child(choices)
        .child(dynamic(move || match sample.get() {
            None => note("Decoding sample…").into_element(),
            Some(Err(e)) => notice(e).into_element(),
            Some(Ok(s)) => text(s)
                .label("Encoding sample")
                .text_color(ink)
                .into_element(),
        }))
        .child(
            wrap()
                .child(when(
                    move || matches!(sample.get(), Some(Ok(_))),
                    move || {
                        primary("Confirm encoding", move || {
                            controls.commit(
                                Change::Encoding(
                                    session.id,
                                    selected.get_untracked(),
                                    session.book.get(),
                                ),
                                None,
                                move |_| open.set(false),
                            )
                        })
                    },
                ))
                .child(button("Cancel encoding", move || open.set(false))),
        )
        .child(text(move || controls.error.get()).text_color(danger))
        .into_element()
}
