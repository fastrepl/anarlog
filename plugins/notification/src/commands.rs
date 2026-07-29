use crate::NotificationPluginExt;
use tauri_plugin_analytics::{AnalyticsPayload, AnalyticsPluginExt};

#[tauri::command]
#[specta::specta]
pub(crate) async fn show_notification<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    v: anlg_notification::Notification,
) -> Result<(), String> {
    let source = match &v.source {
        Some(anlg_notification::NotificationSource::CalendarEvent { .. }) => "calendar_event",
        Some(anlg_notification::NotificationSource::Session { .. }) => "session",
        Some(anlg_notification::NotificationSource::MicDetected { .. }) => "mic_detected",
        None => "unknown",
    };
    let is_persistent = v.is_persistent();
    let has_options = v
        .options
        .as_ref()
        .is_some_and(|options| !options.is_empty());
    app.notification().show(v).map_err(|e| e.to_string())?;
    app.analytics().event_fire_and_forget(
        AnalyticsPayload::builder("notification_shown")
            .with("source_type", source)
            .with("is_persistent", is_persistent)
            .with("has_options", has_options)
            .build(),
    );
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn clear_notifications<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<(), String> {
    app.notification().clear().map_err(|e| e.to_string())
}
