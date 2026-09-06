//! `SessionShareButton variant="cta"` while signed out
//! (`session-sharing/{index,draft-panel,invite-recipients,general-access}.tsx`):
//! the `w-[440px]` app popover under the Share CTA showing the draft share
//! panel behind the `Sign in to share` gate.

use std::sync::Arc;

use gpui::{
    AnyElement, ClickEvent, Context, MouseButton, MouseDownEvent, RenderImage, div, prelude::*, px,
};

use super::Workspace;
use crate::theme::alpha;
use crate::ui::{TailwindText as _, icon};

pub(crate) struct SharePopover {
    session_id: String,
    /// The owner's facehash for the `People with access` row (`You`
    /// while signed out).
    avatar: Arc<RenderImage>,
    opening_sign_in: bool,
}

impl Workspace {
    pub(crate) fn share_popover_open(&self) -> bool {
        self.share_popover.is_some()
    }

    /// The Share CTA toggles the popover (`Popover open` on the button).
    pub(crate) fn toggle_share_popover(&mut self, session_id: String, cx: &mut Context<Self>) {
        if self.share_popover.is_some() {
            self.share_popover = None;
        } else {
            self.share_popover = Some(SharePopover {
                session_id,
                avatar: super::contacts_tab::rasterize_avatar("You"),
                opening_sign_in: false,
            });
        }
        cx.notify();
    }

    pub(crate) fn close_share_popover(&mut self, cx: &mut Context<Self>) {
        if self.share_popover.is_some() {
            self.share_popover = None;
            cx.notify();
        }
    }

    /// `PopoverContent variant="app" align="end" sideOffset={8}` anchored to
    /// the CTA: the draft panel (dimmed and inert behind the gate — GPUI has
    /// no `blur-[3px]`) and the centred sign-in gate.
    pub(super) fn render_share_popover(
        &self,
        session_id: &str,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let popover = self.share_popover.as_ref()?;
        if popover.session_id != session_id {
            return None;
        }
        let theme = self.theme;
        let owner = "You";

        // `ShareInviteForm`: the `h-8 rounded-full border px-3` field and the
        // `h-8 rounded-full px-3` Invite button (disabled).
        let invite_form = div()
            .flex()
            .items_start()
            .gap_2()
            .child(
                div()
                    .flex()
                    .h(px(32.0))
                    .min_w_0()
                    .flex_1()
                    .items_center()
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(theme.border)
                    .px_3()
                    .opacity(0.5)
                    .child(
                        div()
                            .tw_text_xs()
                            .text_color(theme.muted_foreground)
                            .child("Email or name"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .h(px(32.0))
                    .flex_shrink_0()
                    .items_center()
                    .px_3()
                    .rounded(px(8.0))
                    .bg(theme.primary)
                    .text_color(theme.primary_foreground)
                    .tw_text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .opacity(0.5)
                    .child("Invite"),
            );

        // `People with access`
        let people = div()
            .mt_2()
            .pt_2()
            .border_t_1()
            .border_color(alpha(theme.border, 0.6))
            .child(
                div()
                    .mb_1()
                    .px(px(6.0))
                    .text_size(px(10.0))
                    .line_height(px(15.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.muted_foreground)
                    .child("People with access"),
            )
            .child(
                div()
                    .flex()
                    .min_h(px(36.0))
                    .items_center()
                    .gap_2()
                    .rounded(px(8.0))
                    .px(px(6.0))
                    .py_1()
                    .child(Self::render_avatar_image(
                        Some(popover.avatar.clone()),
                        owner,
                        24.0,
                    ))
                    .child(
                        div().min_w_0().flex_1().child(
                            div()
                                .flex()
                                .gap_1()
                                .tw_text_xs()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child(owner)
                                .child(
                                    div()
                                        .font_weight(gpui::FontWeight::NORMAL)
                                        .text_color(theme.muted_foreground)
                                        .child("(You)"),
                                ),
                        ),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(11.0))
                            .line_height(px(16.0))
                            .text_color(theme.muted_foreground)
                            .child("Full access"),
                    ),
            );

        // Footer: `GeneralAccessSelector` (the `bg-muted size-7 rounded-md`
        // icon box and the `h-7 px-1.5 text-xs` trigger), the overflow, and
        // the `h-7 rounded-md px-2.5 text-xs` Copy link button.
        let footer = div()
            .flex()
            .items_center()
            .gap_1()
            .border_t_1()
            .border_color(alpha(theme.border, 0.6))
            .px_3()
            .py_2()
            .child(
                div()
                    .flex()
                    .min_w_0()
                    .flex_1()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        div()
                            .flex()
                            .size(px(28.0))
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .bg(theme.muted)
                            .child(icon("lock-key", px(16.0), theme.foreground)),
                    )
                    .child(
                        div()
                            .flex()
                            .h(px(28.0))
                            .items_center()
                            .gap_1()
                            .px(px(6.0))
                            .rounded_md()
                            .tw_text_xs()
                            .opacity(0.5)
                            .child("Only people invited")
                            .child(icon("caret-down", px(14.0), theme.muted_foreground)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .size(px(32.0))
                    .items_center()
                    .justify_center()
                    .rounded_md()
                    .child(icon("dots-three", px(16.0), theme.muted_foreground)),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .h(px(28.0))
                    .flex_shrink_0()
                    .items_center()
                    .gap_2()
                    .px(px(10.0))
                    .tw_text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .opacity(0.5)
                    .child(crate::squircle::squircle(
                        crate::squircle::CONTROL_RADIUS,
                        Some(theme.background),
                        Some((1.0, theme.border)),
                    ))
                    .child(
                        div()
                            .relative()
                            .child(icon("copy", px(16.0), theme.foreground)),
                    )
                    .child(div().relative().child("Copy link")),
            );

        let draft = div()
            .flex()
            .flex_col()
            .child(
                div()
                    .px_3()
                    .py_2()
                    .child(div().flex().flex_col().child(invite_form).child(people)),
            )
            .child(footer);

        let opening = popover.opening_sign_in;
        let gate = div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .px_6()
            .bg(alpha(theme.background, 0.7))
            .child(
                div()
                    .flex()
                    .max_w(px(220.0))
                    .flex_col()
                    .items_center()
                    .child(
                        div()
                            .tw_text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.foreground)
                            .child("Sign in to share"),
                    )
                    .child(
                        div()
                            .mt_1()
                            .tw_text_xs()
                            .line_height(px(20.0))
                            .text_color(theme.muted_foreground)
                            .text_center()
                            .child("Sign in to share this note with others."),
                    )
                    .child(
                        div().mt_4().child(
                            // `Button size="sm"`: `h-8 px-3 text-sm`.
                            div()
                                .id("share-sign-in")
                                .relative()
                                .flex()
                                .h(px(32.0))
                                .items_center()
                                .px_3()
                                .tw_text_sm()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(theme.primary_foreground)
                                .when(opening, |button| button.opacity(0.5))
                                .when(!opening, |button| button.cursor_pointer())
                                .child(crate::squircle::squircle(
                                    crate::squircle::CONTROL_RADIUS,
                                    Some(theme.primary),
                                    None,
                                ))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    let url = this.auth_url();
                                    if let Some(popover) = this.share_popover.as_mut() {
                                        popover.opening_sign_in = true;
                                    }
                                    cx.open_url(&url);
                                    cx.spawn(async move |this, cx| {
                                        cx.background_executor()
                                            .timer(std::time::Duration::from_millis(600))
                                            .await;
                                        this.update(cx, |this, cx| {
                                            if let Some(popover) = this.share_popover.as_mut() {
                                                popover.opening_sign_in = false;
                                                cx.notify();
                                            }
                                        })
                                        .ok();
                                    })
                                    .detach();
                                    cx.notify();
                                }))
                                .child(div().relative().child(if opening {
                                    "Opening…"
                                } else {
                                    "Sign in"
                                })),
                        ),
                    ),
            );

        let panel = super::menu::menu_chrome(theme, "share-popover", 440.0)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| {
                this.close_share_popover(cx);
            }))
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .max_h(px(530.0))
                    .overflow_hidden()
                    .rounded(px(crate::squircle::PANEL_RADIUS))
                    .child(crate::squircle::squircle(
                        crate::squircle::PANEL_RADIUS,
                        Some(theme.floating_panel),
                        Some((1.0, theme.floating_border)),
                    ))
                    .child(div().relative().child(draft))
                    .child(gate),
            );

        Some(
            div()
                .absolute()
                .top(px(32.0 + 8.0))
                .right_0()
                .child(
                    gpui::deferred(
                        gpui::anchored()
                            .anchor(gpui::Corner::TopRight)
                            .snap_to_window_with_margin(px(8.0))
                            .child(panel),
                    )
                    .with_priority(3),
                )
                .into_any_element(),
        )
    }
}
