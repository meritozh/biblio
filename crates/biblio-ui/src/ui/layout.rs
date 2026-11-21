//! Page structure: headers, sections, rows, empty states and disclosure.
use super::*;

/// A page header: the title (and an optional caption beside it) on the
/// leading edge, `trailing` actions on the other, a hairline below.
pub fn page_header(
    title_text: impl Into<String>,
    aside: Option<AnyElement>,
    trailing: Option<AnyElement>,
) -> View {
    let mut lead = row()
        .gap_3()
        .flex_1()
        .min_w_0()
        .items_end()
        .child(title(title_text));
    if let Some(aside) = aside {
        lead = lead.child(view().pb(px(5.)).child(aside));
    }
    let mut bar = wrap()
        .justify_between()
        .items_end()
        .gap_3()
        .pb_3()
        .border_b_1()
        .border_color(rule)
        .child(lead);
    if let Some(trailing) = trailing {
        bar = bar.child(row().gap_2().child(trailing));
    }
    bar
}
/// The plain header of a page whose title says it all.
pub fn heading(s: impl Into<String>) -> View {
    page_header(s, None, None)
}

/// A form group: a hairline above, a serif heading, its rows under it.
pub fn section(s: impl Into<String>) -> View {
    view()
        .gap_3()
        .min_w_0()
        .mt_2()
        .pt_5()
        .border_t_1()
        .border_color(rule)
        .child(subheading(s))
}
/// A section whose heading carries one line of explanation.
pub fn section_with(s: impl Into<String>, about: impl Into<String>) -> View {
    view()
        .gap_3()
        .min_w_0()
        .mt_2()
        .pt_5()
        .border_t_1()
        .border_color(rule)
        .child(
            view()
                .gap_1()
                .child(subheading(s))
                .child(note(about.into())),
        )
}
/// The first group of a page: a heading and one line of explanation, with
/// no rule above (the page header already draws one).
pub fn group(s: impl Into<String>, about: impl Into<String>) -> View {
    view().gap_3().min_w_0().child(
        view()
            .gap_1()
            .child(subheading(s))
            .child(note(about.into())),
    )
}
/// A settings row: label (and help) on the leading side, control on the
/// trailing side; it stacks when the window is narrow.
pub fn setting(name: impl Into<String>, help: Option<&str>, control: impl IntoElement) -> View {
    let mut lead = view()
        .gap(px(2.))
        .w(px(200.))
        .flex_shrink_0()
        .child(label(name));
    if let Some(help) = help {
        lead = lead.child(caption(help.to_owned()));
    }
    wrap()
        .items_start()
        .gap_x(px(16.))
        .gap_y(px(8.))
        .py_2()
        .child(lead)
        .child(view().flex_1().min_w(px(240.)).child(control))
}
/// The empty state: the line mark, one serif sentence and the next step.
pub fn empty(heading: &str, help: &str) -> View {
    view()
        .w_full()
        .items_center()
        .justify_center()
        .gap_2()
        .px_4()
        .py_8()
        .min_h(move || px(if compact() { 220. } else { 320. }))
        .child(index_mark(false))
        .child(
            subheading(heading.to_owned())
                .text_size(px(18.))
                .text_center()
                .pt_3(),
        )
        .child(note(help.to_owned()).text_center().max_w(px(420.)))
}

/// A row that discloses content below it: chevron, caption, a count at
/// the trailing edge. Screen readers hear a button, expanded or not.
pub fn disclosure(
    caption_text: impl Into<String>,
    count: Option<String>,
    open: Signal<bool>,
) -> View {
    let caption_text = caption_text.into();
    let mut line = view()
        .label(caption_text.clone())
        .role(AccessibilityRole::Button)
        .accessibility_state(move || AccessibilityState {
            expanded: Some(open.get()),
            ..Default::default()
        })
        .on_press(move || open.update(|v| *v = !*v))
        .w_full()
        .flex_row()
        .items_center()
        .gap_2()
        .min_h(px(36.))
        .px(px(6.))
        .rounded(px(RADIUS))
        .transition_colors()
        .duration(tween(INSTANT))
        .hover(|s| s.bg(wash))
        .active(|s| s.bg(rule))
        .focus(|s| s.outline(px(2.)).outline_color(accent))
        .child(
            icon(Icon::Disclosure, secondary)
                .transition_transform()
                .duration(tween(BASE))
                .ease_in_out()
                .rotate(move || {
                    Radians(if open.get() {
                        std::f32::consts::FRAC_PI_2
                    } else {
                        0.
                    })
                }),
        )
        .child(
            label(caption_text)
                .font_weight(FontWeight::MEDIUM)
                .number_of_lines(1)
                .flex_1(),
        );
    if let Some(count) = count {
        line = line.child(caption(count));
    }
    line
}
/// A disclosure row and the content it reveals, which springs open.
pub fn reveal(
    caption_text: impl Into<String>,
    count: Option<String>,
    open: Signal<bool>,
    content: impl Fn() -> AnyElement + 'static,
) -> nsh_base::CollapsibleElement {
    nsh_base::collapsible()
        .open(open)
        .reveal(true)
        .gap_1()
        .child(disclosure(caption_text, count, open))
        .content(dynamic(move || {
            if open.get() {
                content()
            } else {
                view().into_element()
            }
        }))
}

/// A hairline.
pub fn separator() -> View {
    view().h(px(1.)).w_full().bg(rule)
}
