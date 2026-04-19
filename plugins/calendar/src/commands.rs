use std::sync::Arc;

use hypr_calendar_interface::{
    CalendarEvent, CalendarListItem, CalendarProviderType, CreateEventInput, EventFilter,
};
use tauri::Manager;

use crate::auth::{access_token, is_apple_authorized, require_access_token};
use crate::error::Error;
use crate::sync::CalendarSyncStore;

#[tauri::command]
#[specta::specta]
pub fn available_providers() -> Vec<CalendarProviderType> {
    hypr_calendar::available_providers()
}

#[tauri::command]
#[specta::specta]
pub async fn is_provider_enabled<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    provider: CalendarProviderType,
) -> Result<bool, Error> {
    let config = app.state::<crate::PluginConfig>();
    let token = access_token(&app);
    let apple = is_apple_authorized(&app).await?;
    hypr_calendar::is_provider_enabled(&config.api_base_url, token.as_deref(), apple, provider)
        .await
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub async fn list_connection_ids<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<hypr_calendar::ProviderConnectionIds>, Error> {
    let config = app.state::<crate::PluginConfig>();
    let token = access_token(&app);
    let apple = is_apple_authorized(&app).await?;
    hypr_calendar::list_connection_ids(&config.api_base_url, token.as_deref(), apple)
        .await
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub async fn list_calendars<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    provider: CalendarProviderType,
    connection_id: String,
) -> Result<Vec<CalendarListItem>, Error> {
    let config = app.state::<crate::PluginConfig>();
    let token = match provider {
        CalendarProviderType::Apple => access_token(&app).unwrap_or_default(),
        _ => require_access_token(&app)?,
    };
    hypr_calendar::list_calendars(&config.api_base_url, &token, provider, &connection_id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub async fn list_events<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    provider: CalendarProviderType,
    connection_id: String,
    filter: EventFilter,
) -> Result<Vec<CalendarEvent>, Error> {
    let config = app.state::<crate::PluginConfig>();
    let token = match provider {
        CalendarProviderType::Apple => access_token(&app).unwrap_or_default(),
        _ => require_access_token(&app)?,
    };
    hypr_calendar::list_events(
        &config.api_base_url,
        &token,
        provider,
        &connection_id,
        filter,
    )
    .await
    .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn open_calendar<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
    provider: CalendarProviderType,
) -> Result<(), Error> {
    hypr_calendar::open_calendar(provider).map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn create_event<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
    provider: CalendarProviderType,
    input: CreateEventInput,
) -> Result<String, Error> {
    hypr_calendar::create_event(provider, input).map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn parse_meeting_link(text: String) -> Option<String> {
    hypr_calendar::parse_meeting_link(&text)
}

#[tauri::command]
#[specta::specta]
pub fn request_calendar_sync<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> bool {
    app.state::<hypr_calendar_sync::CalendarSyncHandle>()
        .request_sync()
        .is_ok()
}

#[tauri::command]
#[specta::specta]
pub fn get_calendar_sync_status<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> hypr_calendar_sync::SyncStatus {
    app.state::<hypr_calendar_sync::CalendarSyncHandle>()
        .status()
}

#[tauri::command]
#[specta::specta]
pub async fn set_calendar_enabled<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    calendar_id: String,
    enabled: bool,
) -> Result<(), Error> {
    let store = app.state::<Arc<dyn CalendarSyncStore>>().inner().clone();

    store
        .mutate(Box::new(move |snap| {
            match snap.calendars.get_mut(&calendar_id) {
                Some(cal) if cal.enabled != enabled => {
                    cal.enabled = enabled;
                    true
                }
                _ => false,
            }
        }))
        .await
        .map_err(|error| Error::Store(error.to_string()))?;

    Ok(())
}
