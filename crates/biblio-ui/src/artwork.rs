//! Small catalogue marks drawn by NSH at the host's backing scale.
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
            Self::Menu => "M3 5h18 M3 12h18 M3 19h18",
        }
    }
}

pub fn icon(which: Icon, color: fn() -> Rgba) -> impl IntoElement {
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

/// The gold tab and ruled face also appear on the application icon.
fn index_cards() -> Painter {
    Painter::new()
        .fill_color(rgb(0x839ac8))
        .fill_svg("M16 15 85 5 92 55 23 65Z")
        .fill_color(rgb(0xaebfec))
        .fill_svg("M26 9 95 15 91 66 22 60Z")
        .fill_color(gold())
        .fill_svg("M86 23h14q2 0 2 2v14H85Z")
        .fill_color(rgb(0xfff8e7))
        .fill_svg("M11 22h75q2 0 2 2v50H11q-2 0-2-2V24q0-2 2-2Z")
        .fill_color(rgb(0x3c568c))
        .fill_svg("M18 30h10v9H18Z M33 32h25v3H33Z")
        .stroke_color(rgb(0x8594b0))
        .line_width(1.3)
        .stroke_svg("M18 47h60 M18 53h60 M18 59h39")
}

pub fn index_mark(small: bool) -> impl IntoElement {
    // Keep the small mark legible by using the same silhouette at both sizes.
    canvas(if small {
        Painter::new()
            .scale(0.33, 0.33)
            .fill_color(rgb(0xb2c5ee))
            .fill_svg("M22 5h70v53H22Z")
            .fill_color(gold())
            .fill_svg("M88 18h15v26H88Z")
            .fill_color(rgb(0xfff8e7))
            .fill_svg("M5 16h74v62H5Z")
            .fill_color(rgb(0x364b75))
            .fill_svg("M16 29h14v10H16Z M35 30h28v5H35Z M16 48h46v4H16Z M16 57h34v4H16Z")
    } else {
        index_cards()
    })
    .w(px(if small { 36. } else { 112. }))
    .h(px(if small { 28. } else { 82. }))
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
                .line_width(1.5)
                .stroke_svg(STAR)
        }
    })
    .w(px(20.))
    .h(px(20.))
    .accessible(false)
}
