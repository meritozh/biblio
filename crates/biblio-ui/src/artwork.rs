//! Small monochrome marks drawn by NSH at the host's backing scale.
use crate::theme::*;
use nsh_core::prelude::*;

#[derive(Clone, Copy)]
pub enum Icon {
    Library,
    Category,
    Import,
    Categories,
    Tags,
    Authors,
    Schemas,
    Prompts,
    Cleanup,
    Settings,
    Search,
    Plus,
    Menu,
    Back,
}

impl Icon {
    fn path(self) -> &'static str {
        match self {
            Self::Library => "M3 4h4v15H3z M10 4h3v15h-3z M15 5l3-1 4 14-3 1z",
            Self::Category => "M2.5 7h6l2-3h4l2 3h4v12h-18z",
            Self::Import => "M12 2v12m-4-4 4 4 4-4 M4 14v5h16v-5",
            Self::Categories => "M3 5h6l2 3h10v11H3z",
            Self::Tags => "M3 3h8l10 10-8 8L3 11z M7 7h1",
            Self::Authors => "M16 7a4 4 0 1 1-8 0 4 4 0 1 1 8 0 M4 20v-2c0-6 16-6 16 0v2",
            Self::Schemas => "M3 4h18v16H3z M3 9h18 M9 9v11",
            Self::Prompts => "M3 4h18v13H9l-5 4v-4H3z M7 9l3 2-3 2 M12 13h4",
            Self::Cleanup => {
                "M3 20l9-9 3 3-9 6z M13 5l1-3 1 3 3 1-3 1-1 3-1-3-3-1z M18 12h4m-2-2v4"
            }
            Self::Settings => {
                "M3 6h5m4 0h9 M3 12h11m4 0h3 M3 18h3m4 0h11 M8 4v4h4V4z M14 10v4h4v-4z M6 16v4h4v-4z"
            }
            Self::Search => "M16 10a6 6 0 1 1-12 0 6 6 0 1 1 12 0 M15 15l6 6",
            Self::Plus => "M12 4v16 M4 12h16",
            Self::Menu => "M3 6h18 M3 12h18 M3 18h18",
            Self::Back => "M15 5l-7 7 7 7",
        }
    }
}

pub fn icon(which: Icon, color: impl Fn() -> Rgba + 'static) -> impl IntoElement {
    canvas(move || {
        Painter::new()
            .scale(0.75, 0.75)
            .stroke_color(color())
            .line_width(1.7)
            .stroke_svg(which.path())
    })
    .w(px(18.))
    .h(px(18.))
    .flex_shrink_0()
    .accessible(false)
}

/// The app icon's ruled slip and index tab, reduced to a line drawing: an
/// outlined slip in ink with a filled accent tab. `small` is the wordmark size.
pub fn index_mark(small: bool) -> impl IntoElement {
    let (w, h, scale) = if small {
        (20., 16., 0.2)
    } else {
        (64., 52., 0.64)
    };
    canvas(move || {
        let back = if small { ink() } else { keyline() };
        Painter::new()
            .scale(scale, scale)
            .stroke_color(back)
            .line_width(if small { 7. } else { 2.6 })
            .stroke_svg("M18 8h70v54")
            .fill_color(accent())
            .fill_svg("M84 22h14v20H84Z")
            .stroke_color(ink())
            .line_width(if small { 8. } else { 3. })
            .stroke_svg("M6 18h76v58H6Z")
            .stroke_color(secondary())
            .line_width(if small { 7. } else { 2.4 })
            .stroke_svg(if small {
                "M18 36h46 M18 54h30"
            } else {
                "M18 34h52 M18 46h52 M18 58h32"
            })
    })
    .w(px(w))
    .h(px(h))
    .flex_shrink_0()
    .accessible(false)
}

pub fn favorite_mark(selected: bool) -> impl IntoElement {
    const STAR: &str = "M10 1.5l2.6 5.3 5.9.8-4.3 4.2 1 5.9-5.2-2.8-5.2 2.8 1-5.9L1.5 7.6l5.9-.8Z";
    canvas(move || {
        if selected {
            Painter::new().fill_color(accent()).fill_svg(STAR)
        } else {
            Painter::new()
                .stroke_color(secondary())
                .line_width(1.4)
                .stroke_svg(STAR)
        }
    })
    .w(px(18.))
    .h(px(18.))
    .accessible(false)
}

/// A drawn checkbox face; the surrounding control carries the semantics.
pub fn tick_box(checked: impl Fn() -> bool + 'static) -> impl IntoElement {
    canvas(move || {
        if checked() {
            Painter::new()
                .fill_color(accent())
                .fill_svg("M3 1h10q2 0 2 2v10q0 2-2 2H3q-2 0-2-2V3q0-2 2-2Z")
                .stroke_color(on_accent())
                .line_width(1.8)
                .stroke_svg("M4.5 8.2l2.4 2.4 4.6-5")
        } else {
            Painter::new()
                .fill_color(sheet())
                .fill_svg("M3 1h10q2 0 2 2v10q0 2-2 2H3q-2 0-2-2V3q0-2 2-2Z")
                .stroke_color(keyline())
                .line_width(1.2)
                .stroke_svg(
                    "M3 1.5h10q1.5 0 1.5 1.5v10q0 1.5-1.5 1.5H3q-1.5 0-1.5-1.5V3q0-1.5 1.5-1.5Z",
                )
        }
    })
    .w(px(16.))
    .h(px(16.))
    .flex_shrink_0()
    .accessible(false)
}
