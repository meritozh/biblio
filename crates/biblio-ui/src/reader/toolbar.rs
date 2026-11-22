//! Window toolbar and the two optional reader panels.
use crate::{app::App, ui::*};
use nsh_core::{element::desktop::title_bar, prelude::*};

#[derive(Clone, Copy)]
pub(super) struct Toolbar {
    pub contents: Signal<bool>,
    pub settings: Signal<bool>,
    pub ready: Signal<bool>,
    pub status: Signal<String>,
}

impl Toolbar {
    pub fn new() -> Self {
        Self {
            contents: signal(false),
            settings: signal(false),
            ready: signal(false),
            status: signal(String::new()),
        }
    }

    pub fn header(self, app: App, name: String) -> View {
        let toggle = move |panel: Signal<bool>, other: Signal<bool>| {
            other.set(false);
            panel.update(|v| *v = !*v);
        };
        row()
            .test_id("reader-toolbar")
            .gap_2()
            .h(move || band().max(px(48.)))
            .pl(move || controls() + px(8.))
            .pr(move || title_bar().right + inset())
            .flex_shrink_0()
            .window_drag(true)
            .border_b_1()
            .border_color(rule)
            .child(icon_button(
                "Back to library",
                Icon::Back,
                Some("⌘["),
                move || super::navigation::back(app),
            ))
            .child(subheading(name).number_of_lines(1).min_w_0().flex_1())
            .child(when(
                move || self.ready.get(),
                move || {
                    row()
                        .gap_1()
                        .child(when(
                            move || !compact(),
                            move || {
                                text(move || self.status.get())
                                    .text_color(secondary)
                                    .text_size(px(12.))
                                    .max_w(px(180.))
                                    .number_of_lines(1)
                                    .mr_1()
                            },
                        ))
                        .child(
                            icon_button("Contents", Icon::List, None, move || {
                                toggle(self.contents, self.settings)
                            })
                            .selected(move || self.contents.get())
                            .bg(move || if self.contents.get() { tint() } else { clear() }),
                        )
                        .child(icon_button(
                            "Find in book",
                            Icon::Search,
                            Some("⌘F"),
                            move || {
                                app.reader.find.update(|n| *n += 1);
                            },
                        ))
                        .child(
                            icon_button("Reading settings", Icon::Typography, None, move || {
                                toggle(self.settings, self.contents)
                            })
                            .selected(move || self.settings.get())
                            .bg(move || if self.settings.get() { tint() } else { clear() }),
                        )
                },
            ))
    }
}

pub(super) fn side_panel(
    name: &'static str,
    open: Signal<bool>,
    width: f32,
    content: impl IntoElement,
) -> View {
    view()
        .w(px(width))
        .min_h_0()
        .flex_shrink_0()
        .bg(rail)
        .child(
            row()
                .px_4()
                .py_3()
                .justify_between()
                .child(subheading(name))
                .child(icon_button(
                    format!("Close {name}"),
                    Icon::Close,
                    None,
                    move || open.set(false),
                )),
        )
        .child(content)
}
