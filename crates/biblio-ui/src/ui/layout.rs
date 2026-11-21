//! Page structure: headers, sections, rows, empty states and disclosure.
use super::*;
use nsh_core::{element::desktop::title_bar, platform::Os};

/// The height of the band the hidden title bar leaves at the top of the
/// window: what the host reports, else the height asked for on macOS.
pub fn band() -> Points {
    let reported = title_bar().top;
    if reported > px(0.) {
        reported
    } else if os() == Os::MacOs {
        px(52.)
    } else {
        px(0.)
    }
}
/// The width the window controls take at the band's leading end.
pub fn controls() -> Points {
    let reported = title_bar().left;
    if reported > px(0.) {
        reported
    } else if os() == Os::MacOs {
        px(78.)
    } else {
        px(0.)
    }
}

/// The horizontal inset of page headers and bodies.
pub fn inset() -> Points {
    px(if compact() { 16. } else { 24. })
}

/// A page: its header across the workspace, then `body`, which fills the
/// rest and scrolls on its own.
pub fn page(header: View, body: impl IntoElement) -> View {
    view()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .child(header)
        .child(body)
}

/// The body of a page whose content manages its own scrolling (tables,
/// shelves): inset, filling the height under the header.
pub fn page_body() -> View {
    view()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .px(inset)
        .pt_5()
        .pb_5()
        .gap_4()
}

/// The body of a form page: it scrolls, and its content keeps `measure`.
pub fn page_scroll(measure: f32, content: impl IntoElement) -> Scroll {
    scroll().flex_1().min_h_0().child(
        view()
            .px(inset)
            .pt_5()
            .pb_8()
            .child(view().w_full().max_w(px(measure)).child(content)),
    )
}

/// A page header: the page's icon, its title and a caption beside them on
/// the leading edge, `trailing` commands on the other, a hairline across
/// the workspace below. On macOS it is also the window's drag band.
pub fn page_header(
    glyph: Icon,
    title_text: impl Into<String>,
    aside: Option<AnyElement>,
    trailing: Option<AnyElement>,
) -> View {
    header_bar(
        icon(glyph, secondary).into_element(),
        title_text,
        aside,
        trailing,
    )
}
/// [`page_header`] with `lead` (an icon, a Back button) before the title.
pub fn header_bar(
    lead: AnyElement,
    title_text: impl Into<String>,
    aside: Option<AnyElement>,
    trailing: Option<AnyElement>,
) -> View {
    let mut lead = row()
        .gap(px(10.))
        .flex_1()
        .min_w(px(160.))
        .child(lead)
        .child(title(title_text).number_of_lines(1).flex_shrink(1.));
    if let Some(aside) = aside {
        lead = lead.child(view().flex_shrink_0().pt(px(4.)).child(aside));
    }
    let mut bar = wrap()
        .justify_between()
        .items_center()
        .gap_x(px(16.))
        .gap_y(px(10.))
        .px(inset)
        .pt(move || {
            if !compact() && band() > px(0.) {
                band() - px(20.)
            } else {
                px(16.)
            }
        })
        .pb(px(14.))
        .border_b_1()
        .border_color(rule)
        .z_index(5)
        .window_drag(true)
        .child(lead.min_h(px(36.)));
    if let Some(trailing) = trailing {
        bar = bar.child(row().gap_2().flex_shrink_0().child(trailing));
    }
    bar
}
/// The header of a page whose title says it all.
pub fn heading(glyph: Icon, s: impl Into<String>) -> View {
    page_header(glyph, s, None, None)
}
/// The muted count beside a page title ("3 categories").
pub fn count_caption(s: impl Fn() -> String + 'static) -> Text {
    text(s)
        .text_size(px(13.))
        .text_color(secondary)
        .number_of_lines(1)
}
/// A count with its noun, singular or plural.
pub fn counted(n: i64, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

/// A short paragraph explaining a page, above its content.
pub fn about(s: impl Into<String>) -> Text {
    note(s).max_w(px(760.)).line_height(px(20.))
}

/// A note in a quiet keyline box with an info mark: how a page's records
/// work, when the consequence is not obvious.
pub fn notice(s: impl Into<String>) -> View {
    row()
        .items_start()
        .gap_3()
        .px_4()
        .py_3()
        .rounded(px(RADIUS))
        .border_1()
        .border_color(rule)
        .child(view().pt(px(2.)).child(icon(Icon::Info, secondary)))
        .child(note(s).line_height(px(20.)).flex_1())
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
