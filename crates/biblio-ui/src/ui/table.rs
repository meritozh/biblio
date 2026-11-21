//! Tables: nsh-base's `DataTable` in the app's tokens. Only the rows in
//! view are made, so a list of thousands of names scrolls as one of ten,
//! and a table that holds part of its rows asks for more as it nears the
//! last (`has_more`, `on_load_more`).
//!
//! The table takes presses on a layer over its cells, so a cell cannot
//! hold a button of its own: a command on a row is a narrow column of its
//! own ([`Col::command`]) whose press the page handles by the column's key.
use super::*;
use nsh_base::{
    DataTableAlign, DataTableCell, DataTableClasses, DataTableColumn, DataTableElement,
    DataTableRows,
};
use std::rc::Rc;

/// The height of a body row.
pub const ROW_HEIGHT: f32 = 44.;
/// The height of the title row.
pub const HEADER_HEIGHT: f32 = 38.;

/// How a column's cells look.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CellKind {
    /// The row's text in the body face.
    Text,
    /// The row's text in the muted caption face (counts, dates, slugs).
    Muted,
    /// The row's text in a monospaced face (folders, keys).
    Mono,
    /// One icon command, drawn when the row's text is not empty; the text
    /// is the command's accessible name ("Edit Fantasy").
    Command(Icon),
    /// Drawn by the page's own `cell`.
    Custom,
}

/// A column of a [`table`].
#[derive(Clone)]
pub struct Col {
    pub key: &'static str,
    pub title: &'static str,
    /// Its width; `None` takes the room the other columns leave.
    pub width: Option<f32>,
    pub align: DataTableAlign,
    pub kind: CellKind,
}

impl Col {
    /// A column of text, `width` points wide (`None`: the rest).
    pub fn text(key: &'static str, title: &'static str, width: Option<f32>) -> Self {
        Self {
            key,
            title,
            width,
            align: DataTableAlign::Start,
            kind: CellKind::Text,
        }
    }
    pub fn muted(key: &'static str, title: &'static str, width: Option<f32>) -> Self {
        Self {
            kind: CellKind::Muted,
            ..Self::text(key, title, width)
        }
    }
    pub fn mono(key: &'static str, title: &'static str, width: Option<f32>) -> Self {
        Self {
            kind: CellKind::Mono,
            ..Self::text(key, title, width)
        }
    }
    pub fn custom(key: &'static str, title: &'static str, width: Option<f32>) -> Self {
        Self {
            kind: CellKind::Custom,
            ..Self::text(key, title, width)
        }
    }
    /// A narrow column of one icon command; its cells do not select.
    pub fn command(key: &'static str, glyph: Icon) -> Self {
        Self {
            key,
            title: "",
            width: Some(40.),
            align: DataTableAlign::Center,
            kind: CellKind::Command(glyph),
        }
    }
    /// Numbers and counts line up at the end.
    pub fn end(mut self) -> Self {
        self.align = DataTableAlign::End;
        self
    }
}

/// The part classes in the current tokens.
fn classes() -> DataTableClasses {
    let c = colors();
    let mut k = DataTableClasses::default();
    k.root = Some(
        format!(
            "rounded-md border border-[{}] bg-[{}] text-[{}]",
            hex(c.rule),
            hex(c.paper),
            hex(c.ink)
        )
        .into(),
    );
    k.header = Some(format!("border-b border-[{}] bg-[{}]", hex(c.rule), hex(c.rail)).into());
    k.header_cell =
        Some(format!("px-3 text-[13px] font-medium text-[{}]", hex(c.secondary)).into());
    k.row = Some(format!("border-b border-[{}]", hex(c.rule)).into());
    k.row_hover = Some(format!("bg-[{}]", hex(c.wash)).into());
    k.row_selected = Some(format!("bg-[{}]", hex(c.tint)).into());
    k.row_active = Some(format!("bg-[{}]", hex(c.wash)).into());
    k.cell = Some("px-3".into());
    k
}

/// The columns' definitions for a table `width` points wide: the flexible
/// column takes what the fixed ones leave (never under 160).
fn definitions(cols: &[Col], width: f32) -> Vec<DataTableColumn> {
    let fixed: f32 = cols.iter().filter_map(|c| c.width).sum();
    let flexible = cols.iter().filter(|c| c.width.is_none()).count().max(1) as f32;
    // The keyline on each side.
    let rest = ((width - fixed - 2.) / flexible).max(160.);
    cols.iter()
        .map(|c| {
            let mut d = DataTableColumn::default();
            d.key = c.key.into();
            d.title = Some(c.title.into());
            d.width = Some(px(c.width.unwrap_or(rest)));
            d.align = c.align;
            d.resizable = Some(false);
            d.movable = Some(false);
            if matches!(c.kind, CellKind::Command(_)) {
                d.selectable = Some(false);
            }
            d
        })
        .collect()
}

/// A table of `count` rows whose cells `rows` gives, by the columns'
/// keys. `cell` draws the [`CellKind::Custom`] columns; `events` adds the
/// page's handlers (presses, loading more). Its height fits its rows, up
/// to the room its parent leaves (then it scrolls); rows select on a
/// press, and the arrow keys move the selection.
pub fn table(
    name: &str,
    cols: Vec<Col>,
    count: impl Fn() -> usize + Copy + 'static,
    rows: impl Fn() -> DataTableRows + 'static,
    cell: impl Fn(&DataTableCell) -> AnyElement + 'static,
    events: impl FnOnce(DataTableElement) -> DataTableElement,
) -> View {
    let width = signal(0f32);
    let cols = Rc::new(cols);
    let shown = cols.clone();
    let kinds = cols.clone();
    let cell = Rc::new(cell);
    let body = nsh_base::data_table()
        .accessibility_label(name.to_owned())
        .w_full()
        .h_full()
        .columns(move || definitions(&shown, width.get()))
        .data(rows)
        .row_height(px(ROW_HEIGHT))
        .header_height(Some(px(HEADER_HEIGHT)))
        .column_selectable(false)
        .resizable_columns(false)
        .movable_columns(false)
        .classes(classes)
        .render_cell(move |c: &DataTableCell| -> AnyElement {
            let (kind, align) = kinds
                .get(c.column as usize)
                .map_or((CellKind::Text, DataTableAlign::Start), |col| {
                    (col.kind, col.align)
                });
            // The cell's content fills it: its text lines up as the
            // column does.
            let aligned = move |t: Text| match align {
                DataTableAlign::End => t.text_right(),
                DataTableAlign::Center => t.text_center(),
                DataTableAlign::Start => t,
            };
            match kind {
                CellKind::Text => {
                    aligned(label(c.text.to_string()).number_of_lines(1)).into_element()
                }
                CellKind::Muted => aligned(
                    caption(c.text.to_string())
                        .text_size(px(13.))
                        .number_of_lines(1),
                )
                .into_element(),
                CellKind::Mono => aligned(
                    note(c.text.to_string())
                        .font_family("Menlo, monospace")
                        .text_size(px(13.))
                        .number_of_lines(1),
                )
                .into_element(),
                CellKind::Command(glyph) if !c.text.is_empty() => view()
                    .label(c.text.to_string())
                    .role(AccessibilityRole::Button)
                    .w(px(28.))
                    .h(px(28.))
                    .items_center()
                    .justify_center()
                    .rounded(px(RADIUS))
                    .child(icon(glyph, move || {
                        if glyph == Icon::Trash {
                            danger()
                        } else {
                            secondary()
                        }
                    }))
                    .into_element(),
                CellKind::Command(_) => view().into_element(),
                CellKind::Custom => cell(c),
            }
        });
    view()
        .test_id(&format!("table:{name}"))
        .w_full()
        .min_h_0()
        .flex_shrink(1.)
        .h(move || px(HEADER_HEIGHT + ROW_HEIGHT * count() as f32 + 2.))
        .on_layout(move |b: &Bounds<Points>| width.set(b.size.width.0))
        .child(events(body))
}

/// A small label on a quiet fill: a schema, a scope, "Built-in".
pub fn badge(s: impl Into<String>) -> View {
    view()
        .flex_shrink_0()
        .px(px(8.))
        .py(px(2.))
        .rounded(px(RADIUS_SM))
        .bg(wash)
        .child(
            label(s)
                .text_size(px(12.))
                .text_color(secondary)
                .font_weight(FontWeight::MEDIUM)
                .number_of_lines(1),
        )
}

/// The key of the column a press landed on.
pub fn pressed_key(
    cols: &[&'static str],
    press: &nsh_base::DataTablePress,
) -> Option<&'static str> {
    press.column.and_then(|c| cols.get(c as usize).copied())
}
