//! `apps/desktop/src/main/windows-title-bar.tsx`: the app draws its own title
//! bar on Windows and Linux (sidebar toggle, menu bar, window controls) and
//! lets the whole strip drag the window.

use gpui::{
    App, ClickEvent, Context, Div, MouseButton, MouseDownEvent, Pixels, Point, ResizeEdge,
    SharedString, Size, Stateful, Window, div, prelude::*, px,
};

use super::Workspace;
use crate::ui::{TailwindText as _, icon, window_control};

pub fn uses_windows_style_title_bar() -> bool {
    cfg!(any(target_os = "windows", target_os = "linux"))
}

/// Which window edge a point within `threshold` of the border belongs to.
pub fn resize_edge(
    position: Point<Pixels>,
    threshold: Pixels,
    size: Size<Pixels>,
) -> Option<ResizeEdge> {
    let top = position.y < threshold;
    let bottom = position.y > size.height - threshold;
    let left = position.x < threshold;
    let right = position.x > size.width - threshold;
    Some(match (top, bottom, left, right) {
        (true, _, true, _) => ResizeEdge::TopLeft,
        (true, _, _, true) => ResizeEdge::TopRight,
        (_, true, true, _) => ResizeEdge::BottomLeft,
        (_, true, _, true) => ResizeEdge::BottomRight,
        (true, ..) => ResizeEdge::Top,
        (_, true, ..) => ResizeEdge::Bottom,
        (_, _, true, _) => ResizeEdge::Left,
        (_, _, _, true) => ResizeEdge::Right,
        _ => return None,
    })
}

impl Workspace {
    pub(super) fn render_title_bar(&self, window: &Window, cx: &Context<Self>) -> Stateful<Div> {
        let theme = self.theme;
        let maximized = window.is_maximized();

        let menu = |label: &'static str| {
            div()
                .id(SharedString::from(format!("menu-{label}")))
                .h(px(28.0))
                .px(px(10.0))
                .flex()
                .items_center()
                .rounded_md()
                .tw_text_sm()
                .text_color(theme.muted_foreground)
                .cursor_pointer()
                .hover(move |style| style.bg(theme.accent).text_color(theme.foreground))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(label)
        };

        div()
            .id("title-bar")
            .flex()
            .h(px(40.0))
            .flex_shrink_0()
            .bg(theme.background)
            // `data-tauri-drag-region`: press-and-drag anywhere that is not a
            // control moves the window; a double click toggles maximize.
            .on_mouse_down(
                MouseButton::Left,
                |_: &MouseDownEvent, window: &mut Window, _: &mut App| window.start_window_move(),
            )
            .on_click(|event: &ClickEvent, window: &mut Window, _: &mut App| {
                if event.click_count() == 2 {
                    window.zoom_window();
                }
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .pl_2()
                    .child(
                        self.tracked_chrome_button("toggle-sidebar", cx)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.toggle_sidebar(cx);
                            }))
                            .child(icon(
                                if self.sidebar_expanded {
                                    "sidebar-left"
                                } else {
                                    "view-sidebar-left"
                                },
                                px(16.0),
                                self.chrome_icon_color("toggle-sidebar"),
                            )),
                    )
                    .child(
                        div()
                            .ml_2()
                            .flex()
                            .h_full()
                            .items_center()
                            .child(menu("File"))
                            .child(menu("Edit"))
                            .child(menu("View"))
                            .child(menu("Help")),
                    )
                    .child(div().min_w_4().flex_1()),
            )
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        window_control("minimize", theme, false)
                            .on_click(|_, window, _| window.minimize_window())
                            .child(div().h(px(1.0)).w(px(10.0)).bg(theme.foreground)),
                    )
                    .child(
                        window_control("maximize", theme, false)
                            .on_click(|_, window, _| window.zoom_window())
                            .child(if maximized {
                                div()
                                    .relative()
                                    .size(px(12.0))
                                    .child(
                                        div()
                                            .absolute()
                                            .top(px(2.0))
                                            .right_0()
                                            .size(px(8.0))
                                            .border_1()
                                            .border_color(theme.foreground),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .bottom(px(2.0))
                                            .left_0()
                                            .size(px(8.0))
                                            .bg(theme.background)
                                            .border_1()
                                            .border_color(theme.foreground),
                                    )
                            } else {
                                div()
                                    .size(px(10.0))
                                    .border_1()
                                    .border_color(theme.foreground)
                            }),
                    )
                    .child(
                        window_control("close", theme, true)
                            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                                this.set_hovered("close", *hovered, cx);
                            }))
                            .on_click(|_, window, _| window.remove_window())
                            .child(icon(
                                "close-x",
                                px(12.0),
                                if self.hovered == Some("close") {
                                    theme.white
                                } else {
                                    theme.foreground
                                },
                            )),
                    ),
            )
    }
}
