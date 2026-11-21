//! Text inputs, labelled fields and checkboxes.
use super::*;

/// The look every text input shares.
pub fn input_face() -> Input {
    nsh_base::input()
        .placeholder_color(|| Hsla::from(secondary()))
        .caret_color(|| Hsla::from(accent()))
        .bg(sheet)
        .text_color(ink)
        .text_size(px(14.))
        .border_1()
        .border_color(keyline)
        .rounded(px(RADIUS))
        .transition_colors()
        .duration(tween(INSTANT))
        .focus(|s| s.border_color(accent).outline(px(2.)).outline_color(tint))
        .px(px(10.))
        .py(px(6.))
        .min_h(px(32.))
        .min_w_0()
}
/// A single-line input bound to `value`; `hint` is its label and placeholder.
pub fn input(hint: &str, value: Signal<String>) -> Input {
    input_face()
        .label(hint.to_owned())
        .placeholder(hint.to_owned())
        .value(move || value.get())
        .auto_correct(false)
        .auto_capitalize(AutoCapitalize::None)
        .on_change(move |s: &str| value.set(s.into()))
}
/// The small muted label above a field.
pub fn field_label(name: impl Into<String>) -> Text {
    label(name)
        .text_size(px(13.))
        .text_color(secondary)
        .font_weight(FontWeight::MEDIUM)
}
/// A labelled input.
pub fn field(name: &str, value: Signal<String>) -> View {
    view()
        .gap_1()
        .min_w_0()
        .child(field_label(name))
        .child(input(name, value))
}
/// A labelled input with a line of help under it.
pub fn field_help(name: &str, help: &str, value: Signal<String>) -> View {
    field(name, value).child(caption(help.to_owned()))
}
/// A labelled multi-line input.
pub fn text_area(name: &str, value: Signal<String>) -> View {
    view()
        .gap_1()
        .min_w_0()
        .child(field_label(name))
        .child(input(name, value).multiline(true).min_h(px(84.)))
}

/// A checkbox bound to `value`, labelled `name`.
pub fn checkbox(name: &str, value: Signal<bool>) -> nsh_base::CheckboxElement {
    nsh_base::checkbox()
        .checked(value)
        .accessibility_label(name.to_owned())
        .on_change(move |v: &bool| value.set(*v))
        .self_start()
        .flex_row()
        .items_center()
        .gap_2()
        .py_1()
        .rounded(px(RADIUS_SM))
        .focus(|s| {
            s.outline(px(2.))
                .outline_color(accent)
                .outline_offset(px(2.))
        })
        .indicator_style(|s| {
            s.w(px(16.))
                .h(px(16.))
                .items_center()
                .justify_center()
                .rounded(px(RADIUS_SM))
                .border_1()
                .border_color(keyline)
                .bg(sheet)
                .checked(|s| s.bg(accent).border_color(accent))
        })
        .indicator(
            icon(
                Icon::Check,
                move || {
                    if value.get() { on_accent() } else { clear() }
                },
            )
            .w(px(12.))
            .h(px(12.)),
        )
        .child(label(name.to_owned()))
}
/// A checkbox with its own value.
pub fn check(name: &str, initial: bool) -> (Signal<bool>, AnyElement) {
    let value = signal(initial);
    (value, checkbox(name, value).into_element())
}

/// A bare tick for selecting one item; `name` is its accessible name.
pub fn tick(
    name: String,
    checked: impl Fn() -> bool + Copy + 'static,
    toggle: impl Fn() + 'static,
) -> nsh_base::CheckboxElement {
    nsh_base::checkbox()
        .checked(checked)
        .accessibility_label(name)
        .on_change(move |_: &bool| toggle())
        .self_start()
        .flex_row()
        .items_center()
        .gap_2()
        .hit_slop(6.)
        .rounded(px(RADIUS_SM))
        .focus(|s| {
            s.outline(px(2.))
                .outline_color(accent)
                .outline_offset(px(2.))
        })
        .indicator_style(|s| {
            s.w(px(16.))
                .h(px(16.))
                .items_center()
                .justify_center()
                .rounded(px(RADIUS_SM))
                .border_1()
                .border_color(keyline)
                .bg(sheet)
                .checked(|s| s.bg(accent).border_color(accent))
        })
        .indicator(
            icon(
                Icon::Check,
                move || {
                    if checked() { on_accent() } else { clear() }
                },
            )
            .w(px(12.))
            .h(px(12.)),
        )
}
