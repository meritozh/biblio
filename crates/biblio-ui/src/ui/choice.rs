//! Choices: the segmented control (one of a few, visible) and the select
//! (one of many, in a list).
use super::*;
use nsh_base::SelectItem;

/// One segment: its value, its caption (also its accessible name) and an
/// optional icon. An icon segment with `show_caption` false shows only the
/// icon and keeps the caption as its name and tooltip.
#[derive(Clone)]
pub struct Segment {
    pub value: &'static str,
    pub caption: String,
    pub glyph: Option<Icon>,
    pub show_caption: bool,
}

impl Segment {
    pub fn text(value: &'static str, caption: impl Into<String>) -> Self {
        Self {
            value,
            caption: caption.into(),
            glyph: None,
            show_caption: true,
        }
    }
    pub fn icon(value: &'static str, caption: impl Into<String>, glyph: Icon) -> Self {
        Self {
            value,
            caption: caption.into(),
            glyph: Some(glyph),
            show_caption: false,
        }
    }
}

/// A segmented control: one outline whose chosen segment is raised on the
/// sheet. Arrow keys move between segments; Space and Return choose.
pub fn segmented(
    name: &str,
    segments: Vec<Segment>,
    current: impl Fn() -> &'static str + Copy + 'static,
    choose: impl Fn(&'static str) + Copy + 'static,
) -> nsh_base::ToggleGroupElement {
    let _ = name;
    let mut group = nsh_base::toggle_group()
        .self_start()
        .flex_row()
        .items_center()
        .p(px(2.))
        .gap(px(2.))
        .rounded(px(RADIUS))
        .bg(wash);
    for segment in segments {
        let value = segment.value;
        let on = move || current() == value;
        let mut content = row().gap(px(6.)).justify_center();
        if let Some(glyph) = segment.glyph {
            content = content.child(icon(glyph, move || if on() { ink() } else { secondary() }));
        }
        if segment.show_caption {
            content = content.child(
                label(segment.caption.clone())
                    .text_size(px(13.))
                    .number_of_lines(1)
                    .style_with(move |s| {
                        if on() {
                            s.text_color(ink).font_weight(FontWeight::SEMIBOLD)
                        } else {
                            s.text_color(secondary).font_weight(FontWeight::MEDIUM)
                        }
                    }),
            );
        }
        if !segment.show_caption {
            content = content.tooltip(hint(segment.caption.clone()));
        }
        let toggle = nsh_base::toggle()
            .pressed(on)
            .accessibility_label(segment.caption.clone())
            .on_change(move |_: &bool| choose(value))
            .flex_row()
            .items_center()
            .justify_center()
            .min_h(px(26.))
            .px(px(if segment.show_caption { 10. } else { 6. }))
            .rounded(px(RADIUS - 2.))
            .border_1()
            .border_color(clear)
            .transition()
            .duration(tween(FAST))
            .ease_out()
            .hover(|s| s.bg(rule))
            .pressed_style(|s| s.bg(sheet).border_color(keyline))
            .active(|s| s.scale(press_scale(0.97)))
            .focus(|s| s.outline(px(2.)).outline_color(accent));
        group = group.child(toggle.child(content));
    }
    group
}

/// A select: the trigger looks like an input with a chevron; the list is
/// the shared popup surface. `value` is the chosen item's value.
pub fn select(
    name: &str,
    items: Vec<(String, String)>,
    value: impl Fn() -> Option<String> + Copy + 'static,
    choose: impl Fn(String) + 'static,
) -> nsh_base::SelectElement {
    let titles = items.clone();
    let current = move || {
        value().and_then(|v| {
            titles
                .iter()
                .find(|(key, _)| *key == v)
                .map(|(_, title)| title.clone())
        })
    };
    nsh_base::select()
        .accessibility_label(name.to_owned())
        .items(
            items
                .into_iter()
                .map(|(v, t)| SelectItem::new(v, t))
                .collect::<Vec<_>>(),
        )
        .value(move || nsh_base::SelectValue::Controlled(value().map(SharedString::from)))
        .on_change(move |v: &Option<SharedString>| {
            if let Some(v) = v {
                choose(v.to_string());
            }
        })
        .offset(px(4.))
        .popup_style(popup)
        .item_style(|s| {
            s.px(px(10.))
                .py(px(6.))
                .rounded(px(RADIUS_SM))
                .text_color(ink)
                .text_size(px(14.))
        })
        .trigger(
            view()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_2()
                .min_h(px(32.))
                .min_w(px(200.))
                .px(px(10.))
                .rounded(px(RADIUS))
                .border_1()
                .border_color(keyline)
                .bg(sheet)
                .hover(|s| s.border_color(secondary))
                .focus(|s| s.border_color(accent).outline(px(2.)).outline_color(tint))
                .child(
                    text(move || current().unwrap_or_else(|| "Choose…".into()))
                        .text_size(px(14.))
                        .text_color(move || {
                            if value().is_some() {
                                ink()
                            } else {
                                secondary()
                            }
                        })
                        .number_of_lines(1),
                )
                .child(
                    icon(Icon::Disclosure, secondary).rotate(Radians(std::f32::consts::FRAC_PI_2)),
                ),
        )
}
