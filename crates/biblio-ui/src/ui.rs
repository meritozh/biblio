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
#[derive(Clone, Copy)]
pub struct Palette(pub Signal<bool>);
fn theme(light: u32, night: u32, warm: u32, warm_night: u32) -> Rgba {
    let graphite = use_context::<Palette>().is_some_and(|p| p.0.get());
    rgb(match (graphite, dark()) {
        (false, false) => light,
        (false, true) => night,
        (true, false) => warm,
        (true, true) => warm_night,
    })
}
pub fn ink() -> Rgba {
    theme(0x1b2430, 0xe0e5e9, 0x2a2723, 0xedeae4)
}
pub fn paper() -> Rgba {
    theme(0xf4f6f8, 0x192026, 0xf3f2ef, 0x171513)
}
pub fn surface() -> Rgba {
    theme(0xe8edf1, 0x28323b, 0xebe9e4, 0x2a2723)
}
pub fn label(s: impl Into<String>) -> Text {
    text(s.into()).text_sm().text_color(ink)
}
pub fn title(s: impl Into<String>) -> Text {
    text(s.into())
        .text_2xl()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(ink)
}
pub fn button(caption: impl Into<String>, action: impl Fn() + 'static) -> nsh_base::ButtonElement {
    let caption = caption.into();
    nsh_base::button()
        .accessibility_label(caption.clone())
        .px_3()
        .py_2()
        .rounded_md()
        .bg(surface)
        .hover(|s| s.opacity(0.80))
        .focus(|s| s.border_1().border_color(rgb(0x668ba4)))
        .on_press(action)
        .child(label(caption))
}
pub fn input(hint: &str, value: Signal<String>) -> Input {
    nsh_base::input()
        .label(hint.to_owned())
        .placeholder(hint.to_owned())
        .value(move || value.get())
        .auto_correct(false)
        .auto_capitalize(AutoCapitalize::None)
        .on_change(move |s: &str| value.set(s.into()))
        .bg(paper)
        .text_color(ink)
        .border_1()
        .border_color(rgb(0x8899a4))
        .rounded_md()
        .px_3()
        .py_2()
        .min_h(px(36.))
}
pub fn field(name: &str, value: Signal<String>) -> View {
    stack().gap_1().child(label(name)).child(input(name, value))
}
pub fn check(name: &str, initial: bool) -> (Signal<bool>, AnyElement) {
    let name = name.to_owned();
    let value = signal(initial);
    let element = nsh_base::checkbox()
        .checked(value)
        .accessibility_label(name.clone())
        .on_change(move |v: &bool| value.set(*v))
        .child(
            text(move || format!("{} {name}", if value.get() { "☑" } else { "☐" })).text_color(ink),
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
            row()
                .child(button(format!("Confirm {caption}"), move || {
                    pending.set(false);
                    action();
                }))
                .child(button("Cancel", move || pending.set(false)))
                .into_element()
        } else {
            button(caption.clone(), move || pending.set(true)).into_element()
        }
    })
    .into_element()
}
