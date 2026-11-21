//! Buttons: one control with a variant and a size, so every screen gets the
//! same hover, press, focus and selected looks.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Variant {
    /// The default submit of an area.
    Primary,
    /// A visible command with a keyline.
    Default,
    /// A low-emphasis toolbar or row command.
    Ghost,
    /// A destructive command (it asks first; see `overlay::confirm`).
    Danger,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonSize {
    /// 28 high: toolbars, rows, chips.
    Sm,
    /// 32 high: the default.
    Md,
}

impl ButtonSize {
    fn height(self) -> f32 {
        match self {
            Self::Sm => 28.,
            Self::Md => 32.,
        }
    }
}

/// Foreground, face and edge of a variant.
type Colours = (fn() -> Rgba, fn() -> Rgba, fn() -> Rgba);

fn colours(variant: Variant) -> Colours {
    match variant {
        Variant::Primary => (on_accent, accent, accent),
        Variant::Default => (ink, clear, keyline),
        Variant::Ghost => (ink, clear, clear),
        Variant::Danger => (danger, clear, keyline),
    }
}

/// The frame every button shares: height, corner, hover, press and focus.
fn frame(variant: Variant, size: ButtonSize) -> nsh_base::ButtonElement {
    let (_, bg, edge) = colours(variant);
    let filled = variant == Variant::Primary;
    nsh_base::button()
        .flex_row()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .min_h(px(size.height()))
        .min_w(px(size.height()))
        .rounded(px(RADIUS))
        .bg(bg)
        .border_1()
        .border_color(edge)
        .transition()
        .duration(tween(INSTANT))
        .ease_out()
        .hover(move |s| if filled { s.opacity(0.9) } else { s.bg(wash) })
        .active(move |s| {
            let s = s.scale(press_scale(0.97));
            if filled { s.opacity(0.8) } else { s.bg(rule) }
        })
        .focus(|s| {
            s.outline(px(2.))
                .outline_color(accent)
                .outline_offset(px(1.))
        })
}

/// A button with a caption and an optional leading icon.
pub fn control(
    caption: impl Into<String>,
    variant: Variant,
    size: ButtonSize,
    glyph: Option<Icon>,
    action: impl Fn() + 'static,
) -> nsh_base::ButtonElement {
    let caption = caption.into();
    let (fg, _, _) = colours(variant);
    let mut content = row().gap(px(6.));
    if let Some(glyph) = glyph {
        content = content.child(icon(glyph, fg));
    }
    frame(variant, size)
        .accessibility_label(caption.clone())
        .on_press(action)
        .self_start()
        .px(px(if size == ButtonSize::Sm { 10. } else { 12. }))
        .child(
            content.child(
                label(caption)
                    .text_size(px(if size == ButtonSize::Sm { 13. } else { 14. }))
                    .text_color(fg)
                    .font_weight(FontWeight::MEDIUM)
                    .number_of_lines(1),
            ),
        )
}

/// A default button.
pub fn button(caption: impl Into<String>, action: impl Fn() + 'static) -> nsh_base::ButtonElement {
    control(caption, Variant::Default, ButtonSize::Md, None, action)
}
/// A small default button, for rows and chips.
pub fn small(caption: impl Into<String>, action: impl Fn() + 'static) -> nsh_base::ButtonElement {
    control(caption, Variant::Default, ButtonSize::Sm, None, action)
}
/// A ghost button, for toolbars and rows.
pub fn ghost(caption: impl Into<String>, action: impl Fn() + 'static) -> nsh_base::ButtonElement {
    control(caption, Variant::Ghost, ButtonSize::Sm, None, action)
}
/// The default submit of an area.
pub fn primary(caption: impl Into<String>, action: impl Fn() + 'static) -> nsh_base::ButtonElement {
    control(caption, Variant::Primary, ButtonSize::Md, None, action)
}
/// A square, icon-only button. `name` is its accessible name and tooltip;
/// `shortcut`, when given, joins the tooltip.
pub fn icon_button(
    name: impl Into<String>,
    glyph: Icon,
    shortcut: Option<&str>,
    action: impl Fn() + 'static,
) -> nsh_base::ButtonElement {
    let name = name.into();
    let tip = match shortcut {
        Some(keys) => format!("{name}  {keys}"),
        None => name.clone(),
    };
    frame(Variant::Ghost, ButtonSize::Sm)
        .accessibility_label(name)
        .on_press(action)
        .w(px(28.))
        .h(px(28.))
        .active(|s| s.bg(rule).scale(press_scale(0.95)))
        .child(
            view()
                .w(px(28.))
                .h(px(28.))
                .items_center()
                .justify_center()
                .tooltip(hint(tip))
                .child(icon(glyph, secondary)),
        )
}

/// A choice chip: a small button that stays selected.
pub fn choice(
    caption: impl Into<String>,
    selected: impl Fn() -> bool + Copy + 'static,
    action: impl Fn() + 'static,
) -> nsh_base::ButtonElement {
    control(caption, Variant::Default, ButtonSize::Sm, None, action)
        .selected(selected)
        .bg(move || if selected() { tint() } else { clear() })
        .border_color(move || if selected() { accent() } else { keyline() })
}

/// A removable chip (an active filter, an attached tag): the whole chip
/// removes it, and its name says so.
pub fn chip(caption: impl Into<String>, action: impl Fn() + 'static) -> nsh_base::ButtonElement {
    let caption = caption.into();
    frame(Variant::Default, ButtonSize::Sm)
        .accessibility_label(format!("{caption} ×"))
        .on_press(action)
        .self_start()
        .min_h(px(26.))
        .pl(px(10.))
        .pr(px(6.))
        .rounded(px(RADIUS_SM))
        .bg(rail)
        .border_color(rule)
        .child(
            row()
                .gap_1()
                .child(label(caption).text_size(px(13.)).number_of_lines(1))
                .child(icon(Icon::Close, secondary).w(px(14.)).h(px(14.))),
        )
}
