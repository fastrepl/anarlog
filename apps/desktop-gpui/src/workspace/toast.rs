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

pub(crate) struct Toast {
    pub id: &'static str,
    pub description: SharedString,
    pub action: Option<(&'static str, Box<dyn gpui::Action>)>,
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
            action: Some(("Sign in", Box::new(actions::OpenSettings))),
        });
    }
    if !has_usable_stt {
        return Some(Toast {
            id: "missing-stt",
            description: "Transcription provider needed".into(),
            action: Some(("Add", Box::new(actions::OpenTranscriptionSettings))),
        });
    }
    if !has_usable_llm {
        return Some(Toast {
            id: "missing-llm",
            description: "Language model needed".into(),
            action: Some(("Add", Box::new(actions::OpenIntelligenceSettings))),
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
        return Some(Toast {
            id: "upgrade-to-pro",
            description: "Pro features available".into(),
            action: Some(("Upgrade", Box::new(actions::OpenSettings))),
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
    pub(super) fn settings_alert(&self) -> Option<&'static str> {
        match self.settings_tab? {
            super::settings::SettingsTab::Transcription => {
                let configured = self.provider_settings.stt_provider.is_some()
                    && self.provider_settings.stt_model.is_some();
                (!configured).then_some("Choose a transcription model to start listening.")
            }
            super::settings::SettingsTab::Intelligence => {
                let configured = self.provider_settings.llm_provider.is_some()
                    && self.provider_settings.llm_model.is_some();
                (!configured).then_some("Choose a language model for summaries and chat.")
            }
            _ => None,
        }
    }

    /// `sonnerToast.warning` with `richColors`: `#fffbeb` on a `#fef3c7`
    /// border in `#92400e`, the 16px triangle in the icon slot (`-3px` /
    /// `4px` margins), `Infinity` duration, not dismissible.
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
                gpui::rgb(0xfffbeb),
                gpui::rgb(0xfef3c7),
                gpui::rgb(0x92400e),
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
                        .flex_shrink_0()
                        .ml(px(-3.0))
                        .mr(px(4.0))
                        .child(crate::ui::icon("alert-triangle", px(16.0), text)),
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
