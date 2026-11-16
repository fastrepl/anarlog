use std::{sync::Arc, time::Duration};

use gpui::{Context, FocusHandle, FontWeight, Window, div, prelude::*, px, rgb, svg};

use super::ApplicationView;
use crate::{
    contracts::ProductRoute,
    product::settings::{self, Snapshot},
    ui::theme::{system_font, theme},
};

pub(super) struct Notifications {
    preferences: Option<Arc<Snapshot>>,
    add_focus: FocusHandle,
}

impl Notifications {
    pub(super) fn new(cx: &mut Context<ApplicationView>) -> Self {
        Self {
            preferences: None,
            add_focus: cx.focus_handle().tab_stop(true),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProviderNotice {
    Transcription,
    Intelligence,
}

impl ProviderNotice {
    fn route(self) -> ProductRoute {
        match self {
            Self::Transcription => ProductRoute::Transcription,
            Self::Intelligence => ProductRoute::Intelligence,
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::Transcription => "Transcription provider needed",
            Self::Intelligence => "Language model needed",
        }
    }
}

fn configured_stt(provider: &str, model: &str) -> bool {
    if provider.is_empty() || model.is_empty() {
        return false;
    }
    match provider {
        "anarlog" => {
            model == "cloud"
                || model.starts_with("soniqo-")
                || model == "apple-speech"
                || model.starts_with("am-")
                || model.starts_with("Quantized")
        }
        "soniqo" => model.starts_with("soniqo-"),
        "apple_speech" => model == "apple-speech",
        "local_file" => model == "local-file",
        _ => true,
    }
}

fn provider_notice(
    stt: (&str, &str),
    llm: (&str, &str),
    signed_in: bool,
    section: Option<&str>,
) -> Option<ProviderNotice> {
    let usable_stt = configured_stt(stt.0, stt.1) && (signed_in || stt != ("anarlog", "cloud"));
    let usable_llm = !llm.0.is_empty() && !llm.1.is_empty() && (signed_in || llm.0 != "anarlog");
    if !usable_stt && section != Some("transcription") {
        Some(ProviderNotice::Transcription)
    } else if usable_stt && !usable_llm && section != Some("intelligence") {
        Some(ProviderNotice::Intelligence)
    } else {
        None
    }
}

fn choice<'a>(snapshot: &'a Snapshot, key: &str) -> &'a str {
    snapshot
        .get(key)
        .and_then(|setting| setting.value.as_str())
        .unwrap_or_default()
}

impl Notifications {
    /// Shipping's `floating_bar_enabled` setting, on by default.
    pub(super) fn floating_bar_enabled(&self) -> bool {
        self.preferences
            .as_ref()
            .and_then(|snapshot| snapshot.get("floating_bar_enabled"))
            .and_then(|setting| setting.value.as_bool())
            .unwrap_or(true)
    }
}

impl ApplicationView {
    pub(super) fn watch_preferences(&mut self, cx: &mut Context<Self>) {
        let reply = settings::watch(&self.runtime);
        self.preferences_task = Some(cx.spawn(async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            let mut watch = match result {
                Ok(watch) => watch,
                Err(error) => {
                    let _ = this.update(cx, |this, cx| this.status(&error.to_string(), cx));
                    return;
                }
            };
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            loop {
                if let Some(error) = watch
                    .terminal_error()
                    .or_else(|| watch.errors.try_recv().ok())
                {
                    let _ = this.update(cx, |this, cx| this.status(&error.to_string(), cx));
                    break;
                }
                let rows = watch.snapshots.borrow_and_update().rows.clone();
                let snapshot = cx
                    .background_executor()
                    .spawn(async move { settings::decode(&rows).map(Arc::new) })
                    .await;
                if this
                    .update(cx, |this, cx| {
                        match snapshot {
                            Ok(snapshot) => {
                                let use_24_hour_time = snapshot
                                    .get("use_24_hour_time")
                                    .and_then(|setting| setting.value.as_bool())
                                    .unwrap_or(false);
                                let timezone = choice(&snapshot, "timezone").parse().ok();
                                let show_folder = snapshot
                                    .get("sidebar_show_folder")
                                    .and_then(|setting| setting.value.as_bool())
                                    .unwrap_or(true);
                                let show_tags = snapshot
                                    .get("sidebar_show_tags")
                                    .and_then(|setting| setting.value.as_bool())
                                    .unwrap_or(false);
                                let week_start = crate::workspace::calendar::week_start(
                                    choice(&snapshot, "week_start"),
                                    sys_locale::get_locale().as_deref(),
                                );
                                this.workspace.update(cx, |workspace, cx| {
                                    workspace.set_timeline_clock(use_24_hour_time, timezone, cx);
                                    workspace.set_timeline_metadata(show_folder, show_tags, cx);
                                    workspace.set_calendar_week_start(week_start, cx);
                                });
                                this.notifications.preferences = Some(snapshot);
                            }
                            Err(error) => {
                                this.notifications.preferences = None;
                                this.status(&error.to_string(), cx);
                            }
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
                if watch.snapshots.changed().await.is_err() {
                    break;
                }
            }
            let _ = watch.unsubscribe().await;
        }));
    }

    pub(super) fn provider_notification(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        if self.closing || self.writers_paused || !self.status.is_empty() {
            return None;
        }
        let snapshot = self.notifications.preferences.as_ref()?;
        let product = self.product.as_ref().map(|product| product.read(cx));
        if product.is_some_and(|product| matches!(product.route, ProductRoute::Onboarding)) {
            return None;
        }
        let signed_in = self
            .native
            .as_ref()
            .and_then(|native| native.cloud.service.as_ref())
            .is_some_and(|cloud| cloud.scope().account_id.is_some());
        let notice = provider_notice(
            (
                choice(snapshot, "current_stt_provider"),
                choice(snapshot, "current_stt_model"),
            ),
            (
                choice(snapshot, "current_llm_provider"),
                choice(snapshot, "current_llm_model"),
            ),
            signed_in,
            product.map(|product| product.active_section()),
        )?;
        let colors = theme(window);
        // The shipping compact app toast; provider prompts are condition-bound, so there is no
        // dismiss action and nothing to expand.
        Some(
            div()
                .id("provider-notification")
                .absolute()
                .bottom(px(16.))
                .right(px(16.))
                .w(px(320.).min(window.bounds().size.width - px(32.)))
                .rounded(px(14.))
                .border_1()
                .border_color(colors.border.opacity(0.7))
                .bg(colors.popover)
                .font_family(system_font(cx))
                .text_color(colors.popover_foreground)
                .shadow_md()
                .occlude()
                .on_key_down(cx.listener(|_, event: &gpui::KeyDownEvent, window, cx| {
                    if event.keystroke.key == "tab" {
                        if event.keystroke.modifiers.shift {
                            window.focus_prev(cx);
                        } else {
                            window.focus_next(cx);
                        }
                        cx.stop_propagation();
                    }
                }))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .min_h(px(44.))
                        .gap(px(8.))
                        .px(px(10.))
                        .py(px(6.))
                        .child(
                            div()
                                .size(px(32.))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    svg()
                                        .path("InfoIcon.svg")
                                        .size(px(16.))
                                        .text_color(rgb(0x155dfc)),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(px(13.))
                                .line_height(px(20.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(notice.message()),
                        ),
                )
                .child(
                    div().px(px(10.)).pb(px(10.)).flex().child(
                        div()
                            .id("add-provider")
                            .track_focus(&self.notifications.add_focus)
                            .min_h(px(28.))
                            .px(px(10.))
                            .py(px(4.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .bg(colors.primary)
                            .text_color(colors.primary_foreground)
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .cursor_pointer()
                            .hover(|style| style.bg(colors.primary.opacity(0.9)))
                            .focus(|style| style.bg(colors.primary.opacity(0.9)))
                            .child("Add")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_product(notice.route(), window, cx);
                            })),
                    ),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_prompts_follow_configuration_auth_and_active_settings() {
        let missing = ("", "");
        let local = ("anarlog", "soniqo-parakeet-streaming");
        let cloud = ("anarlog", "cloud");
        let byok = ("openai", "gpt-4o");
        assert_eq!(
            provider_notice(missing, missing, false, None),
            Some(ProviderNotice::Transcription)
        );
        assert_eq!(
            provider_notice(missing, missing, false, Some("transcription")),
            None
        );
        assert_eq!(
            provider_notice(local, missing, false, None),
            Some(ProviderNotice::Intelligence)
        );
        assert_eq!(
            provider_notice(local, missing, false, Some("intelligence")),
            None
        );
        assert_eq!(provider_notice(local, byok, false, None), None);
        assert_eq!(
            provider_notice(cloud, byok, false, None),
            Some(ProviderNotice::Transcription)
        );
        assert_eq!(provider_notice(cloud, byok, true, None), None);
        assert_eq!(
            provider_notice(local, ("anarlog", "any"), false, None),
            Some(ProviderNotice::Intelligence)
        );
        for (provider, model) in [
            ("anarlog", "unknown"),
            ("soniqo", "cloud"),
            ("apple_speech", "cloud"),
            ("local_file", "cloud"),
            ("openai", ""),
            ("", "whisper"),
        ] {
            assert!(!configured_stt(provider, model), "{provider}/{model}");
        }
        for (provider, model) in [
            local,
            cloud,
            ("apple_speech", "apple-speech"),
            ("local_file", "local-file"),
            ("deepgram", "nova-3"),
            ("anarlog", "am-small"),
            ("anarlog", "QuantizedTiny"),
        ] {
            assert!(configured_stt(provider, model), "{provider}/{model}");
        }
    }
}
