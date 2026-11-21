//! Biblio's component layer over `nsh_base`: tokens, icons and the styled
//! controls every page composes. Feature modules take layout from here and
//! never style `nsh_base` controls or write colours themselves.
mod button;
mod choice;
mod field;
mod icon;
mod layout;
mod overlay;
mod theme;

pub use button::*;
pub use choice::*;
pub use field::*;
pub use icon::*;
pub use layout::*;
pub use overlay::*;
pub use theme::*;

use nsh_core::prelude::*;

/// Serif face of titles, headings and coverless slips.
pub const DISPLAY_FACE: &str = "Georgia, serif";
/// Readable measure of form pages.
pub const MEASURE: f32 = 680.;
/// Below this window width the rail folds into a top bar.
pub const COMPACT: f32 = 760.;

pub fn compact() -> bool {
    window().width < px(COMPACT)
}

pub fn row() -> View {
    view().flex_row().items_center().gap_2()
}
pub fn wrap() -> View {
    row().flex_wrap()
}
pub fn stack() -> View {
    view().gap_3().min_w_0()
}

/// Body text, 14.
pub fn label(s: impl Into<String>) -> Text {
    text(s.into()).text_size(px(14.)).text_color(ink)
}
/// Muted body text, 14.
pub fn note(s: impl Into<String>) -> Text {
    label(s).text_color(secondary)
}
/// Muted caption, 12: status, counts, small labels.
pub fn caption(s: impl Into<String>) -> Text {
    note(s).text_size(px(12.))
}
/// The page title, 26 serif.
pub fn title(s: impl Into<String>) -> Text {
    text(s.into())
        .font_family(DISPLAY_FACE)
        .text_size(px(26.))
        .text_color(ink)
        .number_of_lines(2)
}
/// A section heading, 17 serif.
pub fn subheading(s: impl Into<String>) -> Text {
    text(s.into())
        .font_family(DISPLAY_FACE)
        .text_size(px(17.))
        .text_color(ink)
}
