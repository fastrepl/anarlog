//! Native counterparts of the shipping settings building blocks
//! (`settings/page-title.tsx`, `settings/setting-row.tsx`, and the `@anlg/ui` switch, button,
//! and select). Sizes follow the Tailwind classes those components use.

use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, ElementId, FocusHandle, FontWeight, KeyDownEvent,
    MouseDownEvent, Pixels, SharedString, Stateful, Window, canvas, deferred, div, point,
    prelude::*, px, svg,
};

use crate::ui::a11y::Accessible;
use crate::ui::theme::Theme;

/// Shipping's `--font-hand` stack.
pub const HAND_FONT: &str = if cfg!(target_os = "macos") {
    "Bradley Hand"
} else if cfg!(target_os = "windows") {
    "Segoe Print"
} else {
    "Comic Sans MS"
};

/// Width of the fixed control column in a setting row (`w-48`).
pub const CONTROL_WIDTH: f32 = 192.;

type Handler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

pub fn page_title(title: impl Into<SharedString>) -> Stateful<Div> {
    let title = title.into();
    div()
        .id(SharedString::from(format!("page-title:{title}")))
        .a11y_heading(title.clone(), 1)
        .font_family(HAND_FONT)
        .text_size(px(30.))
        .line_height(px(37.5))
        .font_weight(FontWeight::SEMIBOLD)
        .child(title)
}

pub fn section_title(title: impl Into<SharedString>) -> Stateful<Div> {
    let title = title.into();
    div()
        .id(SharedString::from(format!("section-title:{title}")))
        .a11y_heading(title.clone(), 2)
        .text_size(px(18.))
        .line_height(px(28.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(title)
}

pub fn description(text: impl Into<SharedString>, colors: &Theme) -> Stateful<Div> {
    let text = text.into();
    div()
        .id(SharedString::from(format!("description:{text}")))
        .a11y_text(text.clone())
        .text_xs()
        .line_height(px(16.))
        .text_color(colors.muted_foreground)
        .child(text)
}

/// `SettingRow`: a title and description on the left, the control on the right.
/// `fixed` keeps the control column at `w-48`; otherwise it shrinks to the control. A screen
/// reader meets it as a group named by the title, so its control has that context.
pub fn setting_row(
    title: impl Into<SharedString>,
    detail: Option<SharedString>,
    fixed: bool,
    control: impl IntoElement,
    colors: &Theme,
) -> Stateful<Div> {
    let title = title.into();
    div()
        .id(SharedString::from(format!("setting:{title}")))
        .a11y_group(gpui::Role::Group, title.clone())
        .when_some(detail.clone(), |view, detail| view.aria_description(detail))
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .when(detail.is_some(), |view| view.mb_1())
                        .text_sm()
                        .line_height(px(20.))
                        .font_weight(crate::ui::theme::MEDIUM)
                        .child(title),
                )
                .when_some(detail, |view, detail| {
                    view.child(
                        div()
                            .text_xs()
                            .line_height(px(16.))
                            .text_color(colors.muted_foreground)
                            .child(detail),
                    )
                }),
        )
        .child(
            div()
                .flex()
                .justify_end()
                .when(fixed, |view| view.w(px(CONTROL_WIDTH)).min_w_0())
                .when(!fixed, |view| view.flex_shrink_0())
                .child(control),
        )
}

/// The indented group shipping uses for dependent settings (`ml-3 border-l-2 pl-4`).
pub fn nested(colors: &Theme) -> Div {
    div()
        .ml_3()
        .pl_4()
        .border_l_2()
        .border_color(colors.muted)
        .flex()
        .flex_col()
        .gap_4()
}

pub fn switch(
    id: impl Into<ElementId>,
    checked: bool,
    enabled: bool,
    colors: &Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    // Callers name the switch with its row's title.
    div()
        .id(id)
        .role(gpui::Role::Switch)
        .aria_toggled(crate::ui::a11y::toggled(checked))
        .a11y_disabled(!enabled)
        .w(px(44.))
        .h(px(24.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .border_2()
        .border_color(if checked {
            colors.primary
        } else {
            colors.border
        })
        .bg(if checked {
            colors.primary
        } else {
            colors.muted
        })
        .when(checked, |view| view.pl(px(20.)))
        .child(div().size(px(20.)).shadow_lg().bg(if checked {
            colors.primary_foreground
        } else {
            colors.background
        }))
        .when(!enabled, |view| view.opacity(0.5))
        .when(enabled, |view| view.cursor_pointer().on_click(on_click))
}

pub fn outline_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    enabled: bool,
    colors: &Theme,
    on_click: Option<Handler>,
) -> Stateful<Div> {
    let label = label.into();
    div()
        .id(id)
        .a11y_button(label.clone())
        .a11y_disabled(!enabled)
        .h(px(36.))
        .px_4()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .gap_2()
        .border_1()
        .border_color(colors.input)
        .bg(colors.background)
        .shadow_xs()
        .text_sm()
        .font_weight(crate::ui::theme::MEDIUM)
        .child(label)
        .when(!enabled, |view| view.opacity(0.5))
        .when_some(on_click.filter(|_| enabled), |view, on_click| {
            view.cursor_pointer()
                .hover(|style| style.bg(colors.accent))
                .on_click(on_click)
        })
}

pub fn icon(path: &'static str, size: f32, color: gpui::Hsla) -> gpui::Svg {
    svg()
        .path(path)
        .size(px(size))
        .flex_shrink_0()
        .text_color(color)
}

/// The select trigger (`SelectTrigger` with `bg-card h-9`): the current label, or a muted
/// placeholder, with the caret.
pub fn select_trigger(
    id: impl Into<ElementId>,
    label: Option<SharedString>,
    placeholder: &'static str,
    enabled: bool,
    colors: &Theme,
) -> Stateful<Div> {
    let empty = label.is_none();
    // Radix's select trigger is a combobox showing its value; callers name it and set
    // `aria_expanded`.
    div()
        .id(id)
        .role(gpui::Role::ComboBox)
        .aria_value(label.clone().unwrap_or_else(|| placeholder.into()))
        .a11y_disabled(!enabled)
        .w_full()
        .h(px(36.))
        .pl_3()
        .pr_2()
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .border_1()
        .border_color(colors.input)
        .bg(colors.card)
        .text_sm()
        .child(
            div()
                .min_w_0()
                .truncate()
                .when(empty, |view| view.text_color(colors.muted_foreground))
                .child(label.unwrap_or_else(|| placeholder.into())),
        )
        .child(icon(
            "ArrowDown01Icon.svg",
            16.,
            colors.foreground.opacity(0.5),
        ))
        .when(!enabled, |view| view.opacity(0.5))
        .when(enabled, |view| view.cursor_pointer())
}

/// What a dropdown keeps between frames: its trigger's width, measured while drawn, and the
/// focus it takes while open.
struct DropdownState {
    trigger: Pixels,
    focus: FocusHandle,
    /// The focus from before the menu opened, given back when it closes; `Some` while open.
    restore: Option<Option<FocusHandle>>,
}

/// Floats `menu` under the element it is attached to, which must be a `relative` wrapper
/// around the trigger, above all other content and at least as wide as the trigger
/// (`min-w-(--radix-select-trigger-width)`). As with Radix menus, a click outside closes it
/// without reaching the page, and it takes focus while open so Escape closes it too; focus
/// goes back when it closes. Render it with `None` while closed so the trigger stays measured.
pub fn dropdown<T: 'static>(
    id: impl Into<ElementId>,
    menu: Option<impl IntoElement>,
    align_end: bool,
    close: impl Fn(&mut T) + 'static,
    window: &mut Window,
    cx: &mut Context<T>,
) -> AnyElement {
    dropdown_focusing(id, menu, align_end, None, close, window, cx)
}

/// [`dropdown`] that focuses `initial`, such as the menu's search field, when it opens. Pass it
/// here instead of focusing it from the trigger's click handler, so the focus to give back on
/// close is read before it moves.
pub fn dropdown_focusing<T: 'static>(
    id: impl Into<ElementId>,
    menu: Option<impl IntoElement>,
    align_end: bool,
    initial: Option<FocusHandle>,
    close: impl Fn(&mut T) + 'static,
    window: &mut Window,
    cx: &mut Context<T>,
) -> AnyElement {
    let state = window.use_keyed_state(id, cx, |_, cx| DropdownState {
        trigger: px(0.),
        focus: cx.focus_handle(),
        restore: None,
    });
    let open = menu.is_some();
    let previous = window.focused(cx);
    let (focus, trigger, opening, closing) = state.update(cx, |state, _| {
        let opening = open && state.restore.is_none();
        if opening {
            state.restore = Some(previous);
        }
        let closing = if open { None } else { state.restore.take() };
        (state.focus.clone(), state.trigger, opening, closing)
    });
    if opening {
        // Once this frame is drawn, unless focus already landed inside the menu.
        let target = initial.unwrap_or_else(|| focus.clone());
        let focus = focus.clone();
        window.defer(cx, move |window, cx| {
            if !focus.contains_focused(window, cx) {
                target.focus(window, cx);
            }
        });
    }
    if let Some(previous) = closing.filter(|_| focus.contains_focused(window, cx)) {
        window.defer(cx, move |window, cx| match previous {
            Some(previous) => previous.focus(window, cx),
            None => window.blur(cx),
        });
    }
    let measure = canvas(
        move |bounds, _, cx| {
            state.update(cx, |state, cx| {
                if state.trigger != bounds.size.width {
                    state.trigger = bounds.size.width;
                    if open {
                        cx.notify();
                    }
                }
            })
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full();
    let close = Rc::new(close);
    let menu = menu.map(|menu| {
        let backdrop = {
            let close = close.clone();
            backdrop(move |this| close(this), window, cx)
        };
        let on_key = cx.listener(move |this, event: &KeyDownEvent, _, cx| {
            if event.keystroke.key == "escape" {
                close(this);
                cx.stop_propagation();
                cx.notify();
            }
        });
        div()
            .absolute()
            .top_full()
            .when(!align_end, |view| view.left_0())
            .when(align_end, |view| view.right_0())
            .child(backdrop)
            .child(
                deferred(
                    gpui::anchored()
                        .anchor(if align_end {
                            gpui::Anchor::TopRight
                        } else {
                            gpui::Anchor::TopLeft
                        })
                        .snap_to_window_with_margin(px(8.))
                        .child(
                            div()
                                .id("dropdown")
                                .track_focus(&focus)
                                .min_w(trigger)
                                .on_key_down(on_key)
                                .child(menu),
                        ),
                )
                .with_priority(1),
            )
    });
    div()
        .absolute()
        .inset_0()
        .child(measure)
        .children(menu)
        .into_any_element()
}

/// A transparent layer over the window, under a menu drawn after it, that calls `close` on a
/// press anywhere outside the menu and keeps the press from reaching the page, as Radix's
/// modal menus do.
pub fn backdrop<T: 'static>(
    close: impl Fn(&mut T) + 'static,
    window: &Window,
    cx: &mut Context<T>,
) -> gpui::Deferred {
    let viewport = window.viewport_size();
    deferred(
        gpui::anchored().position(point(px(0.), px(0.))).child(
            div()
                .id("menu-backdrop")
                .w(viewport.width)
                .h(viewport.height)
                .occlude()
                .on_any_mouse_down(cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                    close(this);
                    cx.stop_propagation();
                    cx.notify();
                })),
        ),
    )
    .with_priority(1)
}

/// `SelectContent`: as wide as its widest item, and at least as wide as the trigger once
/// [`dropdown`] places it.
pub fn menu_panel(colors: &Theme) -> Stateful<Div> {
    div()
        .id("menu-panel")
        .role(gpui::Role::ListBox)
        .mt_1()
        .p_1()
        .flex()
        .flex_col()
        .border_1()
        .border_color(colors.border)
        .bg(colors.popover)
        .text_color(colors.popover_foreground)
        .shadow_md()
        .occlude()
}

/// `SelectItem`: `pl-2 pr-8`, with the selected row's check drawn inside the right padding
/// (`absolute right-2`) so it never moves the row's content.
pub fn select_item(
    id: impl Into<ElementId>,
    label: SharedString,
    selected: bool,
    colors: &Theme,
) -> Stateful<Div> {
    let hover = colors.accent;
    div()
        .id(id)
        .a11y_option(label, selected)
        .relative()
        .w_full()
        .py(px(6.))
        .pl_2()
        .pr_8()
        .flex()
        .items_center()
        .gap_2()
        .text_sm()
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .when(selected, |view| {
            view.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_2()
                    .flex()
                    .items_center()
                    .child(icon("product/CheckIcon.svg", 16., colors.foreground)),
            )
        })
}

/// A [`select_item`] showing `label`, with `detail` at its right edge.
pub fn menu_item(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    detail: Option<SharedString>,
    selected: bool,
    colors: &Theme,
) -> Stateful<Div> {
    let label = label.into();
    select_item(id, option_name(&label, &detail), selected, colors)
        .child(div().flex_1().min_w_0().truncate().child(label))
        .when_some(detail, |view, detail| {
            view.child(
                div()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(detail),
            )
        })
}

/// A searchable select's `CommandItem` (`settings/general/searchable-select.tsx`): `px-2`,
/// with the check after the detail on the selected row only, so no row reserves its space.
pub fn command_item(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    detail: Option<SharedString>,
    selected: bool,
    colors: &Theme,
) -> Stateful<Div> {
    let hover = colors.accent;
    let label = label.into();
    div()
        .id(id)
        .a11y_option(option_name(&label, &detail), selected)
        .w_full()
        .py(px(6.))
        .px_2()
        .flex()
        .items_center()
        .gap_2()
        .text_sm()
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(div().flex_1().min_w_0().truncate().child(label))
        .when_some(detail, |view, detail| {
            view.child(
                div()
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(detail),
            )
        })
        .when(selected, |view| {
            view.child(icon("product/CheckIcon.svg", 16., colors.foreground))
        })
}

fn option_name(label: &SharedString, detail: &Option<SharedString>) -> SharedString {
    match detail {
        Some(detail) => format!("{label}, {detail}").into(),
        None => label.clone(),
    }
}

#[cfg(test)]
mod tests {
    use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext};

    use super::*;

    struct Picker {
        open: bool,
        /// A field outside the menu that has focus before it opens.
        field: FocusHandle,
        /// The menu's search field, which takes focus when it opens.
        search: FocusHandle,
    }

    impl Render for Picker {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let colors = crate::ui::theme::theme(window);
            div()
                .size_full()
                .child(
                    div()
                        .relative()
                        .w(px(160.))
                        .h(px(36.))
                        .child(div().id("trigger").size_full().on_click(cx.listener(
                            |this, _, _, cx| {
                                this.open = !this.open;
                                cx.notify();
                            },
                        )))
                        .child(dropdown_focusing(
                            "menu",
                            self.open.then(|| {
                                menu_panel(&colors)
                                    .child(div().h(px(20.)).track_focus(&self.search))
                                    .child(menu_item("item", "Item", None, false, &colors))
                            }),
                            false,
                            Some(self.search.clone()),
                            |this: &mut Self| this.open = false,
                            window,
                            cx,
                        )),
                )
                .child(div().h(px(20.)).track_focus(&self.field))
        }
    }

    #[gpui::test]
    fn an_open_dropdown_closes_on_escape_and_on_a_press_outside(cx: &mut TestAppContext) {
        let (picker, cx) = cx.add_window_view(|_, cx| Picker {
            open: false,
            field: cx.focus_handle(),
            search: cx.focus_handle(),
        });
        let open = |cx: &mut VisualTestContext| {
            // Draw, so the open menu takes focus once the frame is done.
            cx.update(|window, cx| window.draw(cx).clear(cx));
            cx.run_until_parked();
            picker.read_with(cx, |picker, _| picker.open)
        };
        let focused = |handle: fn(&Picker) -> &FocusHandle, cx: &mut VisualTestContext| {
            let handle = picker.read_with(cx, |picker, _| handle(picker).clone());
            cx.update(|window, _| handle.is_focused(window))
        };
        let field = picker.read_with(cx, |picker, _| picker.field.clone());
        cx.update(|window, cx| field.focus(window, cx));
        cx.simulate_click(point(px(20.), px(18.)), Modifiers::none());
        assert!(open(cx));
        assert!(
            focused(|picker| &picker.search, cx),
            "the search field takes focus"
        );
        cx.simulate_keystrokes("escape");
        assert!(!open(cx), "Escape closes the menu");
        assert!(
            focused(|picker| &picker.field, cx),
            "focus goes back to the field that had it"
        );
        cx.simulate_click(point(px(20.), px(18.)), Modifiers::none());
        assert!(open(cx));
        cx.simulate_mouse_down(
            point(px(400.), px(300.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        assert!(!open(cx), "a press outside closes the menu");
    }
}
