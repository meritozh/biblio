//! Overlays: one popup surface shared by menus, popovers, select lists and
//! tooltips; menus and their items; popovers; and the native alert that
//! confirms consequential actions.
use super::*;
use nsh_base::AlertDialogResult;
use nsh_core::element::StyleRefinement;

/// The popup surface: sheet, keyline, the control corner and one shadow,
/// growing from its trigger.
pub fn popup(s: StyleRefinement) -> StyleRefinement {
    s.min_w(px(200.))
        .p(px(4.))
        .gap(px(1.))
        .rounded(px(RADIUS))
        .border_1()
        .border_color(keyline)
        .bg(sheet)
        .shadow_md()
        .transition()
        .duration(tween(SLOW))
        .ease_out()
}
fn popup_from(s: StyleRefinement) -> StyleRefinement {
    s.opacity(0.).translate_y(px(-4.))
}
fn popup_leave(s: StyleRefinement) -> StyleRefinement {
    s.opacity(0.).duration(tween(FAST)).ease_in()
}

/// A tooltip bubble saying `text`.
pub fn hint(text: impl Into<String>) -> nsh_base::TooltipElement {
    nsh_base::tooltip()
        .delay(500.)
        .px(px(8.))
        .py(px(4.))
        .rounded(px(RADIUS_SM))
        .bg(ink)
        .transition_opacity()
        .duration(tween(INSTANT))
        .starting_style(|s| s.opacity(0.))
        .child(caption(text.into()).text_color(paper))
}

/// What opens a menu or popover: a toolbar button face with a caption, an
/// optional leading icon and a trailing chevron, or an icon alone.
pub fn trigger(name: &str, glyph: Option<Icon>, show_caption: bool, chevron: bool) -> View {
    let mut face = view()
        .label(name.to_owned())
        .flex_row()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .min_h(px(32.))
        .min_w(px(32.))
        .px(px(if show_caption { 10. } else { 0. }))
        .rounded(px(RADIUS))
        .border_1()
        .border_color(if show_caption { keyline } else { clear })
        .transition()
        .duration(tween(INSTANT))
        .hover(|s| s.bg(wash))
        .active(|s| s.bg(rule).scale(press_scale(0.97)))
        .focus(|s| {
            s.outline(px(2.))
                .outline_color(accent)
                .outline_offset(px(1.))
        });
    if let Some(glyph) = glyph {
        face = face.child(icon(glyph, if show_caption { ink } else { secondary }));
    }
    if show_caption {
        face = face.child(
            label(name.to_owned())
                .text_size(px(14.))
                .font_weight(FontWeight::MEDIUM)
                .number_of_lines(1),
        );
    } else {
        face = face.tooltip(hint(name.to_owned()));
    }
    if chevron {
        face = face.child(
            icon(Icon::Disclosure, secondary)
                .w(px(12.))
                .h(px(12.))
                .rotate(Radians(std::f32::consts::FRAC_PI_2)),
        );
    }
    face
}

/// A menu opened by `opener`.
pub fn menu(opener: View) -> nsh_base::MenuElement {
    nsh_base::menu()
        .offset(px(4.))
        .popup_style(popup)
        .popup_starting_style(popup_from)
        .popup_ending_style(popup_leave)
        .trigger(opener)
}
/// A context menu about `about` (right click, or a held finger).
pub fn context_menu(about: impl IntoElement) -> nsh_base::ContextMenuElement {
    nsh_base::context_menu()
        .popup_style(popup)
        .popup_starting_style(popup_from)
        .popup_ending_style(popup_leave)
        .trigger(about)
}

fn item_frame() -> nsh_base::MenuItemElement {
    nsh_base::menu_item()
        .flex_row()
        .items_center()
        .gap_2()
        .min_h(px(28.))
        .px(px(8.))
        .rounded(px(RADIUS_SM))
        .hover(|s| s.bg(wash))
        .active(|s| s.bg(rule))
        .focus(|s| s.bg(wash))
}

/// A menu command with an optional icon in the leading slot.
pub fn menu_item(
    caption: impl Into<String>,
    glyph: Option<Icon>,
    action: impl Fn() + 'static,
) -> nsh_base::MenuItemElement {
    let caption = caption.into();
    let slot = match glyph {
        Some(glyph) => icon(glyph, secondary),
        None => view().w(px(16.)).h(px(16.)).flex_shrink_0(),
    };
    item_frame()
        .accessibility_label(caption.clone())
        .on_select(action)
        .child(slot)
        .child(label(caption).number_of_lines(1))
}
/// A destructive menu command, set apart in the danger colour.
pub fn menu_danger(
    caption: impl Into<String>,
    glyph: Icon,
    action: impl Fn() + 'static,
) -> nsh_base::MenuItemElement {
    let caption = caption.into();
    item_frame()
        .accessibility_label(caption.clone())
        .on_select(action)
        .child(icon(glyph, danger))
        .child(label(caption).text_color(danger).number_of_lines(1))
}
fn checkable(
    caption: String,
    on: impl Fn() -> bool + Copy + 'static,
    role: AccessibilityRole,
    close: bool,
    action: impl Fn() + 'static,
) -> nsh_base::MenuItemElement {
    item_frame()
        .accessibility_label(caption.clone())
        .role(role)
        .checked(move || Some(on()))
        .close_on_select(close)
        .on_select(action)
        .indicator_style(|s| s.w(px(16.)).h(px(16.)).items_center().justify_center())
        .indicator(icon(
            Icon::Check,
            move || if on() { accent() } else { clear() },
        ))
        .child(label(caption).number_of_lines(1))
}
/// One choice of a group in a menu (a sort key).
pub fn menu_radio(
    caption: impl Into<String>,
    on: impl Fn() -> bool + Copy + 'static,
    action: impl Fn() + 'static,
) -> nsh_base::MenuItemElement {
    checkable(
        caption.into(),
        on,
        AccessibilityRole::MenuItemRadio,
        true,
        action,
    )
}
/// A line between groups of menu items.
pub fn menu_separator() -> nsh_base::MenuSeparatorElement {
    nsh_base::menu_separator()
        .h(px(1.))
        .my(px(4.))
        .mx(px(4.))
        .bg(rule)
}
/// A small heading over a group of menu items.
pub fn menu_heading(text: &str) -> Text {
    caption(text.to_owned())
        .font_weight(FontWeight::SEMIBOLD)
        .px(px(8.))
        .pt(px(6.))
        .pb(px(2.))
}

/// A popover opened by `opener`, holding `content` on the popup surface.
pub fn popover(
    opener: View,
    content: impl Fn() -> AnyElement + 'static,
) -> nsh_base::PopoverElement {
    nsh_base::popover()
        .offset(px(4.))
        .align(Align::Start)
        .popup_style(|s| {
            popup(s)
                .p(px(12.))
                .gap(px(12.))
                .min_w(px(300.))
                .max_w(px(420.))
        })
        .popup_starting_style(popup_from)
        .popup_ending_style(popup_leave)
        .trigger(opener)
        .child(dynamic(content))
}

/// Asks before a consequential action with the platform's alert: `title`
/// names the object, `detail` the consequence, `verb` the action's button.
/// `destructive` marks the action in the platform's destructive style.
pub fn ask(
    title: impl Into<String>,
    detail: impl Into<String>,
    verb: impl Into<String>,
    destructive: bool,
    action: impl FnOnce() + 'static,
) {
    let dialog = nsh_base::alert_dialog()
        .title(title.into())
        .description(detail.into())
        .cancel("cancel", "Cancel");
    let dialog = if destructive {
        dialog.destructive("go", verb.into())
    } else {
        dialog.action("go", verb.into())
    };
    let answer = dialog.show();
    spawn(async move {
        if answer
            .await
            .is_ok_and(|r: AlertDialogResult| r.action.as_deref() == Some("go"))
        {
            action();
        }
    });
}

/// A button that asks first. `caption` is the command and the alert's
/// action; `title` and `detail` say what and why.
pub fn confirm(
    caption: impl Into<String>,
    title: impl Into<String>,
    detail: impl Into<String>,
    destructive: bool,
    action: impl Fn() + 'static,
) -> nsh_base::ButtonElement {
    let caption = caption.into();
    let (title, detail) = (title.into(), detail.into());
    let action = std::rc::Rc::new(action);
    let verb = caption.clone();
    control(
        caption,
        if destructive {
            Variant::Danger
        } else {
            Variant::Default
        },
        ButtonSize::Md,
        None,
        move || {
            let action = action.clone();
            ask(
                title.clone(),
                detail.clone(),
                verb.clone(),
                destructive,
                move || action(),
            )
        },
    )
}

/// A toolbar menu face on a quiet fill: the control's muted name, its
/// current value and a chevron ("Sort  Date added ⌄"). `name` is also its
/// accessible name.
pub fn tool_trigger(name: &str, glyph: Option<Icon>, value: impl Fn() -> String + 'static) -> View {
    let mut face = view()
        .label(name.to_owned())
        .flex_row()
        .items_center()
        .gap(px(6.))
        .min_h(px(32.))
        .px(px(10.))
        .rounded(px(RADIUS))
        .bg(rail)
        .transition()
        .duration(tween(INSTANT))
        .hover(|s| s.bg(wash))
        .active(|s| s.bg(rule).scale(press_scale(0.97)))
        .focus(|s| {
            s.outline(px(2.))
                .outline_color(accent)
                .outline_offset(px(1.))
        });
    if let Some(glyph) = glyph {
        face = face.child(icon(glyph, secondary));
    }
    face.child(note(name.to_owned()).text_size(px(13.)))
        .child(
            text(value)
                .text_size(px(13.))
                .text_color(ink)
                .font_weight(FontWeight::MEDIUM)
                .number_of_lines(1),
        )
        .child(
            icon(Icon::Disclosure, secondary)
                .w(px(12.))
                .h(px(12.))
                .rotate(Radians(std::f32::consts::FRAC_PI_2)),
        )
}

/// A dialog: a sheet in the middle of the window over a dim backdrop, with
/// its title and a Close button at the top and `content` scrolling under
/// them. Escape, a press on the backdrop and Close set `open` false.
/// `width` is the sheet's widest.
pub fn modal(
    open: Signal<bool>,
    heading: impl Fn() -> String + 'static,
    width: f32,
    content: impl Fn() -> AnyElement + 'static,
) -> AnyElement {
    let heading = std::rc::Rc::new(heading);
    let content = std::rc::Rc::new(content);
    when(
        move || open.get(),
        move || {
            let heading = heading.clone();
            let content = content.clone();
            nsh_base::dialog()
                .animation(if reduce_motion() {
                    ModalAnimation::None
                } else {
                    ModalAnimation::Fade
                })
                .on_cancel(move || open.set(false))
                .child(
                    view()
                        .w(move || window().width)
                        .h(move || window().height)
                        .items_center()
                        .justify_center()
                        .p_6()
                        .child(
                            view()
                                .absolute()
                                .inset_0()
                                .bg(scrim)
                                .accessible(false)
                                .on_press(move || open.set(false)),
                        )
                        .child(
                            view()
                                .label(untracked(|| heading()))
                                .w_full()
                                .max_w(px(width))
                                .max_h(move || window().height - px(48.))
                                .rounded(px(10.))
                                .border_1()
                                .border_color(keyline)
                                .bg(sheet)
                                .shadow_lg()
                                .overflow_hidden()
                                .child(
                                    row()
                                        .justify_between()
                                        .gap_3()
                                        .pl_5()
                                        .pr_3()
                                        .pt_4()
                                        .pb_3()
                                        .border_b_1()
                                        .border_color(rule)
                                        .child(
                                            subheading(untracked(|| heading()))
                                                .number_of_lines(1)
                                                .flex_1(),
                                        )
                                        .child(icon_button(
                                            "Close",
                                            Icon::Close,
                                            Some("Esc"),
                                            move || open.set(false),
                                        )),
                                )
                                .child(scroll().flex_shrink(1.).min_h_0().child(
                                    view().px_5().pt_4().pb_5().child(untracked(|| content())),
                                )),
                        ),
                )
        },
    )
    .into_element()
}
