//! `apps/desktop/src/sidebar/toast/registry.tsx` + `index.tsx`: one toast at a
//! time, the first registry entry whose condition holds, shown through the
//! `Toaster` in `packages/ui` (sonner, bottom-right, 300px wide).

use gpui::{
    AnyElement, BoxShadow, ClickEvent, Context, MouseButton, SharedString, Window, div, hsla,
    point, prelude::*, px,
};

use super::Workspace;
use crate::actions;
use crate::db::ProviderSettings;
use crate::theme::alpha;

/// What the shell knows about the account. Without the auth service the
/// session never resolves, which is also the state of the Tauri app when
/// Supabase is unreachable, so the sign-in and Pro promotions stay hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Auth {
    Loading,
    #[allow(dead_code)]
    SignedOut,
    #[allow(dead_code)]
    SignedIn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlashVariant {
    Success,
    Error,
    Warning,
    Info,
}

/// A transient sonner toast (`success` / `error` / `warning`).
pub(crate) struct FlashToast {
    variant: FlashVariant,
    message: SharedString,
    /// sonner's `description` under the title.
    description: Option<SharedString>,
    /// sonner's `action` button.
    action: Option<(&'static str, Box<dyn gpui::Action>)>,
    /// sonner's `id`, for a later `toast.dismiss(id)`.
    id: Option<&'static str>,
    generation: u64,
}

pub(crate) struct Toast {
    pub id: &'static str,
    pub description: SharedString,
    pub action: Option<(&'static str, Box<dyn gpui::Action>)>,
    /// A `persistent` lifecycle: sonner's close button, whose dismissal is
    /// remembered under this `dismissalId`.
    pub dismissal_id: Option<&'static str>,
}

/// `createToastRegistry` reduced to the conditions the shell can evaluate;
/// downloads, updates, and the local STT server are not wired yet.
pub(crate) fn current_toast(
    settings: &ProviderSettings,
    auth: Auth,
    dismissed: &[String],
) -> Option<Toast> {
    // `isToastDismissed` for the `permanent` lifecycle: both promotions
    // share the `auth-promotion` dismissal id.
    let promotion_dismissed = dismissed.iter().any(|id| id == "auth-promotion");
    let is_auth_loading = auth == Auth::Loading;
    let is_authenticated = auth == Auth::SignedIn;
    let has_usable_stt =
        settings.has_stt() && (is_auth_loading || is_authenticated || !settings.has_pro_stt());
    let has_usable_llm =
        settings.has_llm() && (is_auth_loading || is_authenticated || !settings.has_pro_llm());

    if !is_auth_loading && !is_authenticated && !promotion_dismissed {
        return Some(Toast {
            id: "sign-in-benefits",
            description: "Sign in to get the most out of Anarlog".into(),
            action: Some(("Sign in", Box::new(actions::SignIn))),
            dismissal_id: Some("auth-promotion"),
        });
    }
    if !has_usable_stt {
        return Some(Toast {
            id: "missing-stt",
            description: "Transcription provider needed".into(),
            action: Some(("Add", Box::new(actions::OpenTranscriptionSettings))),
            dismissal_id: None,
        });
    }
    if !has_usable_llm {
        return Some(Toast {
            id: "missing-llm",
            description: "Language model needed".into(),
            action: Some(("Add", Box::new(actions::OpenIntelligenceSettings))),
            dismissal_id: None,
        });
    }
    if !is_auth_loading
        && !is_authenticated
        && !promotion_dismissed
        && settings.has_llm()
        && settings.has_stt()
        && !settings.has_pro_stt()
        && !settings.has_pro_llm()
    {
        // `onSignIn`: both promotions hand off to the browser sign-in.
        return Some(Toast {
            id: "upgrade-to-pro",
            description: "Pro features available".into(),
            action: Some(("Upgrade", Box::new(actions::SignIn))),
            dismissal_id: Some("auth-promotion"),
        });
    }
    None
}

impl Workspace {
    /// `<Toaster position="bottom-right">`. Measured against the app, the
    /// toast renders sonner's own light theme rather than the Tailwind class
    /// overrides: white, `#ededed` border, 8px radius, 16px padding,
    /// `0 4px 12px rgba(0,0,0,.1)` shadow, 13px/500 title, and a 24px
    /// `#171717` action button, 32px from the window edges.
    pub(super) fn render_toast_host(
        &self,
        _window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let toast = current_toast(&self.provider_settings, self.auth, &self.dismissed_toasts)?;
        let theme = self.theme;
        let text = theme.toast_text;
        // sonner's collapsed stack: an older toast sits 14px behind the
        // newest one at 95% width and only its top edge shows.
        let behind = !self.pending_deletions.is_empty() || self.settings_alert().is_some();
        let bottom = if behind { px(32.0 + 14.0) } else { px(32.0) };
        let width = if behind { px(300.0 * 0.95) } else { px(300.0) };
        let right = if behind {
            px(32.0 + 300.0 * 0.025)
        } else {
            px(32.0)
        };

        Some(
            div()
                .id("toast-host")
                .absolute()
                .right(right)
                .bottom(bottom)
                .w(width)
                .flex()
                .items_center()
                .gap(px(6.0))
                .p(px(16.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(theme.toast_border)
                .bg(theme.toast_background)
                .shadow(vec![BoxShadow {
                    color: hsla(0.0, 0.0, 0.0, 0.1),
                    offset: point(px(0.0), px(4.0)),
                    blur_radius: px(12.0),
                    spread_radius: px(0.0),
                }])
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .when_some(toast.dismissal_id, |host, dismissal_id| {
                    host.child(
                        close_button(toast.id, theme.toast_background, theme.toast_border, text)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.dismiss_toast(dismissal_id, cx);
                            })),
                    )
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(13.0))
                        .line_height(px(19.0))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(text)
                        .child(toast.description),
                )
                .when_some(toast.action, |host, (label, action)| {
                    host.child(
                        div()
                            .id(SharedString::from(format!("toast-action-{}", toast.id)))
                            .ml_auto()
                            .h(px(24.0))
                            .px_2()
                            .flex()
                            .items_center()
                            .rounded(px(4.0))
                            .bg(text)
                            .text_color(theme.toast_background)
                            .text_size(px(12.0))
                            .line_height(px(24.0))
                            .cursor_pointer()
                            .hover(move |style| style.bg(alpha(text, 0.9)))
                            .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| {
                                window.dispatch_action(action.boxed_clone(), cx);
                            }))
                            .child(label),
                    )
                })
                .into_any_element(),
        )
    }

    /// `SettingsAlertToast` on the AI pages: the `stt-settings-alert` /
    /// `llm-settings-alert` warning while no model is configured.
    pub(super) fn settings_alert(&self) -> Option<SharedString> {
        // `isConfigured`: the visible selection — a configured (available,
        // verified) provider — with a model.
        let selection_configured = |kind: super::ai_settings::ProviderKind,
                                    provider: Option<&str>,
                                    model: Option<&str>| {
            provider.is_some_and(|id| {
                kind.providers().iter().any(|provider| {
                    provider.id == id && self.ai_provider_configured(kind, provider)
                })
            }) && model.is_some_and(|model| !model.is_empty())
        };
        match self.settings_tab? {
            super::settings::SettingsTab::Transcription => {
                let configured = selection_configured(
                    super::ai_settings::ProviderKind::Stt,
                    self.provider_settings.stt_provider.as_deref(),
                    self.provider_settings.stt_model.as_deref(),
                );
                if !configured {
                    return Some("Choose a transcription model to start listening.".into());
                }
                match self.stt_health_status() {
                    Some(super::stt_selection::SttHealth::Error(message)) => Some(message.into()),
                    _ => None,
                }
            }
            super::settings::SettingsTab::Intelligence => {
                let configured = selection_configured(
                    super::ai_settings::ProviderKind::Llm,
                    self.provider_settings.llm_provider.as_deref(),
                    self.provider_settings.llm_model.as_deref(),
                );
                if !configured {
                    return Some("Choose a language model for summaries and chat.".into());
                }
                // `hasError`: the connection probe's message replaces the hint.
                match self.llm_health_status() {
                    Some(crate::ai_health::Health::Error(message)) => Some(message.clone().into()),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// `sonnerToast.success` / `sonnerToast.error` from the settings pages:
    /// shown for `TOAST_DURATIONS` (3s / 5s), newest replacing the previous.
    pub(crate) fn flash(
        &mut self,
        variant: FlashVariant,
        message: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.flash_toast(variant, message.into(), None, cx);
    }

    /// `sonnerToast.<variant>(message, { description })`.
    pub(crate) fn flash_with_description(
        &mut self,
        variant: FlashVariant,
        message: impl Into<SharedString>,
        description: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.flash_toast(variant, message.into(), Some(description.into()), cx);
    }

    /// `sonnerToast.<variant>(message, { action: { label, onClick } })`.
    pub(crate) fn flash_with_action(
        &mut self,
        variant: FlashVariant,
        message: impl Into<SharedString>,
        action: (&'static str, Box<dyn gpui::Action>),
        cx: &mut Context<Self>,
    ) {
        self.flash_toast_with(variant, message.into(), None, Some(action), cx);
    }

    /// sonner's close button (the `Toaster` sets `closeButton`), or a
    /// `persistent` registry toast's `onDismiss`.
    pub(crate) fn dismiss_flash(&mut self, cx: &mut Context<Self>) {
        if self.flash.take().is_some() {
            cx.notify();
        }
    }

    /// `sonnerToast.dismiss(id)`: only the toast with that id goes away.
    pub(crate) fn dismiss_flash_with_id(&mut self, id: &str, cx: &mut Context<Self>) {
        if self
            .flash
            .as_ref()
            .is_some_and(|flash| flash.id == Some(id))
            && self.flash.take().is_some()
        {
            cx.notify();
        }
    }

    /// `sonnerToast.<variant>(message, { id, duration: Infinity, description })`:
    /// stays until dismissed by id or the close button.
    pub(crate) fn flash_persistent(
        &mut self,
        variant: FlashVariant,
        id: &'static str,
        message: impl Into<SharedString>,
        description: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        let generation = self.flash.as_ref().map_or(0, |flash| flash.generation + 1);
        self.flash = Some(FlashToast {
            variant,
            message: message.into(),
            description: Some(description.into()),
            action: None,
            id: Some(id),
            generation,
        });
        cx.notify();
    }

    /// `dismissToast(dismissalId)`: remembered in `store.json` like
    /// `setDismissedToasts`, so neither shell shows the promotion again.
    pub(crate) fn dismiss_toast(&mut self, dismissal_id: &str, cx: &mut Context<Self>) {
        if self.dismissed_toasts.iter().any(|id| id == dismissal_id) {
            return;
        }
        self.dismissed_toasts.push(dismissal_id.to_string());
        if let Err(error) = self.store_file.set_dismissed_toasts(&self.dismissed_toasts) {
            tracing::warn!(%error, "failed to save the dismissed toasts");
        }
        cx.notify();
    }

    fn flash_toast(
        &mut self,
        variant: FlashVariant,
        message: SharedString,
        description: Option<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.flash_toast_with(variant, message, description, None, cx);
    }

    /// `TOAST_DURATIONS`: success 3s, error 5s, warning 6s.
    fn flash_toast_with(
        &mut self,
        variant: FlashVariant,
        message: SharedString,
        description: Option<SharedString>,
        action: Option<(&'static str, Box<dyn gpui::Action>)>,
        cx: &mut Context<Self>,
    ) {
        let generation = self.flash.as_ref().map_or(0, |flash| flash.generation + 1);
        self.flash = Some(FlashToast {
            variant,
            message,
            description,
            action,
            id: None,
            generation,
        });
        cx.notify();
        let duration = match variant {
            FlashVariant::Success => std::time::Duration::from_secs(3),
            FlashVariant::Error => std::time::Duration::from_secs(5),
            FlashVariant::Warning => std::time::Duration::from_secs(6),
            FlashVariant::Info => std::time::Duration::from_secs(4),
        };
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(duration).await;
            this.update(cx, |this, cx| {
                if this
                    .flash
                    .as_ref()
                    .is_some_and(|flash| flash.generation == generation)
                {
                    this.flash = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// The flash toast in front of everything else, with sonner's richColors
    /// success / error palettes.
    pub(super) fn render_flash_toast(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let flash = self.flash.as_ref()?;
        let (background, border, text, glyph) = match (flash.variant, self.theme.dark) {
            (FlashVariant::Success, false) => (
                gpui::rgb(0xecfdf3),
                gpui::rgb(0xd3fde5),
                gpui::rgb(0x008a2e),
                "check-circle",
            ),
            (FlashVariant::Success, true) => (
                gpui::rgb(0x001f0f),
                gpui::rgb(0x003d1c),
                gpui::rgb(0x59f3a6),
                "check-circle",
            ),
            (FlashVariant::Error, false) => (
                gpui::rgb(0xfff0f0),
                gpui::rgb(0xffe0e1),
                gpui::rgb(0xe60000),
                "warning-circle",
            ),
            (FlashVariant::Error, true) => (
                gpui::rgb(0x2d0607),
                gpui::rgb(0x4d0408),
                gpui::rgb(0xff9ea1),
                "warning-circle",
            ),
            (FlashVariant::Warning, false) => (
                gpui::rgb(0xfffcf0),
                gpui::rgb(0xfdf5d3),
                gpui::rgb(0xdc7609),
                "alert-triangle",
            ),
            (FlashVariant::Warning, true) => (
                gpui::rgb(0x1d1f00),
                gpui::rgb(0x3d3d00),
                gpui::rgb(0xf3cf58),
                "alert-triangle",
            ),
            // sonner's `--info-bg` / `--info-border` / `--info-text`.
            (FlashVariant::Info, false) => (
                gpui::rgb(0xf0f8ff),
                gpui::rgb(0xd3e3fd),
                gpui::rgb(0x0973dc),
                "info-circle",
            ),
            (FlashVariant::Info, true) => (
                gpui::rgb(0x000d1f),
                gpui::rgb(0x18283e),
                gpui::rgb(0x589cf3),
                "info-circle",
            ),
        };
        Some(
            div()
                .id("flash-toast")
                .absolute()
                .right(px(32.0))
                .bottom(px(32.0))
                .w(px(300.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .p(px(16.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(border)
                .bg(background)
                .shadow(vec![BoxShadow {
                    color: hsla(0.0, 0.0, 0.0, 0.1),
                    offset: point(px(0.0), px(4.0)),
                    blur_radius: px(12.0),
                    spread_radius: px(0.0),
                }])
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    close_button("flash", background, border, text).on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| this.dismiss_flash(cx)),
                    ),
                )
                .child(
                    // `[data-icon]`: a 16px box holding the Toaster's 20px
                    // Hugeicons glyph (`icons={{ success: <CheckCircle size={20} /> ... }}`).
                    div()
                        .flex()
                        .size(px(16.0))
                        .flex_shrink_0()
                        .items_center()
                        .ml(px(-3.0))
                        .mr(px(4.0))
                        .child(crate::ui::icon(glyph, px(20.0), text)),
                )
                .child(
                    // `[data-content]`: the `font-weight: 500; line-height: 1.5`
                    // title, then the `font-weight: 400; line-height: 1.4`
                    // description 2px below.
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .flex_1()
                        .min_w_0()
                        .text_size(px(13.0))
                        .text_color(text)
                        .child(
                            div()
                                .line_height(px(19.0))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child(flash.message.clone()),
                        )
                        .when_some(flash.description.clone(), |content, description| {
                            content.child(div().line_height(px(18.0)).child(description))
                        }),
                )
                // `[data-button]`: `--normal-text` on `--normal-bg` (`#171717`
                // on white; `#fcfcfc` on black in dark), 12px, 24px tall, `0 8px`.
                .when_some(flash.action.as_ref(), |toast, (label, action)| {
                    let action = action.boxed_clone();
                    toast.child(
                        div()
                            .id("flash-toast-action")
                            .ml_auto()
                            .h(px(24.0))
                            .px_2()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .rounded(px(4.0))
                            .bg(self.theme.toast_text)
                            .text_color(self.theme.toast_background)
                            .text_size(px(12.0))
                            .line_height(px(24.0))
                            .font_weight(gpui::FontWeight::NORMAL)
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                this.dismiss_flash(cx);
                                window.dispatch_action(action.boxed_clone(), cx);
                            }))
                            .child(*label),
                    )
                })
                .into_any_element(),
        )
    }

    /// `sonnerToast.warning` with `richColors`: sonner's `--warning-bg` /
    /// `--warning-border` / `--warning-text` (`#fffcf0` on a `#fdf5d3` border
    /// in `#dc7609`, measured), the 16px triangle in the icon slot (`-3px` /
    /// `4px` margins), `Infinity` duration, not dismissible (`condition-bound`).
    pub(super) fn render_settings_alert_toast(&self) -> Option<AnyElement> {
        let description = self.settings_alert()?;
        let (background, border, text) = if self.theme.dark {
            (
                gpui::rgb(0x2d2306),
                gpui::rgb(0x5c4206),
                gpui::rgb(0xfef3c7),
            )
        } else {
            (
                gpui::rgb(0xfffcf0),
                gpui::rgb(0xfdf5d3),
                gpui::rgb(0xdc7609),
            )
        };
        Some(
            div()
                .id("settings-alert-toast")
                .absolute()
                .right(px(32.0))
                .bottom(px(32.0))
                .w(px(300.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .p(px(16.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(border)
                .bg(background)
                .shadow(vec![BoxShadow {
                    color: hsla(0.0, 0.0, 0.0, 0.1),
                    offset: point(px(0.0), px(4.0)),
                    blur_radius: px(12.0),
                    spread_radius: px(0.0),
                }])
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .flex()
                        .size(px(16.0))
                        .flex_shrink_0()
                        .items_center()
                        .ml(px(-3.0))
                        .mr(px(4.0))
                        .child(crate::ui::icon("alert-triangle", px(20.0), text)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(13.0))
                        .line_height(px(19.0))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(text)
                        .child(description),
                )
                .into_any_element(),
        )
    }
}

/// sonner's `[data-close-button]`: 20px round, at the toast's top-left corner
/// translated `-35%`, the toast's own background and border (rich colours
/// included) and its text colour for the 12px `X`.
fn close_button(
    id: &'static str,
    background: gpui::Rgba,
    border: gpui::Rgba,
    text: gpui::Rgba,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(format!("toast-close-{id}")))
        .absolute()
        .left(px(-7.0))
        .top(px(-7.0))
        .flex()
        .size(px(20.0))
        .items_center()
        .justify_center()
        .rounded_full()
        .border_1()
        .border_color(border)
        .bg(background)
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(crate::ui::icon("x", px(12.0), text))
}
