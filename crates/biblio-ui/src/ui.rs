use crate::artwork::{Icon, icon, tick_box};
pub use crate::theme::*;
use nsh_core::prelude::*;

pub fn row() -> View {
    view().flex_row().items_center().gap_2()
}
pub fn wrap() -> View {
    row().flex_wrap()
}
pub fn stack() -> View {
    view().gap_3().min_w_0()
}
pub const DISPLAY_FACE: &str = "Georgia, serif";
/// Readable measure for form pages.
pub const MEASURE: f32 = 680.;

pub fn label(s: impl Into<String>) -> Text {
    text(s.into()).text_size(px(14.)).text_color(ink)
}
pub fn note(s: impl Into<String>) -> Text {
    label(s).text_color(secondary)
}
/// 12-pixel secondary caption used for catalogue status and small labels.
pub fn caption(s: impl Into<String>) -> Text {
    note(s).text_size(px(12.))
}
pub fn title(s: impl Into<String>) -> Text {
    text(s.into())
        .font_family(DISPLAY_FACE)
        .text_size(px(28.))
        .text_color(ink)
}
pub fn subheading(s: impl Into<String>) -> Text {
    title(s).text_size(px(18.))
}
pub fn heading_bar() -> View {
    wrap()
        .justify_between()
        .gap_3()
        .pb_3()
        .border_b_1()
        .border_color(rule)
}
pub fn heading(s: impl Into<String>) -> View {
    heading_bar().child(title(s))
}
/// A form group heading: rule above, generous space, smaller serif title.
pub fn section(s: impl Into<String>) -> View {
    stack()
        .mt_4()
        .pt_5()
        .border_t_1()
        .border_color(rule)
        .child(subheading(s))
}
/// A fixed-width leading label for an inline row of choices.
pub fn lead(s: impl Into<String>) -> Text {
    label(s).w(px(96.)).flex_shrink_0()
}

fn face(
    caption: String,
    action: impl Fn() + 'static,
    bg: fn() -> Rgba,
    fg: fn() -> Rgba,
    edge: fn() -> Rgba,
    glyph: Option<Icon>,
    quiet: bool,
) -> nsh_base::ButtonElement {
    let mut contents = row().justify_center().gap_2();
    if let Some(glyph) = glyph {
        contents = contents.child(icon(glyph, fg));
    }
    nsh_base::button()
        .accessibility_label(caption.clone())
        .on_press(action)
        .self_start()
        .px_3()
        .py_1()
        .min_h(px(32.))
        .min_w_0()
        .justify_center()
        .rounded_md()
        .bg(bg)
        .border_1()
        .border_color(edge)
        .hover(move |s| if quiet { s.bg(wash) } else { s.opacity(0.88) })
        .active(move |s| if quiet { s.bg(rule) } else { s.opacity(0.76) })
        .focus(|s| s.outline(px(2.)).outline_color(accent))
        .child(
            contents.child(
                label(caption)
                    .text_color(fg)
                    .font_weight(FontWeight::MEDIUM),
            ),
        )
}
/// Secondary action: transparent face, keyline edge.
pub fn button(caption: impl Into<String>, action: impl Fn() + 'static) -> nsh_base::ButtonElement {
    face(caption.into(), action, clear, ink, keyline, None, true)
}
/// Secondary action with a leading glyph.
pub fn icon_button(
    caption: impl Into<String>,
    glyph: Icon,
    action: impl Fn() + 'static,
) -> nsh_base::ButtonElement {
    face(
        caption.into(),
        action,
        clear,
        ink,
        keyline,
        Some(glyph),
        true,
    )
}
/// The one principal action of a task.
pub fn primary(caption: impl Into<String>, action: impl Fn() + 'static) -> nsh_base::ButtonElement {
    face(
        caption.into(),
        action,
        accent,
        on_accent,
        accent,
        None,
        false,
    )
}
pub fn import_button(action: impl Fn() + 'static) -> nsh_base::ButtonElement {
    face(
        "Import files".into(),
        action,
        accent,
        on_accent,
        accent,
        Some(Icon::Plus),
        false,
    )
}
pub fn choice(
    caption: impl Into<String>,
    selected: impl Fn() -> bool + Copy + 'static,
    action: impl Fn() + 'static,
) -> nsh_base::ButtonElement {
    button(caption, action)
        .selected(selected)
        .bg(move || if selected() { tint() } else { clear() })
        .border_color(move || if selected() { accent() } else { keyline() })
}
pub fn input_face() -> Input {
    nsh_base::input()
        .placeholder_color(|| Hsla::from(secondary()))
        .caret_color(|| Hsla::from(accent()))
        .bg(sheet)
        .text_color(ink)
        .text_size(px(14.))
        .border_1()
        .border_color(keyline)
        .rounded_md()
        .focus(|s| s.outline(px(2.)).outline_color(accent))
        .px_3()
        .py_2()
        .min_h(px(36.))
        .min_w_0()
}
pub fn input(hint: &str, value: Signal<String>) -> Input {
    input_face()
        .label(hint.to_owned())
        .placeholder(hint.to_owned())
        .value(move || value.get())
        .auto_correct(false)
        .auto_capitalize(AutoCapitalize::None)
        .on_change(move |s: &str| value.set(s.into()))
}
pub fn field(name: &str, value: Signal<String>) -> View {
    stack()
        .gap_1()
        .child(field_label(name))
        .child(input(name, value))
}
pub fn field_label(name: impl Into<String>) -> Text {
    label(name)
        .text_size(px(13.))
        .text_color(secondary)
        .font_weight(FontWeight::MEDIUM)
}
pub fn check(name: &str, initial: bool) -> (Signal<bool>, AnyElement) {
    let name = name.to_owned();
    let value = signal(initial);
    let element = nsh_base::checkbox()
        .checked(value)
        .accessibility_label(name.clone())
        .on_change(move |v: &bool| value.set(*v))
        .self_start()
        .rounded_md()
        .focus(|s| s.outline(px(2.)).outline_color(accent))
        .child(
            row()
                .py_1()
                .child(tick_box(move || value.get()))
                .child(label(name)),
        );
    (value, element.into_element())
}
pub fn confirm(caption: impl Into<String>, action: impl Fn() + 'static) -> AnyElement {
    let caption = caption.into();
    let pending = signal(false);
    let action = std::rc::Rc::new(action);
    dynamic(move || {
        if pending.get() {
            let action = action.clone();
            wrap()
                .child(face(
                    format!("Confirm {caption}"),
                    move || {
                        pending.set(false);
                        action();
                    },
                    clear,
                    danger,
                    danger,
                    None,
                    true,
                ))
                .child(button("Cancel", move || pending.set(false)))
                .into_element()
        } else {
            button(caption.clone(), move || pending.set(true)).into_element()
        }
    })
    .into_element()
}
