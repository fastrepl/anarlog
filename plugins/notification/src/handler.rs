use tauri_plugin_analytics::{AnalyticsPayload, AnalyticsPluginExt};
use tauri_plugin_windows::WindowsPluginExt;
use tauri_specta::Event;

use crate::events::NotificationEvent;

pub fn init(app: tauri::AppHandle<tauri::Wry>) {
    {
        let app = app.clone();
        hypr_notification::setup_collapsed_confirm_handler(move |ctx| {
            if let Err(_e) = app.windows().show(tauri_plugin_windows::AppWindow::Main) {}

            let _ = NotificationEvent::Confirm {
                key: ctx.key,
                source: ctx.source,
            }
            .emit(&app);

            app.analytics()
                .event_fire_and_forget(AnalyticsPayload::builder("collapsed_confirm").build());
        });
    }

    {
        let app = app.clone();
        hypr_notification::setup_expanded_accept_handler(move |ctx| {
            if let Err(_e) = app.windows().show(tauri_plugin_windows::AppWindow::Main) {}

            let _ = NotificationEvent::Accept {
                key: ctx.key,
                source: ctx.source,
            }
            .emit(&app);

            app.analytics()
                .event_fire_and_forget(AnalyticsPayload::builder("expanded_accept").build());
        });
    }

    {
        let app = app.clone();
        hypr_notification::setup_dismiss_handler(move |ctx| {
            let _ = NotificationEvent::Dismiss {
                key: ctx.key,
                source: ctx.source,
            }
            .emit(&app);

            app.analytics()
                .event_fire_and_forget(AnalyticsPayload::builder("dismiss").build());
        });
    }

    {
        let app = app.clone();
        hypr_notification::setup_collapsed_timeout_handler(move |ctx| {
            let _ = NotificationEvent::Timeout {
                key: ctx.key,
                source: ctx.source,
            }
            .emit(&app);

            app.analytics()
                .event_fire_and_forget(AnalyticsPayload::builder("collapsed_timeout").build());
        });
    }

    {
        let app = app.clone();
        hypr_notification::setup_option_selected_handler(move |ctx, selected_index| {
            if let Err(_e) = app.windows().show(tauri_plugin_windows::AppWindow::Main) {}

            let _ = NotificationEvent::OptionSelected {
                key: ctx.key,
                source: ctx.source,
                selected_index,
            }
            .emit(&app);

            app.analytics()
                .event_fire_and_forget(AnalyticsPayload::builder("option_selected").build());
        });
    }
}
