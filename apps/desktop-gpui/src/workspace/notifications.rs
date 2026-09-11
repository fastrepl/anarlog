//! `showBatchCompletedNotification` / `showSummaryReadyNotification` and
//! their gates (`isAppWindowInactive`, `shouldShowNotification`), plus the
//! `openNew` a notification click performs.

use gpui::{Context, Window};

use super::Workspace;

impl Workspace {
    /// `isAppWindowInactive`, tracked from the window's activation events.
    pub(super) fn observe_window_activity(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.window_active = window.is_window_active();
        cx.observe_window_activation(window, |this, window, _| {
            this.window_active = window.is_window_active();
            // Focusing the window answers the attention request.
            if this.window_active && this.attention_requested {
                this.attention_requested = false;
                #[cfg(target_os = "linux")]
                crate::x11::set_urgent(
                    this.viewport_width as u32,
                    this.viewport_height as u32,
                    false,
                );
            }
        })
        .detach();
    }

    /// `shouldShowNotification(settingKey)`: notifications on, and the kind on.
    fn should_show_notification(&self, key: &str, path: &[&str]) -> bool {
        let settings = &self.provider_settings;
        !settings.bool_setting(
            "notification_disabled",
            &["notification", "disabled"],
            false,
        ) && settings.bool_setting(key, path, true)
    }

    /// `requestAppAttention`: with notifications on, `notification_bounce`
    /// and `show_app_in_dock` on, and the window inactive, ask for the user's
    /// attention — `requestUserAttention(Informational)`, which GTK turns
    /// into the `WM_HINTS` urgency flag (the Dock bounce on macOS is not
    /// available through gpui).
    pub(crate) fn request_app_attention(&mut self) {
        let settings = &self.provider_settings;
        if settings.bool_setting(
            "notification_disabled",
            &["notification", "disabled"],
            false,
        ) || !settings.bool_setting("notification_bounce", &["notification", "bounce"], true)
            || !settings.bool_setting("show_app_in_dock", &["general", "show_app_in_dock"], true)
            || self.window_active
        {
            return;
        }
        self.attention_requested = true;
        #[cfg(target_os = "linux")]
        crate::x11::set_urgent(
            self.viewport_width as u32,
            self.viewport_height as u32,
            true,
        );
        #[cfg(not(target_os = "linux"))]
        tracing::debug!("app attention request is not available on this platform");
    }

    /// `showBatchCompletedNotification(sessionId)`: only while the window is
    /// inactive and `notification_transcription_complete` is on.
    pub(crate) fn notify_batch_completed(&self, session_id: &str) {
        if self.window_active
            || !self.should_show_notification(
                "notification_transcription_complete",
                &["notification", "transcription_complete"],
            )
        {
            return;
        }
        crate::notifications::show(&crate::notifications::batch_completed(session_id));
    }

    /// `showSummaryReadyNotification(sessionId, title)`.
    pub(crate) fn notify_summary_ready(&self, session_id: &str, title: Option<&str>) {
        if self.window_active
            || !self.should_show_notification(
                "notification_summary_complete",
                &["notification", "summary_complete"],
            )
        {
            return;
        }
        crate::notifications::show(&crate::notifications::summary_ready(session_id, title));
    }

    /// A notification's `Open Anarlog`: `openNew({ type: "sessions", id })`.
    pub(crate) fn open_session_from_notification(
        &mut self,
        session_id: String,
        cx: &mut Context<Self>,
    ) {
        self.open_new(session_id, cx);
    }
}
