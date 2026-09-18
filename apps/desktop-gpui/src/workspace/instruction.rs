//! `instruction/index.tsx` + the windows plugin's `openUrlWithInstruction` /
//! `dismissInstruction`: a browser hand-off (sign-in, checkout) swaps the
//! whole window for the `InstructionScreen` — `Back`, the app icon, a title
//! and description, the reopen button or the callback-link fallback — while
//! the URL opens in the browser, and `Back` returns to the app.
//!
//! The Tauri plugin also saves the window frame and animates it to 340×500 at
//! the screen's top-right on macOS only (`set_frame_animated` is a no-op
//! elsewhere); GPUI can resize but not move a window, so macOS gets the size
//! change in place and the frame comes back on dismiss.

use gpui::{
    AnyElement, ClickEvent, Context, Entity, Focusable as _, MouseButton, Pixels, Size, Window,
    div, img, linear_color_stop, linear_gradient, prelude::*, px, relative,
};

use super::Workspace;
use super::note::embedded;
use crate::text_input::TextInput;
use crate::theme::alpha;
use crate::ui::{TailwindText as _, icon};

/// `InstructionType`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstructionKind {
    SignIn,
    Billing,
}

pub(crate) struct Instruction {
    kind: InstructionKind,
    url: String,
    /// `showCallbackInput`
    show_callback: bool,
    callback: Entity<TextInput>,
    /// `windowSaveFrame`: the content size to restore on dismiss.
    saved_size: Option<Size<Pixels>>,
}

/// `windowSetFrameAnimated({ type: "main" }, "TopRight", 340, 500)`
const HANDOFF_SIZE: Size<Pixels> = Size {
    width: px(340.0),
    height: px(500.0),
};

impl Workspace {
    pub(crate) fn instruction_open(&self) -> bool {
        self.instruction.is_some()
    }

    /// `signIn`: `buildWebAppUrl("/auth")` through the sign-in instruction.
    pub(crate) fn sign_in(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let url = self.auth_url();
        self.open_url_with_instruction(url, InstructionKind::SignIn, window, cx);
    }

    /// `openUpgrade("feature_gate")`: the checkout page through the billing
    /// instruction.
    pub(crate) fn upgrade_to_pro(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let url = self.web_app_url(
            "/app/checkout",
            &[("period", "monthly"), ("source", "feature_gate")],
        );
        self.open_url_with_instruction(url, InstructionKind::Billing, window, cx);
    }

    /// `openUrlWithInstruction(url, type, openUrl)`
    pub(crate) fn open_url_with_instruction(
        &mut self,
        url: String,
        kind: InstructionKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let saved_size = if cfg!(target_os = "macos") {
            let current = window.viewport_size();
            window.resize(HANDOFF_SIZE);
            Some(current)
        } else {
            None
        };
        let style = self.plain_input_style();
        let callback = cx.new(|cx| {
            TextInput::new(
                "anarlog://auth/callback?access_token=...",
                style,
                window,
                cx,
            )
        });
        self.instruction = Some(Instruction {
            kind,
            url: url.clone(),
            show_callback: false,
            callback,
            saved_size,
        });
        crate::opener::open_url(&url);
        cx.notify();
    }

    /// `dismissInstruction`: back to `/app`, then `windowRestoreFrameAnimated`.
    pub(crate) fn dismiss_instruction(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(instruction) = self.instruction.take() {
            if let Some(saved) = instruction.saved_size {
                window.resize(saved);
            }
            self.focus_handle.focus(window);
            cx.notify();
        }
    }

    /// `auth.handleAuthCallback(callbackUrl)`: the pasted link takes the
    /// deep-link route.
    fn submit_callback_url(&mut self, cx: &mut Context<Self>) {
        let Some(instruction) = self.instruction.as_ref() else {
            return;
        };
        let url = instruction.callback.read(cx).text().trim().to_string();
        if url.is_empty() {
            return;
        }
        if let crate::deeplink::Incoming::DeepLink(link) = crate::deeplink::classify(&url) {
            self.handle_deep_link(link, cx);
        }
    }

    /// `InstructionScreen`, filling the window.
    pub(super) fn render_instruction(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let Some(instruction) = self.instruction.as_ref() else {
            return div().into_any_element();
        };
        let kind = instruction.kind;
        let url = instruction.url.clone();
        let show_callback = instruction.show_callback;
        let callback = instruction.callback.clone();
        let callback_empty = callback.read(cx).text().trim().is_empty();
        let callback_focused = callback.read(cx).focus_handle(cx).is_focused(window);
        // `sm:text-[28px]` above the 640px breakpoint, `text-[22px]` below
        // (the 340px hand-off window); `leading-[1.15]` as WebKit floors it.
        let viewport_width = f32::from(window.viewport_size().width);
        let wide = viewport_width >= 640.0;
        let (title_px, title_line) = if wide { (28.0, 32.0) } else { (22.0, 25.0) };
        // `max-w-sm` inside the `p-6` body, less the column's `px-10`: the
        // paragraphs wrap at this width (the intrinsically sized column offers
        // them none while it is measured).
        let text_width = (viewport_width - 48.0).min(384.0) - 80.0;
        let (title, description) = match kind {
            InstructionKind::SignIn => (
                "Sign in to your account",
                "Complete sign-in in your browser, then return to Anarlog.",
            ),
            InstructionKind::Billing => (
                "Upgrade to Pro",
                "Finish checkout in your browser to unlock more, then return to Anarlog.",
            ),
        };
        let prose = |text: &str,
                     font_px: f32,
                     line: f32,
                     color: gpui::Rgba,
                     weight: gpui::FontWeight,
                     pretty: bool| {
            let mut style = window.text_style();
            style.font_size = px(font_px).into();
            style.color = color.into();
            style.font_weight = weight;
            if let Some(font) = &self.font_family {
                style.font_family = font.clone();
            }
            let mut prose = crate::prose_text::ProseText::new(
                text.to_string(),
                vec![style.to_run(text.len())],
                px(font_px),
                px(line),
            )
            .centered()
            .max_width(px(text_width));
            // The app's global rules: `p { text-wrap: pretty }`,
            // `h2 { text-wrap: balance }`.
            prose = if pretty {
                prose.pretty()
            } else {
                prose.balance()
            };
            prose
        };

        // `from-background via-card to-card`: the top half fades into the
        // card colour; `from-muted/40` washes the top 128px.
        let backdrop =
            div()
                .absolute()
                .inset_0()
                .bg(theme.card)
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .h(relative(0.5))
                        .bg(linear_gradient(
                            180.0,
                            linear_color_stop(theme.background, 0.0),
                            linear_color_stop(theme.card, 1.0),
                        )),
                )
                .child(div().absolute().top_0().left_0().right_0().h(px(128.0)).bg(
                    linear_gradient(
                        180.0,
                        linear_color_stop(alpha(theme.muted, 0.4), 0.0),
                        linear_color_stop(alpha(theme.muted, 0.0), 1.0),
                    ),
                ));

        // `px-3 pt-12`, the `h-9 rounded-full px-3 gap-1.5` Back button.
        let back_hovered = self.hovered == Some("instruction-back");
        let header = div()
            .relative()
            .flex()
            .flex_shrink_0()
            .items_center()
            .px_3()
            .pt(px(48.0))
            .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
            .child(
                div()
                    .id("instruction-back")
                    .flex()
                    .h(px(36.0))
                    .items_center()
                    .gap(px(6.0))
                    .rounded_full()
                    .px_3()
                    .when(back_hovered, |button| button.bg(alpha(theme.muted, 0.7)))
                    .cursor_pointer()
                    .on_hover(cx.listener(|this, hovering: &bool, _, cx| {
                        this.set_hovered("instruction-back", *hovering, cx);
                    }))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.dismiss_instruction(window, cx);
                    }))
                    .child(icon("caret-left", px(16.0), theme.muted_foreground))
                    .child(
                        div()
                            .tw_text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.muted_foreground)
                            .child("Back"),
                    ),
            );

        let mut column = div()
            .flex()
            .w_full()
            .max_w(px(384.0))
            .flex_col()
            .items_center()
            .gap_6()
            .px(px(40.0))
            .pb(px(40.0))
            .child(
                img(embedded("anarlog-icon.png"))
                    .size(px(56.0))
                    .flex_shrink_0(),
            )
            .child(
                div()
                    .flex()
                    .w_full()
                    .flex_col()
                    .gap_3()
                    .child(div().w_full().child(prose(
                        title,
                        title_px,
                        title_line,
                        theme.foreground,
                        gpui::FontWeight::SEMIBOLD,
                        false,
                    )))
                    .child(div().w_full().child(prose(
                        description,
                        14.0,
                        24.0,
                        theme.muted_foreground,
                        gpui::FontWeight::NORMAL,
                        true,
                    ))),
            );

        match kind {
            InstructionKind::Billing => {
                // `Button variant="outline"`: `h-10 w-full bg-card
                // text-muted-foreground border-border`, label + `ArrowSquareOut`.
                let hovered = self.hovered == Some("instruction-reopen");
                column = column.child(
                    div().w_full().child(
                        div()
                            .id("instruction-reopen")
                            .flex()
                            .h(px(40.0))
                            .w_full()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .rounded_md()
                            .border_1()
                            .border_color(theme.border)
                            .bg(if hovered {
                                theme.background
                            } else {
                                theme.card
                            })
                            .shadow(crate::ui::input_shadow())
                            .tw_text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.muted_foreground)
                            .cursor_pointer()
                            .on_hover(cx.listener(|this, hovering: &bool, _, cx| {
                                this.set_hovered("instruction-reopen", *hovering, cx);
                            }))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(move |_: &ClickEvent, _, _cx| crate::opener::open_url(&url))
                            .child("Reopen checkout page")
                            .child(icon("arrow-square-out", px(14.0), theme.muted_foreground)),
                    ),
                );
            }
            InstructionKind::SignIn if show_callback => {
                let focus_callback = callback.clone();
                column = column.child(
                    div()
                        .flex()
                        .w_full()
                        .flex_col()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .w_full()
                                .flex_col()
                                .gap_2()
                                .child(
                                    // `Input className="h-10 font-mono text-xs"`
                                    div()
                                        .id("instruction-callback")
                                        .relative()
                                        .flex()
                                        .h(px(40.0))
                                        .w_full()
                                        .items_center()
                                        .rounded_md()
                                        .border_1()
                                        .border_color(theme.border)
                                        .bg(theme.card)
                                        .shadow(crate::ui::input_shadow())
                                        .when(callback_focused, |field| {
                                            field.child(crate::ui::input_focus_ring(theme))
                                        })
                                        .px_3()
                                        // `text-base md:text-sm` outranks the `text-xs`
                                        // class from the 768px breakpoint up.
                                        .map(|field| if viewport_width >= 768.0 { field.tw_text_sm() } else { field.tw_text_xs() })
                                        .when_some(
                                            crate::theme::mono_font_family(cx.text_system()),
                                            |field, family| field.font_family(family),
                                        )
                                        .cursor_text()
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation()
                                        })
                                        .on_click(move |_: &ClickEvent, window, cx| {
                                            focus_callback.read(cx).focus_handle(cx).focus(window);
                                        })
                                        .child(div().min_w_0().flex_1().child(callback.clone())),
                                )
                                .child({
                                    // `Button className="h-10"`, disabled while empty.
                                    let hovered = self.hovered == Some("instruction-submit");
                                    div()
                                        .id("instruction-submit")
                                        .flex()
                                        .h(px(40.0))
                                        .w_full()
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(8.0))
                                        // `disabled:opacity-50` fades the whole button,
                                        // leaving its white label white over the faded fill.
                                        .bg(if callback_empty {
                                            alpha(theme.primary, 0.5)
                                        } else if hovered {
                                            alpha(theme.primary, 0.9)
                                        } else {
                                            theme.primary
                                        })
                                        .shadow(crate::ui::input_shadow())
                                        .tw_text_sm()
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(theme.primary_foreground)
                                        .when(callback_empty, |button| button.cursor_not_allowed())
                                        .when(!callback_empty, |button| {
                                            button
                                                .cursor_pointer()
                                                .on_hover(cx.listener(|this, hovering: &bool, _, cx| {
                                                    this.set_hovered("instruction-submit", *hovering, cx);
                                                }))
                                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                    this.submit_callback_url(cx);
                                                }))
                                        })
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation()
                                        })
                                        .child("Submit callback URL")
                                }),
                        )
                        .child(div().w_full().child(prose(
                            "Paste the browser URL here if the browser button did not reopen Anarlog.",
                            12.0,
                            20.0,
                            theme.muted_foreground,
                            gpui::FontWeight::NORMAL,
                            true,
                        ))),
                );
            }
            InstructionKind::SignIn => {
                // The `text-xs font-medium underline underline-offset-4` text
                // button that reveals the callback field.
                let text = "Browser handoff not working? Paste the callback link instead";
                let mut style = window.text_style();
                style.font_size = px(12.0).into();
                style.color = theme.muted_foreground.into();
                style.font_weight = gpui::FontWeight::MEDIUM;
                style.underline = Some(gpui::UnderlineStyle {
                    thickness: px(1.0),
                    color: Some(theme.muted_foreground.into()),
                    wavy: false,
                });
                if let Some(font) = &self.font_family {
                    style.font_family = font.clone();
                }
                column = column.child(
                    div()
                        .flex()
                        .w_full()
                        .flex_col()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .id("instruction-show-callback")
                                .w_full()
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    if let Some(instruction) = this.instruction.as_mut() {
                                        instruction.show_callback = true;
                                        cx.notify();
                                    }
                                }))
                                .child(
                                    crate::prose_text::ProseText::new(
                                        text.to_string(),
                                        vec![style.to_run(text.len())],
                                        px(12.0),
                                        px(16.0),
                                    )
                                    .centered()
                                    .max_width(px(text_width)),
                                ),
                        ),
                );
            }
        }

        div()
            .id("instruction-screen")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .track_focus(&self.focus_handle)
            .when_some(self.font_family.clone(), |root, family| {
                root.font_family(family)
            })
            .child(backdrop)
            .child(header)
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .p_6()
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
                    .child(column),
            )
            .into_any_element()
    }

    /// `buildWebAppUrl(path, params)`: `flow=desktop&scheme=…` first, then
    /// the caller's parameters in order.
    pub(crate) fn web_app_url(&self, path: &str, params: &[(&str, &str)]) -> String {
        let mut url = format!(
            "{}{}?flow=desktop&scheme={}",
            super::settings::web_app_url(),
            path,
            crate::deeplink::scheme(self.store.identifier())
        );
        for (key, value) in params {
            url.push('&');
            url.push_str(key);
            url.push('=');
            url.push_str(value);
        }
        url
    }
}
