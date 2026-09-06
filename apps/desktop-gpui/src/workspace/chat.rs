//! `PersistentChat` in its `FloatingOpen` mode (`chat/components/*`): the
//! floating panel over the main surface with the toolbar and, without a
//! language model, `ChatBodyEmpty`'s setup prompt. `mod+j` toggles it,
//! Escape and a press on the frame close it.

use gpui::{AnyElement, ClickEvent, Context, MouseButton, MouseDownEvent, div, prelude::*, px};

use super::Workspace;
use crate::theme::alpha;
use crate::ui::{TailwindText as _, icon};

/// `FLOATING_PANEL_MIN_WIDTH`
const PANEL_MIN_WIDTH: f32 = 476.0;
/// `FLOATING_CHAT_INPUT_MAX_WIDTH + FLOATING_CHAT_SHELL_INSET * 2`
const PANEL_MAX_WIDTH: f32 = 648.0;
/// `FLOATING_PANEL_TOP_CLEARANCE`
const TOP_CLEARANCE: f32 = 46.0;

impl Workspace {
    /// `chat.sendEvent({ type: "TOGGLE" })`
    pub(crate) fn toggle_chat(&mut self, cx: &mut Context<Self>) {
        self.chat_open = !self.chat_open;
        cx.notify();
    }

    pub(crate) fn close_chat(&mut self, cx: &mut Context<Self>) {
        if self.chat_open {
            self.chat_open = false;
            cx.notify();
        }
    }

    /// `ChatBodyEmpty` without a model: the greeting, the Beta chip, and the
    /// `Open AI Settings` button. Shared by the floating frame and the
    /// automations tab's right panel.
    pub(super) fn render_chat_body(&self, cx: &Context<Self>) -> AnyElement {
        let theme = self.theme;
        div()
            .flex()
            .flex_col()
            .px_5()
            .py_3()
            .child(
                div()
                    .flex()
                    .py_2()
                    .pb_1()
                    .child(
                        div()
                            .flex()
                            .w_full()
                            .flex_col()
                            .child(
                                div()
                                    .mb_2()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .tw_text_sm()
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .text_color(theme.foreground)
                                            .child("Anarlog AI"),
                                    )
                                    .child(
                                        // `BetaChip`: `rounded-full border px-1.5 py-0.5
                                        // text-[10px] font-medium border-sky-200 bg-sky-100
                                        // text-sky-900`.
                                        div()
                                            .rounded(px(8.0))
                                            .border_1()
                                            .border_color(gpui::rgb(0xbae6fd))
                                            .bg(gpui::rgb(0xe0f2fe))
                                            .text_color(gpui::rgb(0x0c4a6e))
                                            .px(px(6.0))
                                            .py(px(2.0))
                                            .text_size(px(10.0))
                                            .line_height(px(15.0))
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .child("Beta"),
                                    ),
                            )
                            .child(
                                div()
                                    .mb_2()
                                    .tw_text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child("Hi, I'm Anarlog AI. Set up a language model and I'll be ready to help."),
                            )
                            .child(
                                div().flex().child(
                                div()
                                    .id("chat-open-ai-settings")
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    .rounded(px(8.0))
                                    .border_1()
                                    .border_color(theme.primary)
                                    .bg(theme.primary)
                                    .text_color(theme.primary_foreground)
                                    .px_3()
                                    .py(px(6.0))
                                    .tw_text_xs()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .shadow(vec![gpui::BoxShadow {
                                        color: gpui::Rgba {
                                            r: 87.0 / 255.0,
                                            g: 83.0 / 255.0,
                                            b: 78.0 / 255.0,
                                            a: 0.18,
                                        }
                                        .into(),
                                        offset: gpui::point(px(0.0), px(4.0)),
                                        blur_radius: px(14.0),
                                        spread_radius: px(0.0),
                                    }])
                                    .cursor_pointer()
                                    .hover(move |style| style.bg(alpha(theme.primary, 0.9)))
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        this.close_chat(cx);
                                        this.open_settings(
                                            super::settings::SettingsTab::Intelligence,
                                            window,
                                            cx,
                                        );
                                    }))
                                    .child(icon("sparkle", px(12.0), theme.primary_foreground))
                                    .child("Open AI Settings"),
                            )),
                    ),
            )
        .into_any_element()
    }

    /// The `[data-chat-floating-frame]` over the main surface: `items-end
    /// justify-center px-3 pb-2` with the top clearance, closing on a press
    /// outside the panel.
    pub(super) fn render_chat_frame(&self, cx: &Context<Self>) -> Option<AnyElement> {
        if !self.chat_open {
            return None;
        }
        let theme = self.theme;
        let panel_bg = if theme.dark {
            gpui::rgb(0x202020)
        } else {
            gpui::rgb(0xf4f4f5)
        };
        let ghost = |id: &'static str, glyph: &'static str| {
            div()
                .id(id)
                .flex()
                .size(px(32.0))
                .items_center()
                .justify_center()
                .rounded(px(8.0))
                .cursor_pointer()
                .hover(move |style| style.bg(alpha(theme.muted, 0.8)))
                .child(icon(glyph, px(16.0), theme.muted_foreground))
        };
        // `ChatGroups` trigger: `-ml-2 h-8 gap-1.5 rounded-full px-2.5`.
        let history = div()
            .id("chat-history")
            .flex()
            .h(px(32.0))
            .items_center()
            .gap(px(6.0))
            .ml(px(-8.0))
            .px(px(10.0))
            .rounded(px(8.0))
            .cursor_pointer()
            .hover(move |style| style.bg(alpha(theme.muted, 0.8)))
            .child(icon(
                "clock-counter-clockwise",
                px(16.0),
                theme.muted_foreground,
            ))
            .child(icon("caret-down", px(14.0), theme.muted_foreground));
        let toolbar = div()
            .flex()
            .h(px(44.0))
            .flex_shrink_0()
            .items_center()
            .gap_2()
            .px_3()
            .child(
                div()
                    .flex()
                    .min_w_0()
                    .flex_1()
                    .items_center()
                    .gap_1()
                    .child(history),
            )
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .child(ghost("chat-new", "plus"))
                    .child(ghost("chat-right-panel", "sidebar-left")),
            );

        let body = self.render_chat_body(cx);

        let panel = div()
            .id("chat-panel")
            .relative()
            .flex()
            .flex_col()
            .w_full()
            .min_w(px(PANEL_MIN_WIDTH))
            .max_w(px(PANEL_MAX_WIDTH))
            .rounded(px(24.0))
            .border_1()
            .border_color(alpha(theme.border, 0.7))
            .bg(panel_bg)
            .shadow(vec![gpui::BoxShadow {
                color: gpui::hsla(0.0, 0.0, 0.0, 0.32),
                offset: gpui::point(px(0.0), px(32.0)),
                blur_radius: px(84.0),
                spread_radius: px(0.0),
            }])
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            // `border-t-app-floating-border`: the lighter top edge.
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(1.0))
                    .bg(theme.floating_border),
            )
            .child(toolbar)
            .child(body);

        Some(
            div()
                .id("chat-frame")
                .absolute()
                .inset_0()
                .flex()
                .items_end()
                .justify_center()
                .px_3()
                .pb_2()
                .pt(px(TOP_CLEARANCE))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        this.close_chat(cx);
                    }),
                )
                .child(panel)
                .into_any_element(),
        )
    }
}
