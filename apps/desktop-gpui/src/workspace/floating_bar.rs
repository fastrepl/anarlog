//! The floating recording bar: `plugins/windows/src/window/floating_bar.rs`
//! (the transparent always-on-top window at the work area's top right) and
//! `meeting-float/overlay/bar.tsx` (the compact pill with the stop control).
//! `FloatingMeetingWindowHost` shows it whenever `floating_bar_enabled` holds
//! and a live session is active, and hides it when the session ends.

use gpui::{AnyElement, App, Context, Render, WeakEntity, Window, div, prelude::*, px, rgba};

use super::Workspace;
use crate::ui::{TailwindText, icon};

/// `layout` constants.
pub const INSET: f32 = 4.0;
pub const SCREEN_MARGIN: f32 = 8.0;
pub const COMPACT_HEIGHT: f32 = 38.0;
pub const COMPACT_STOP_WIDTH: f32 = 62.0;
pub const COMPACT_SOLO_STOP_WIDTH: f32 = 68.0;
pub const COMPACT_ICON_SIZE: f32 = 30.0;
pub const COMPACT_GAP: f32 = 3.0;
pub const COMPACT_HORIZONTAL_PADDING: f32 = 4.0;
pub const HOVER_HANDLE_HEIGHT: f32 = 12.0;
pub const HOVER_HANDLE_TOP_PADDING: f32 = 7.0;
pub const HOVER_HANDLE_GAP: f32 = 2.0;
pub const HOVER_HANDLE_RESERVED_HEIGHT: f32 =
    HOVER_HANDLE_TOP_PADDING + HOVER_HANDLE_HEIGHT + HOVER_HANDLE_GAP;
pub const CONTROL_RADIUS: f32 = 10.0;
pub const COMPACT_RADIUS: f32 = 14.0;

/// `compactControlsWidth`
pub fn compact_controls_width(shows_expand: bool) -> f32 {
    if shows_expand {
        COMPACT_STOP_WIDTH + COMPACT_GAP + COMPACT_ICON_SIZE
    } else {
        COMPACT_SOLO_STOP_WIDTH
    }
}

/// `compactWidth`
pub fn compact_width(shows_expand: bool) -> f32 {
    compact_controls_width(shows_expand) + COMPACT_HORIZONTAL_PADDING * 2.0
}

/// `container_size(false, shows_expand)`: the window around the compact pill.
pub fn compact_container_size(shows_expand: bool) -> (f32, f32) {
    (
        compact_width(shows_expand) + INSET * 2.0,
        COMPACT_HEIGHT + HOVER_HANDLE_RESERVED_HEIGHT + INSET * 2.0,
    )
}

/// `top_right_origin`: `SCREEN_MARGIN` inside the work area's top-right corner.
pub fn top_right_origin(
    work_x: f32,
    work_y: f32,
    work_width: f32,
    window_width: f32,
) -> (f32, f32) {
    (
        work_x + work_width - window_width - SCREEN_MARGIN,
        work_y + SCREEN_MARGIN,
    )
}

/// `FloatingBarState` (the fields the compact pill renders).
#[derive(Clone, Debug, PartialEq)]
pub struct FloatingBarState {
    pub amplitude: f32,
    pub title: String,
    pub error: bool,
    pub dark: bool,
    pub opacity: f32,
    pub live_caption_toggle_visible: bool,
}

/// `barColors(state)`
struct BarColors {
    surface: gpui::Rgba,
    envelope_surface: gpui::Rgba,
    content: gpui::Rgba,
    handle: gpui::Rgba,
    outer_stroke: gpui::Rgba,
    control_fill: gpui::Rgba,
    accent: gpui::Rgba,
}

fn with_alpha(rgb: u32, alpha: f32) -> gpui::Rgba {
    rgba((rgb << 8) | ((alpha.clamp(0.0, 1.0) * 255.0).round() as u32))
}

fn bar_colors(state: &FloatingBarState) -> BarColors {
    let opacity = state.opacity.clamp(0.35, 0.95);
    let surface_rgb = if state.dark { 0x6e7066 } else { 0xdbd9d1 };
    let ink = if state.dark { 0xffffff } else { 0x1f1c1a };
    BarColors {
        surface: with_alpha(surface_rgb, opacity * 0.82),
        envelope_surface: with_alpha(surface_rgb, (opacity * 1.08).min(0.95)),
        content: with_alpha(ink, 1.0),
        handle: with_alpha(ink, if state.dark { 0.48 } else { 0.36 }),
        outer_stroke: with_alpha(ink, if state.dark { 0.14 } else { 0.12 }),
        control_fill: with_alpha(ink, if state.dark { 0.08 } else { 0.07 }),
        accent: if state.error {
            gpui::rgb(0xff403d)
        } else {
            gpui::rgb(0xff334d)
        },
    }
}

/// The floating window's root view.
pub struct FloatingBar {
    pub state: FloatingBarState,
    workspace: WeakEntity<Workspace>,
    hovered: bool,
    stop_hovered: bool,
}

impl FloatingBar {
    pub fn new(state: FloatingBarState, workspace: WeakEntity<Workspace>) -> Self {
        Self {
            state,
            workspace,
            hovered: false,
            stop_hovered: false,
        }
    }

    /// `HoverHandle`: a `5px × 7px` dot grid, 16px narrower than the pill.
    fn render_hover_handle(&self, colors: &BarColors, width: f32) -> AnyElement {
        let dots_width = (width - 16.0).max(0.0);
        let columns = (dots_width / 5.0).floor() as usize;
        let rows = (HOVER_HANDLE_HEIGHT / 7.0).floor() as usize;
        let handle = colors.handle;
        div()
            .flex()
            .h(px(HOVER_HANDLE_HEIGHT))
            .w(px(width))
            .items_center()
            .justify_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .h_full()
                    .w(px(dots_width))
                    .children((0..rows).map(move |_| {
                        div()
                            .flex()
                            .h(px(7.0))
                            .items_center()
                            .children((0..columns).map(move |_| {
                                div()
                                    .flex()
                                    .w(px(5.0))
                                    .items_center()
                                    .justify_center()
                                    .child(div().size(px(1.6)).rounded_full().bg(handle))
                            }))
                    })),
            )
            .into_any_element()
    }

    /// `StopControl`: the dancing sticks, or `■ Stop` while hovered.
    fn render_stop_control(&self, colors: &BarColors, cx: &mut Context<Self>) -> AnyElement {
        let width = if self.state.live_caption_toggle_visible {
            COMPACT_STOP_WIDTH
        } else {
            COMPACT_SOLO_STOP_WIDTH
        };
        let accent = colors.accent;
        div()
            .id("floating-bar-stop")
            .flex()
            .w(px(width))
            .h(px(COMPACT_ICON_SIZE))
            .items_center()
            .justify_center()
            .rounded(px(CONTROL_RADIUS))
            .bg(if self.stop_hovered {
                with_alpha(0xff334d, 0.18)
            } else {
                colors.control_fill
            })
            .cursor_pointer()
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.stop_hovered = *hovered;
                cx.notify();
            }))
            .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                // `onStop` → the main window's `stopListening`.
                this.workspace
                    .update(cx, |workspace, cx| workspace.stop_listening(cx))
                    .ok();
            }))
            .child(if self.stop_hovered {
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .tw_text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(accent)
                    .child(div().size(px(9.0)).rounded(px(1.0)).bg(accent))
                    .child("Stop")
                    .into_any_element()
            } else {
                super::recording::dancing_sticks(self.state.amplitude, accent, 20.0, 26.0, 3.0, 2.0)
            })
            .into_any_element()
    }
}

impl Render for FloatingBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = bar_colors(&self.state);
        let width = compact_width(self.state.live_caption_toggle_visible);
        let height = COMPACT_HEIGHT
            + if self.hovered {
                HOVER_HANDLE_RESERVED_HEIGHT
            } else {
                0.0
            };
        let hovered = self.hovered;
        let content = colors.content;
        // `FloatingBarOverlay`: the pill sits at the bottom right of the
        // `INSET` container; the whole window is a drag region.
        div()
            .id("floating-bar")
            .size_full()
            .flex()
            .items_end()
            .justify_end()
            .p(px(INSET))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.hovered = *hovered;
                cx.notify();
            }))
            .on_mouse_down(gpui::MouseButton::Left, |_, window, _| {
                window.start_window_move();
            })
            .child(
                div()
                    .relative()
                    .overflow_hidden()
                    .w(px(width))
                    .h(px(height))
                    .rounded(px(COMPACT_RADIUS))
                    .bg(if hovered {
                        colors.envelope_surface
                    } else {
                        colors.surface
                    })
                    .border(px(0.5))
                    .border_color(colors.outer_stroke)
                    .when(hovered, |pill| {
                        pill.child(
                            div()
                                .absolute()
                                .top(px(HOVER_HANDLE_TOP_PADDING))
                                .left_0()
                                .child(self.render_hover_handle(&colors, width)),
                        )
                    })
                    .child(
                        div()
                            .absolute()
                            .right_0()
                            .bottom_0()
                            .flex()
                            .w(px(width))
                            .h(px(COMPACT_HEIGHT))
                            .items_center()
                            .justify_center()
                            .child(
                                // `FloatingControls`
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(COMPACT_GAP))
                                    .child(self.render_stop_control(&colors, cx))
                                    .when(self.state.live_caption_toggle_visible, |controls| {
                                        controls.child(
                                            div()
                                                .flex()
                                                .size(px(COMPACT_ICON_SIZE))
                                                .items_center()
                                                .justify_center()
                                                .rounded(px(CONTROL_RADIUS))
                                                .child(icon(
                                                    "arrows-out-simple",
                                                    px(14.0),
                                                    content,
                                                )),
                                        )
                                    }),
                            ),
                    ),
            )
    }
}

/// `windows.floatingBarShow` + `floatingBarUpdate`: open the window on the
/// primary display's top right (or update the shown one).
pub fn show(
    workspace: WeakEntity<Workspace>,
    state: FloatingBarState,
    cx: &mut App,
) -> Option<gpui::WindowHandle<FloatingBar>> {
    let (width, height) = compact_container_size(state.live_caption_toggle_visible);
    let display = cx.primary_display()?;
    let bounds = display.bounds();
    let (x, y) = top_right_origin(
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        width,
    );
    let window_bounds =
        gpui::Bounds::new(gpui::point(px(x), px(y)), gpui::size(px(width), px(height)));
    let result = cx.open_window(
        gpui::WindowOptions {
            window_bounds: Some(gpui::WindowBounds::Windowed(window_bounds)),
            titlebar: None,
            focus: false,
            show: true,
            kind: gpui::WindowKind::PopUp,
            window_decorations: Some(gpui::WindowDecorations::Client),
            is_movable: true,
            is_resizable: false,
            is_minimizable: false,
            window_background: gpui::WindowBackgroundAppearance::Transparent,
            app_id: Some(crate::APP_ID.to_string()),
            ..Default::default()
        },
        move |_, cx| cx.new(|_| FloatingBar::new(state, workspace)),
    );
    match result {
        Ok(handle) => Some(handle),
        Err(error) => {
            tracing::error!(%error, "failed to open the floating bar window");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_sizes_follow_the_layout_constants() {
        assert_eq!(compact_controls_width(false), 68.0);
        assert_eq!(compact_controls_width(true), 62.0 + 3.0 + 30.0);
        assert_eq!(compact_width(false), 76.0);
        assert_eq!(compact_container_size(false), (84.0, 38.0 + 21.0 + 8.0));
        assert_eq!(compact_container_size(true), (111.0, 67.0));
    }

    #[test]
    fn the_window_sits_inside_the_top_right_margin() {
        assert_eq!(top_right_origin(0.0, 29.0, 1920.0, 84.0), (1828.0, 37.0));
    }

    #[test]
    fn colours_follow_bar_colors() {
        let state = FloatingBarState {
            amplitude: 0.0,
            title: String::new(),
            error: false,
            dark: false,
            opacity: 0.78,
            live_caption_toggle_visible: false,
        };
        let colors = bar_colors(&state);
        // rgba(219, 217, 209, 0.78 * 0.82)
        assert!((colors.surface.a - 0.6396).abs() < 0.01);
        assert!((colors.envelope_surface.a - 0.8424).abs() < 0.01);
        assert_eq!(colors.accent, gpui::rgb(0xff334d));
        let error = bar_colors(&FloatingBarState {
            error: true,
            ..state.clone()
        });
        assert_eq!(error.accent, gpui::rgb(0xff403d));
        // The opacity is clamped to `[0.35, 0.95]`.
        let dim = bar_colors(&FloatingBarState {
            opacity: 0.1,
            ..state
        });
        assert!((dim.surface.a - 0.35 * 0.82).abs() < 0.01);
    }

    #[test]
    fn unused_state_fields_are_kept_for_the_expanded_panel() {
        let state = FloatingBarState {
            amplitude: 0.5,
            title: "Standup".into(),
            error: false,
            dark: true,
            opacity: 0.78,
            live_caption_toggle_visible: true,
        };
        assert_eq!(state.title, "Standup");
        assert!(bar_colors(&state).content.r > 0.99);
    }
}
