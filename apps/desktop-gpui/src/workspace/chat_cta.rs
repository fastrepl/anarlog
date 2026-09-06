//! `ChatCTA` / `FloatingActionButton` (`shared/chat-cta.tsx`,
//! `session/components/floating/index.tsx`): the slim "Ask anything" pill at
//! the bottom of the main surface that grows into a prompt-like bar on hover
//! and opens the chat; sessions stack the transcript selection bar above it.

use gpui::{
    AnyElement, ClickEvent, Context, MouseButton, div, linear_color_stop, linear_gradient,
    prelude::*, px, relative,
};

use super::Workspace;
use crate::db::NotePreview;
use crate::theme::alpha;
use crate::ui::TailwindText as _;

/// `h-10 w-[180px]`
const CTA_WIDTH: f32 = 180.0;
const CTA_HEIGHT: f32 = 40.0;

impl Workspace {
    pub(super) fn chat_cta_hovered(&self) -> bool {
        self.chat_cta_hovered
    }

    /// The FAB column for the main surface: nothing while the chat is open,
    /// otherwise the CTA with, for a session, the selection bar slot above
    /// it. `preview` is the open session, `None` on the empty view.
    pub(super) fn render_floating_action_button(
        &self,
        preview: Option<&NotePreview>,
        window: &gpui::Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        if self.chat_open {
            return None;
        }
        let bar = preview.and_then(|preview| self.render_transcript_selection_bar(preview, cx));
        Some(
            div()
                .absolute()
                .bottom(px(12.0))
                .left(relative(0.5))
                .w(px(0.0))
                .flex()
                .justify_center()
                .child(self.render_chat_cta(window, cx))
                .children(bar)
                .into_any_element(),
        )
    }

    /// `ChatCTA`: a `h-10 w-[180px] cursor-text` hover target whose visible
    /// surface is the `h-2` gradient pill at its bottom, growing to a
    /// `h-10 w-[min(640px,100cqw-2rem)] px-4` bar with the "Ask anything"
    /// label while hovered. Clicking opens the chat.
    fn render_chat_cta(&self, window: &gpui::Window, cx: &Context<Self>) -> AnyElement {
        let theme = self.theme;
        let hovered = self.chat_cta_hovered;
        // `100cqw`: the main surface (the viewport minus the sidebar, its
        // `pl-1` gutter and the left border).
        let surface_width = (f32::from(window.viewport_size().width)
            - if self.sidebar_expanded && !self.is_standalone() {
                self.custom_sidebar_width() + 4.0 + 1.0
            } else {
                0.0
            })
        .max(CTA_WIDTH + 32.0);
        let expanded_width = 640.0_f32.min(surface_width - 32.0);

        let surface = if hovered {
            div()
                .absolute()
                .bottom_0()
                .left(relative(0.5))
                .ml(px(-expanded_width / 2.0))
                .flex()
                .h(px(CTA_HEIGHT))
                .w(px(expanded_width))
                .items_center()
                .px_4()
                .rounded_full()
                .border_1()
                .border_color(alpha(theme.border, 0.7))
                .bg(if theme.dark {
                    gpui::rgb(0x202020)
                } else {
                    gpui::rgb(0xf4f4f5)
                })
                .shadow(vec![gpui::BoxShadow {
                    color: gpui::hsla(0.0, 0.0, 0.0, if theme.dark { 0.64 } else { 0.26 }),
                    offset: gpui::point(px(0.0), px(if theme.dark { 18.0 } else { 16.0 })),
                    blur_radius: px(if theme.dark { 52.0 } else { 42.0 }),
                    spread_radius: px(0.0),
                }])
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .tw_text_sm()
                        .text_color(theme.muted_foreground)
                        .truncate()
                        .child("Ask anything"),
                )
        } else {
            // `h-2` (`dark:h-3`) with the `0 0 0 1px rgb(0 0 0 / 0.1)` ring
            // and the inset highlights drawn as hairlines.
            let height = if theme.dark { 12.0 } else { 8.0 };
            let (top, bottom) = if theme.dark {
                (gpui::rgb(0x211d1d), gpui::rgb(0x574f3b))
            } else {
                (gpui::rgb(0xfaf8f6), gpui::rgb(0xe3e1df))
            };
            div()
                .absolute()
                .bottom(px(-1.0))
                .left(relative(0.5))
                .ml(px(-(CTA_WIDTH + 2.0) / 2.0))
                .flex()
                .flex_col()
                .justify_between()
                .h(px(height + 2.0))
                .w(px(CTA_WIDTH + 2.0))
                .rounded_full()
                .overflow_hidden()
                .border_1()
                .border_color(gpui::hsla(0.0, 0.0, 0.0, 0.1))
                .bg(linear_gradient(
                    180.0,
                    linear_color_stop(top, 0.0),
                    linear_color_stop(bottom, 1.0),
                ))
                .shadow(vec![
                    gpui::BoxShadow {
                        color: gpui::hsla(0.0, 0.0, 0.0, 0.16),
                        offset: gpui::point(px(0.0), px(4.0)),
                        blur_radius: px(12.0),
                        spread_radius: px(0.0),
                    },
                    gpui::BoxShadow {
                        color: gpui::hsla(0.0, 0.0, 0.0, 0.1),
                        offset: gpui::point(px(0.0), px(4.0)),
                        blur_radius: px(16.0),
                        spread_radius: px(0.0),
                    },
                ])
                .child(div().h(px(1.0)).w_full().bg(gpui::hsla(0.0, 0.0, 1.0, 0.4)))
                .child(
                    div()
                        .h(px(1.0))
                        .w_full()
                        .bg(gpui::hsla(0.0, 0.0, 0.0, 0.25)),
                )
        };

        div()
            .id("chat-cta")
            .relative()
            .h(px(CTA_HEIGHT))
            .w(px(CTA_WIDTH))
            .flex_shrink_0()
            .cursor_text()
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if this.chat_cta_hovered != *hovered {
                    this.chat_cta_hovered = *hovered;
                    cx.notify();
                }
            }))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                if !this.chat_open {
                    this.toggle_chat(cx);
                }
            }))
            .child(surface)
            .into_any_element()
    }
}
