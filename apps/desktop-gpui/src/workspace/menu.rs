//! `DropdownMenuContent variant="app"` menus: the `rounded-[22px] p-0.5`
//! chrome around an `AppFloatingPanel` (`rounded-[20px] p-1.5`), items with
//! `rounded-[14px] px-2 py-1.5 text-sm gap-2`, and submenus that open to the
//! right of their trigger on hover.

use gpui::{
    AnyElement, BoxShadow, ClickEvent, Context, Corner, MouseButton, MouseDownEvent, Pixels, Point,
    SharedString, anchored, deferred, div, hsla, point, prelude::*, px,
};

use super::Workspace;
use crate::theme::Theme;
use crate::ui::{TailwindText as _, icon};

pub(crate) const ITEM_HEIGHT: f32 = 32.0;
pub(crate) const SEPARATOR_HEIGHT: f32 = 9.0;
/// chrome border + `p-0.5` + panel border + `p-1.5`
pub(crate) const PANEL_INSET: f32 = 1.0 + 2.0 + 1.0 + 6.0;

pub(crate) type Select = Box<dyn Fn(&mut Workspace, &mut gpui::Window, &mut Context<Workspace>)>;

pub(crate) enum Trailing {
    None,
    /// `<span className="text-muted-foreground">` after a `flex-1` label.
    Text(SharedString),
    /// A `Check` icon when selected.
    Check(bool),
    /// `DropdownMenuSubTrigger`'s caret.
    Submenu,
}

/// What a `DropdownMenuSub` opens: another item list, or a custom panel
/// (`DropdownMenuSubContent` wrapping arbitrary content).
pub(crate) enum Submenu {
    Entries(Vec<Entry>),
    Panel {
        width: f32,
        render: fn(&Workspace, &Context<Workspace>) -> AnyElement,
    },
}

pub(crate) enum Entry {
    Item {
        /// Leading icon, drawn at `opacity-70` when `dim_icon`.
        icon: Option<&'static str>,
        dim_icon: bool,
        label: SharedString,
        trailing: Trailing,
        destructive: bool,
        on_select: Option<Select>,
        submenu: Option<Submenu>,
    },
    Separator,
}

impl Entry {
    pub fn height(&self) -> f32 {
        match self {
            Entry::Item { .. } => ITEM_HEIGHT,
            Entry::Separator => SEPARATOR_HEIGHT,
        }
    }
}

pub(crate) struct MenuSpec {
    pub id: &'static str,
    pub width: f32,
    pub entries: Vec<Entry>,
    /// The entry whose submenu is showing.
    pub open_sub: Option<usize>,
    /// Called with an entry index when the pointer enters a submenu trigger
    /// (or `None` over a plain item).
    pub on_hover_sub: fn(&mut Workspace, Option<usize>, &mut Context<Workspace>),
    pub on_close: fn(&mut Workspace, &mut Context<Workspace>),
}

/// Where the menu attaches: `align="start"` hangs the top-left corner off
/// the point, `align="end"` the top-right.
pub(crate) enum Align {
    Start,
    End,
}

impl Workspace {
    /// Renders a menu (and its open submenu) as a deferred overlay.
    pub(crate) fn render_app_menu(
        &self,
        spec: MenuSpec,
        position: Point<Pixels>,
        align: Align,
        window: &gpui::Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let viewport_width = window.viewport_size().width;
        let width = spec.width;
        let open_sub = spec.open_sub;
        let on_hover_sub = spec.on_hover_sub;
        let on_close = spec.on_close;
        let mut submenu_overlay: Option<AnyElement> = None;
        // The sub panel is a separate deferred element, so the parent's
        // `on_mouse_down_out` has to know its bounds to keep clicks inside a
        // custom panel (the participant field) from closing the menu.
        let sub_bounds: std::rc::Rc<std::cell::Cell<Option<gpui::Bounds<Pixels>>>> =
            std::rc::Rc::new(std::cell::Cell::new(None));
        let mut y = PANEL_INSET;
        let panel_left = match align {
            Align::Start => position.x,
            Align::End => position.x - px(width),
        };

        let items = spec
            .entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| {
                let top = y;
                y += entry.height();
                match entry {
                    Entry::Separator => div()
                        .mx(px(-4.0))
                        .my_1()
                        .h(px(1.0))
                        .bg(theme.accent)
                        .into_any_element(),
                    Entry::Item {
                        icon: glyph,
                        dim_icon,
                        label,
                        trailing,
                        destructive,
                        on_select,
                        submenu,
                    } => {
                        let is_sub = submenu.is_some();
                        if let (Some(submenu), true) = (submenu, open_sub == Some(index)) {
                            // `DropdownMenuSubContent`: measured against the
                            // app, the sub chrome starts at the parent's right
                            // edge, 2px above the trigger row; Radix flips it
                            // to the left edge when the right side lacks room.
                            let sub_width = match &submenu {
                                Submenu::Entries(_) => 176.0,
                                Submenu::Panel { width, .. } => *width,
                            };
                            // Flipped, the sub's right edge sits 9px inside the
                            // parent chrome (measured against the app).
                            let sub_x = if panel_left + px(width + sub_width + 8.0) > viewport_width
                            {
                                panel_left - px(sub_width - 9.0)
                            } else {
                                panel_left + px(width)
                            };
                            let sub_position = point(sub_x, position.y + px(top) - px(2.0));
                            submenu_overlay = Some(match submenu {
                                Submenu::Entries(entries) => self.render_menu_panel(
                                    spec.id,
                                    entries,
                                    sub_width,
                                    sub_position,
                                    on_close,
                                    cx,
                                ),
                                Submenu::Panel { render, .. } => self.render_sub_panel(
                                    sub_width,
                                    sub_position,
                                    render(self, cx),
                                    sub_bounds.clone(),
                                ),
                            });
                        }
                        let color = if destructive {
                            theme.delete_text
                        } else {
                            theme.foreground
                        };
                        let on_select = on_select.map(std::rc::Rc::new);
                        div()
                            .id((spec.id, index))
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_2()
                            .py(px(6.0))
                            .rounded(px(14.0))
                            .tw_text_sm()
                            .text_color(color)
                            .cursor_pointer()
                            .when(open_sub == Some(index), |item| item.bg(theme.accent))
                            .hover(move |style| {
                                if destructive {
                                    style
                                        .bg(theme.delete_hover_background)
                                        .text_color(theme.delete_hover_text)
                                } else {
                                    style.bg(theme.accent)
                                }
                            })
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                                if *hovered {
                                    on_hover_sub(this, is_sub.then_some(index), cx);
                                }
                            }))
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                if is_sub {
                                    return;
                                }
                                on_close(this, cx);
                                if let Some(on_select) = &on_select {
                                    on_select(this, window, cx);
                                }
                            }))
                            .when_some(glyph, |item, glyph| {
                                let tint = if dim_icon {
                                    crate::theme::alpha(color, 0.7)
                                } else {
                                    color
                                };
                                item.child(icon(glyph, px(16.0), tint))
                            })
                            .child(div().flex_1().child(label))
                            .child(match trailing {
                                Trailing::None | Trailing::Submenu => div().into_any_element(),
                                Trailing::Text(text) => div()
                                    .text_color(theme.muted_foreground)
                                    .child(text)
                                    .into_any_element(),
                                Trailing::Check(checked) => {
                                    if checked {
                                        icon("check", px(16.0), color).into_any_element()
                                    } else {
                                        div().into_any_element()
                                    }
                                }
                            })
                            // `DropdownMenuSubTrigger` always ends with the caret.
                            .when(is_sub, |item| {
                                item.child(icon("caret-right", px(16.0), color))
                            })
                            .into_any_element()
                    }
                }
            })
            .collect::<Vec<_>>();

        let panel = menu_chrome(theme, spec.id, width)
            .on_mouse_down_out(cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                if sub_bounds
                    .get()
                    .is_some_and(|bounds| bounds.contains(&event.position))
                {
                    return;
                }
                on_close(this, cx)
            }))
            .child(
                // `AppFloatingPanel`: `rounded-[20px] border` under the panel squircle.
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .p(px(6.0))
                    .child(crate::squircle::squircle(
                        crate::squircle::PANEL_RADIUS,
                        Some(theme.floating_panel),
                        Some((1.0, theme.floating_border)),
                    ))
                    .children(items),
            );

        let corner = match align {
            Align::Start => Corner::TopLeft,
            Align::End => Corner::TopRight,
        };
        div()
            .child(
                deferred(
                    anchored()
                        .anchor(corner)
                        .position(position)
                        .snap_to_window_with_margin(px(8.0))
                        .child(panel),
                )
                .with_priority(1),
            )
            .children(submenu_overlay)
            .into_any_element()
    }

    /// A standalone menu anchored at the element's own layout position (for
    /// dropdown triggers inside scrolling content): the app chrome, items with
    /// hover and click, `on_mouse_down_out` closing it.
    /// Radix menus dismiss on Escape; the same happens when a shortcut swaps
    /// the view under an open menu. Returns whether anything was open.
    pub(crate) fn close_open_menus(&mut self, cx: &mut Context<Self>) -> bool {
        let mut closed = false;
        if self.filter_menu_open {
            self.filter_menu_open = false;
            self.filter_submenu = None;
            closed = true;
        }
        if self.overflow_open {
            self.overflow_open = false;
            self.overflow_submenu = None;
            closed = true;
        }
        if self.open_menu.take().is_some() {
            closed = true;
        }
        if let Some(player) = self.audio_player.as_mut()
            && player.menu_at.take().is_some()
        {
            closed = true;
        }
        closed |= self.close_calendar_context_menu();
        closed |= self.close_contacts_menus();
        closed |= self.close_templates_menus();
        closed |= self.close_automations_menus();
        if closed {
            cx.notify();
        }
        closed
    }

    pub(crate) fn render_menu_inline(
        &self,
        spec: MenuSpec,
        align: Align,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let on_close = spec.on_close;
        let items = self.menu_item_elements(spec.id, spec.entries, on_close, cx);
        let panel = menu_chrome(theme, spec.id, spec.width)
            .on_mouse_down_out(
                cx.listener(move |this, _: &MouseDownEvent, _, cx| on_close(this, cx)),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .p(px(6.0))
                    .child(crate::squircle::squircle(
                        crate::squircle::PANEL_RADIUS,
                        Some(theme.floating_panel),
                        Some((1.0, theme.floating_border)),
                    ))
                    .children(items),
            );
        deferred(
            anchored()
                .anchor(match align {
                    Align::Start => Corner::TopLeft,
                    Align::End => Corner::TopRight,
                })
                .snap_to_window_with_margin(px(8.0))
                .child(panel),
        )
        .with_priority(2)
        .into_any_element()
    }

    /// The `rounded-[14px] px-2 py-1.5 text-sm gap-2` rows of a plain panel.
    fn menu_item_elements(
        &self,
        id: &'static str,
        entries: Vec<Entry>,
        on_close: fn(&mut Workspace, &mut Context<Workspace>),
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        let theme = self.theme;
        entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| match entry {
                Entry::Separator => div()
                    .mx(px(-4.0))
                    .my_1()
                    .h(px(1.0))
                    .bg(theme.accent)
                    .into_any_element(),
                Entry::Item {
                    icon: glyph,
                    dim_icon,
                    label,
                    trailing,
                    destructive,
                    on_select,
                    ..
                } => {
                    let color = if destructive {
                        theme.delete_text
                    } else {
                        theme.foreground
                    };
                    let disabled = on_select.is_none();
                    let on_select = on_select.map(std::rc::Rc::new);
                    div()
                        .id(SharedString::from(format!("{id}-inline-{index}")))
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py(px(6.0))
                        .rounded(px(14.0))
                        .tw_text_sm()
                        .text_color(color)
                        // `data-[disabled]:opacity-50`
                        .when(disabled, |item| item.opacity(0.5))
                        .when(!disabled, |item| {
                            item.cursor_pointer()
                                .hover(move |style| style.bg(theme.accent))
                        })
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            if let Some(on_select) = &on_select {
                                on_close(this, cx);
                                on_select(this, window, cx);
                            }
                        }))
                        .when_some(glyph, |item, glyph| {
                            let tint = if dim_icon {
                                crate::theme::alpha(color, 0.7)
                            } else {
                                color
                            };
                            item.child(icon(glyph, px(16.0), tint))
                        })
                        .child(div().flex_1().child(label))
                        .child(match trailing {
                            Trailing::Check(true) => {
                                icon("check", px(14.0), color).into_any_element()
                            }
                            Trailing::Text(text) => div()
                                .text_color(theme.muted_foreground)
                                .child(text)
                                .into_any_element(),
                            _ => div().into_any_element(),
                        })
                        .into_any_element()
                }
            })
            .collect()
    }

    /// A submenu panel: items only, no hover switching of its own.
    fn render_menu_panel(
        &self,
        id: &'static str,
        entries: Vec<Entry>,
        width: f32,
        position: Point<Pixels>,
        on_close: fn(&mut Workspace, &mut Context<Workspace>),
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let items = entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| match entry {
                Entry::Separator => div()
                    .mx(px(-4.0))
                    .my_1()
                    .h(px(1.0))
                    .bg(theme.accent)
                    .into_any_element(),
                Entry::Item {
                    icon: glyph,
                    dim_icon,
                    label,
                    trailing,
                    destructive,
                    on_select,
                    ..
                } => {
                    let color = if destructive {
                        theme.delete_text
                    } else {
                        theme.foreground
                    };
                    let on_select = on_select.map(std::rc::Rc::new);
                    div()
                        .id(SharedString::from(format!("{id}-sub-{index}")))
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py(px(6.0))
                        .rounded(px(14.0))
                        .tw_text_sm()
                        .text_color(color)
                        .cursor_pointer()
                        .hover(move |style| style.bg(theme.accent))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            on_close(this, cx);
                            if let Some(on_select) = &on_select {
                                on_select(this, window, cx);
                            }
                        }))
                        .when_some(glyph, |item, glyph| {
                            let tint = if dim_icon {
                                crate::theme::alpha(color, 0.7)
                            } else {
                                color
                            };
                            item.child(icon(glyph, px(16.0), tint))
                        })
                        .child(div().flex_1().child(label))
                        .child(match trailing {
                            Trailing::Check(true) => {
                                icon("check", px(16.0), color).into_any_element()
                            }
                            Trailing::Text(text) => div()
                                .text_color(theme.muted_foreground)
                                .child(text)
                                .into_any_element(),
                            _ => div().into_any_element(),
                        })
                        .into_any_element()
                }
            });
        let panel = menu_chrome(theme, "submenu", width).child(
            div()
                .relative()
                .flex()
                .flex_col()
                .p(px(6.0))
                .child(crate::squircle::squircle(
                    crate::squircle::PANEL_RADIUS,
                    Some(theme.floating_panel),
                    Some((1.0, theme.floating_border)),
                ))
                .children(items),
        );
        deferred(
            anchored()
                .anchor(Corner::TopLeft)
                .position(position)
                .snap_to_window_with_margin(px(8.0))
                .child(panel),
        )
        .with_priority(2)
        .into_any_element()
    }
}

impl Workspace {
    /// `DropdownMenuSubContent variant="app"` around an `AppFloatingPanel`
    /// holding custom content.
    fn render_sub_panel(
        &self,
        width: f32,
        position: Point<Pixels>,
        content: AnyElement,
        bounds_out: std::rc::Rc<std::cell::Cell<Option<gpui::Bounds<Pixels>>>>,
    ) -> AnyElement {
        let theme = self.theme;
        let panel = menu_chrome(theme, "submenu", width)
            .child(
                gpui::canvas(
                    move |bounds, _, _| bounds_out.set(Some(bounds)),
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0()
                .size_full(),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .child(crate::squircle::squircle(
                        crate::squircle::PANEL_RADIUS,
                        Some(theme.floating_panel),
                        Some((1.0, theme.floating_border)),
                    ))
                    .child(content),
            );
        deferred(
            anchored()
                .anchor(Corner::TopLeft)
                .position(position)
                .snap_to_window_with_margin(px(8.0))
                .child(panel),
        )
        .with_priority(2)
        .into_any_element()
    }
}

/// `appFloatingContentClassName` with `shadow-lg`.
pub(super) fn menu_chrome(theme: Theme, id: &'static str, width: f32) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(format!("{id}-chrome")))
        .occlude()
        .w(px(width))
        .p(px(2.0))
        .rounded(px(22.0))
        .border_1()
        .border_color(theme.floating_border)
        .bg(theme.floating_chrome)
        .shadow(vec![
            BoxShadow {
                color: hsla(0.0, 0.0, 0.0, 0.1),
                offset: point(px(0.0), px(10.0)),
                blur_radius: px(15.0),
                spread_radius: px(-3.0),
            },
            BoxShadow {
                color: hsla(0.0, 0.0, 0.0, 0.1),
                offset: point(px(0.0), px(4.0)),
                blur_radius: px(6.0),
                spread_radius: px(-4.0),
            },
        ])
}
