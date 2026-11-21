//! One stroke icon set on a 24-unit grid, drawn at 16 points in a fixed
//! slot, plus the monochrome line mark and the favourite star.
use super::theme::*;
use nsh_core::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
    Forward,
    Disclosure,
    More,
    Sort,
    Grid,
    List,
    Filter,
    Check,
    Close,
    Select,
    Shuffle,
    Copy,
    Reveal,
    Trash,
    Open,
    Cover,
    Download,
    Upload,
    Star,
}

impl Icon {
    fn path(self) -> &'static str {
        match self {
            Self::Library => "M4 4h4v16H4z M10.5 4h3.5v16h-3.5z M16 5.2l3.2-.9 3 14.6-3.2.9z",
            Self::Category => "M3 6.5h6l2 2.5h10v10.5H3z",
            Self::Import => "M12 3v12 M7.5 10.5 12 15l4.5-4.5 M4 15v5h16v-5",
            Self::Categories => "M3 8h6l2 2.5h10V20H3z M5 4.5h5l1.5 2",
            Self::Tags => "M3 3h8.5l9.5 9.5-8.5 8.5L3 11.5z M7.5 7.5h.5",
            Self::Authors => "M16 7.5a4 4 0 1 1-8 0 4 4 0 1 1 8 0 M4.5 20.5c0-6 15-6 15 0",
            Self::Schemas => "M3.5 4h17v16h-17z M3.5 9.5h17 M9.5 9.5V20",
            Self::Prompts => "M3.5 4h17v12.5H9.5L4.5 20.5V16.5h-1z M7.5 8.5l2.5 2-2.5 2 M12 12.5h4",
            Self::Cleanup => {
                "M3 21l9-9 3 3-9 6z M14 3.5l1 2.5 2.5 1-2.5 1-1 2.5-1-2.5-2.5-1 2.5-1z M18.5 12.5h3.5m-1.75-1.75v3.5"
            }
            Self::Settings => {
                "M3 6.5h5m4 0h9 M3 12h11m4 0h3 M3 17.5h3m4 0h11 M8 4.5v4h4v-4z M14 10v4h4v-4z M6 15.5v4h4v-4z"
            }
            Self::Search => "M16.5 10.5a6 6 0 1 1-12 0 6 6 0 1 1 12 0 M15 15l5.5 5.5",
            Self::Plus => "M12 4.5v15 M4.5 12h15",
            Self::Menu => "M3.5 6.5h17 M3.5 12h17 M3.5 17.5h17",
            Self::Back => "M14.5 5.5 8 12l6.5 6.5",
            Self::Forward => "M9.5 5.5 16 12l-6.5 6.5",
            Self::Disclosure => "M9.5 5.5 16 12l-6.5 6.5",
            Self::More => {
                "M6.5 12a1.2 1.2 0 1 1-2.4 0 1.2 1.2 0 1 1 2.4 0 M13.2 12a1.2 1.2 0 1 1-2.4 0 1.2 1.2 0 1 1 2.4 0 M19.9 12a1.2 1.2 0 1 1-2.4 0 1.2 1.2 0 1 1 2.4 0"
            }
            Self::Sort => "M7.5 4v16 M4 16.5 7.5 20l3.5-3.5 M16.5 20V4 M13 7.5 16.5 4 20 7.5",
            Self::Grid => {
                "M4 4h6.5v6.5H4z M13.5 4H20v6.5h-6.5z M4 13.5h6.5V20H4z M13.5 13.5H20V20h-6.5z"
            }
            Self::List => "M8.5 6h12 M8.5 12h12 M8.5 18h12 M3.5 6h2 M3.5 12h2 M3.5 18h2",
            Self::Filter => "M3.5 5h17l-6.5 8v6l-4 2v-8z",
            Self::Check => "M5 12.5l4.5 4.5L19 7.5",
            Self::Close => "M6.5 6.5l11 11 M17.5 6.5l-11 11",
            Self::Select => "M4 4h16v16H4z M8 12.5l3 3 5-6.5",
            Self::Shuffle => {
                "M3 7h3.5l11 10H21 M3 17h3.5l3.5-3.2 M14 10.2 17.5 7H21 M18.5 4.5 21 7l-2.5 2.5 M18.5 14.5 21 17l-2.5 2.5"
            }
            Self::Copy => "M8.5 8.5h11.5v11.5H8.5z M15.5 8.5V4H4v11.5h4.5",
            Self::Reveal => "M3 6.5h6l2 2.5h10v10.5H3z M12 12.5v4 M10 14.5l2 2 2-2",
            Self::Trash => "M4 7h16 M9.5 7V4h5v3 M6 7l1 13.5h10L18 7 M10 11v5.5 M14 11v5.5",
            Self::Open => "M13.5 4H20v6.5 M20 4l-9 9 M18 14v6H4V6h6",
            Self::Cover => "M3.5 4.5h17v15h-17z M3.5 16l5-5 4 4 3-3 5 5 M15 8.5h.5",
            Self::Download => "M12 3.5v12 M7.5 11 12 15.5l4.5-4.5 M4.5 20h15",
            Self::Upload => "M12 15.5v-12 M7.5 8 12 3.5 16.5 8 M4.5 20h15",
            Self::Star => {
                "M12 3.2l2.7 5.6 6.1.9-4.4 4.3 1 6.1L12 17.2l-5.4 2.9 1-6.1-4.4-4.3 6.1-.9Z"
            }
        }
    }
}

/// `which` at 16 points, stroked in `color`.
pub fn icon(which: Icon, color: impl Fn() -> Rgba + 'static) -> View {
    view()
        .w(px(16.))
        .h(px(16.))
        .flex_shrink_0()
        .accessible(false)
        .child(
            canvas(move || {
                Painter::new()
                    .scale(2. / 3., 2. / 3.)
                    .stroke_color(color())
                    .line_width(2.)
                    .stroke_svg(which.path())
            })
            .w(px(16.))
            .h(px(16.)),
        )
}

/// The app icon's ruled slip and index tab as a line drawing: an outlined
/// slip in ink with a filled accent tab. `small` is the wordmark size.
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

/// The favourite star: filled in the accent when `selected`.
pub fn favorite_mark(selected: bool) -> impl IntoElement {
    canvas(move || {
        let star = Icon::Star.path();
        if selected {
            Painter::new()
                .scale(2. / 3., 2. / 3.)
                .fill_color(accent())
                .fill_svg(star)
                .stroke_color(accent())
                .line_width(2.)
                .stroke_svg(star)
        } else {
            Painter::new()
                .scale(2. / 3., 2. / 3.)
                .stroke_color(secondary())
                .line_width(2.)
                .stroke_svg(star)
        }
    })
    .w(px(16.))
    .h(px(16.))
    .flex_shrink_0()
    .accessible(false)
}
