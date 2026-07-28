use tauri_plugin_analytics::{AnalyticsPayload, AnalyticsPluginExt};
use tauri_plugin_windows::WindowsPluginExt;
use tauri_specta::Event;

use crate::events::NotificationEvent;

fn source_kind(source: &Option<hypr_notification::NotificationSource>) -> &'static str {
    match source {
        Some(hypr_notification::NotificationSource::CalendarEvent { .. }) => "calendar_event",
        Some(hypr_notification::NotificationSource::Session { .. }) => "session",
        Some(hypr_notification::NotificationSource::MicDetected { .. }) => "mic_detected",
        None => "unknown",
    }
}

fn track_notification_event(
    app: &tauri::AppHandle<tauri::Wry>,
    event: &'static str,
    source: &'static str,
    action: Option<&'static str>,
) {
    let mut payload = AnalyticsPayload::builder(event).with("source_type", source);
    if let Some(action) = action {
        payload = payload.with("action", action);
    }
    app.analytics().event_fire_and_forget(payload.build());
}

pub fn init(app: tauri::AppHandle<tauri::Wry>) {
    {
        let app = app.clone();
        hypr_notification::setup_collapsed_confirm_handler(move |ctx| {
            let source = source_kind(&ctx.source);
            if let Err(_e) = app.windows().show(tauri_plugin_windows::AppWindow::Main) {}

            let _ = NotificationEvent::Confirm {
                key: ctx.key,
                source: ctx.source,
            }
            .emit(&app);

            app.analytics()
                .event_fire_and_forget(AnalyticsPayload::builder("collapsed_confirm").build());
            track_notification_event(&app, "notification_actioned", source, Some("confirm"));
        });
    }

    {
        let app = app.clone();
        hypr_notification::setup_expanded_accept_handler(move |ctx| {
            let source = source_kind(&ctx.source);
            if let Err(_e) = app.windows().show(tauri_plugin_windows::AppWindow::Main) {}

            let _ = NotificationEvent::Accept {
                key: ctx.key,
                source: ctx.source,
            }
            .emit(&app);

            app.analytics()
                .event_fire_and_forget(AnalyticsPayload::builder("expanded_accept").build());
            track_notification_event(&app, "notification_actioned", source, Some("accept"));
        });
    }

    {
        let app = app.clone();
        hypr_notification::setup_dismiss_handler(move |ctx| {
            let source = source_kind(&ctx.source);
            let _ = NotificationEvent::Dismiss {
                key: ctx.key,
                source: ctx.source,
            }
            .emit(&app);

            app.analytics()
                .event_fire_and_forget(AnalyticsPayload::builder("dismiss").build());
            track_notification_event(&app, "notification_dismissed", source, None);
        });
    }

    {
        let app = app.clone();
        hypr_notification::setup_collapsed_timeout_handler(move |ctx| {
            let source = source_kind(&ctx.source);
            let _ = NotificationEvent::Timeout {
                key: ctx.key,
                source: ctx.source,
            }
            .emit(&app);

            app.analytics()
                .event_fire_and_forget(AnalyticsPayload::builder("collapsed_timeout").build());
            track_notification_event(&app, "notification_timed_out", source, None);
        });
    }

    {
        let app = app.clone();
        hypr_notification::setup_option_selected_handler(move |ctx, selected_index| {
            let source = source_kind(&ctx.source);
            if let Err(_e) = app.windows().show(tauri_plugin_windows::AppWindow::Main) {}

            let _ = NotificationEvent::OptionSelected {
                key: ctx.key,
                source: ctx.source,
                selected_index,
            }
            .emit(&app);

            app.analytics()
                .event_fire_and_forget(AnalyticsPayload::builder("option_selected").build());
            app.analytics().event_fire_and_forget(
                AnalyticsPayload::builder("notification_actioned")
                    .with("source_type", source)
                    .with("action", "option_selected")
                    .with("option_index", selected_index)
                    .build(),
            );
        });
    }

    {
        let app = app.clone();
        hypr_notification::setup_footer_action_handler(move |ctx| {
            let source = source_kind(&ctx.source);
            let _ = NotificationEvent::FooterAction {
                key: ctx.key,
                source: ctx.source,
            }
            .emit(&app);

            app.analytics()
                .event_fire_and_forget(AnalyticsPayload::builder("footer_action").build());
            track_notification_event(&app, "notification_actioned", source, Some("footer_action"));
        });
    }
}
