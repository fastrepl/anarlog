use anlg_calendar_interface::{
    CalendarEvent, CalendarListItem, CalendarProviderType, CreateEventInput, EventFilter,
};
use tauri::Manager;
use tauri_plugin_auth::AuthPluginExt;
#[cfg(target_os = "macos")]
use tauri_plugin_permissions::PermissionsPluginExt;

use crate::error::Error;

#[tauri::command]
#[specta::specta]
pub fn available_providers() -> Vec<CalendarProviderType> {
    anlg_calendar::available_providers()
}

#[tauri::command]
#[specta::specta]
pub async fn is_provider_enabled<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    provider: CalendarProviderType,
) -> Result<bool, Error> {
    let config = app.state::<crate::PluginConfig>();
    let token = match provider {
        CalendarProviderType::Apple => None,
        _ => access_token(&app)?,
    };
    let apple = is_apple_authorized(&app).await?;
    anlg_calendar::is_provider_enabled(&config.api_base_url, token.as_deref(), apple, provider)
        .await
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub async fn list_connection_ids<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<anlg_calendar::ProviderConnectionIds>, Error> {
    let config = app.state::<crate::PluginConfig>();
    let token = access_token(&app)?;
    let apple = is_apple_authorized(&app).await?;
    anlg_calendar::list_connection_ids(&config.api_base_url, token.as_deref(), apple)
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
        CalendarProviderType::Apple => String::new(),
        _ => require_access_token(&app)?,
    };
    anlg_calendar::list_calendars(&config.api_base_url, &token, provider, &connection_id)
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
        CalendarProviderType::Apple => String::new(),
        _ => require_access_token(&app)?,
    };
    anlg_calendar::list_events(
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
    anlg_calendar::open_calendar(provider).map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn create_event<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
    provider: CalendarProviderType,
    input: CreateEventInput,
) -> Result<String, Error> {
    anlg_calendar::create_event(provider, input).map_err(Into::into)
}

macro_rules! contacts_command {
    ($name:ident, $request:ty, $call:expr) => {
        #[tauri::command]
        #[specta::specta]
        pub async fn $name<R: tauri::Runtime>(
            app: tauri::AppHandle<R>,
            request: $request,
        ) -> Result<(), String> {
            let runtime = app
                .try_state::<tauri_plugin_db::ManagedState>()
                .map(|state| state.inner().clone())
                .ok_or_else(|| "database is not ready yet".to_string())?;
            let _guard = runtime.synced_write_guard().await;
            $call(runtime.pool(), request).await
        }
    };
}

contacts_command!(
    create_human,
    crate::contacts::CreateHumanRequest,
    crate::contacts::create_human
);
contacts_command!(
    create_organization,
    crate::contacts::CreateOrganizationRequest,
    crate::contacts::create_organization
);
contacts_command!(
    save_personal_contact,
    crate::contacts::SavePersonalContactRequest,
    crate::contacts::save_personal_contact
);
contacts_command!(
    update_human,
    crate::contacts::UpdateHumanRequest,
    crate::contacts::update_human
);
contacts_command!(
    update_organization,
    crate::contacts::UpdateOrganizationRequest,
    crate::contacts::update_organization
);
contacts_command!(
    soft_delete_contact,
    crate::contacts::SoftDeleteContactRequest,
    crate::contacts::soft_delete_contact
);
contacts_command!(
    update_contact_avatar,
    crate::contacts::UpdateContactAvatarRequest,
    crate::contacts::update_contact_avatar
);
contacts_command!(
    update_human_contact_summary,
    crate::contacts::UpdateHumanContactSummaryRequest,
    crate::contacts::update_human_contact_summary
);
contacts_command!(
    toggle_contact_pin,
    crate::contacts::ToggleContactPinRequest,
    crate::contacts::toggle_contact_pin
);
contacts_command!(
    reorder_pinned_contacts,
    crate::contacts::ReorderPinnedContactsRequest,
    crate::contacts::reorder_pinned_contacts
);
contacts_command!(
    merge_humans,
    crate::contacts::MergeHumansRequest,
    crate::contacts::merge_humans
);
contacts_command!(
    apply_contact_enhancement,
    crate::contacts::ApplyContactEnhancementRequest,
    crate::contacts::apply_contact_enhancement
);

fn access_token<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<Option<String>, Error> {
    app.access_token()
        .map(|token| token.filter(|token| !token.is_empty()))
        .map_err(|error| Error::Auth(error.to_string()))
}

fn require_access_token<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<String, Error> {
    let token = access_token(app)?;
    match token {
        Some(t) if !t.is_empty() => Ok(t),
        _ => Err(anlg_calendar::Error::NotAuthenticated.into()),
    }
}

async fn is_apple_authorized<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<bool, Error> {
    #[cfg(target_os = "macos")]
    {
        let status = app
            .permissions()
            .check(tauri_plugin_permissions::Permission::Calendar)
            .await
            .map_err(|e| anlg_calendar::Error::Api(e.to_string()))?;
        Ok(matches!(
            status,
            tauri_plugin_permissions::PermissionStatus::Authorized
        ))
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Ok(false)
    }
}

#[tauri::command]
#[specta::specta]
pub async fn apply_calendar_inventory<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: crate::storage::ApplyCalendarInventoryRequest,
) -> Result<(), String> {
    let runtime = app
        .try_state::<tauri_plugin_db::ManagedState>()
        .map(|state| state.inner().clone())
        .ok_or_else(|| "database is not ready yet".to_string())?;
    let _guard = runtime.synced_write_guard().await;
    crate::storage::apply_calendar_inventory(runtime.pool(), request).await
}

#[tauri::command]
#[specta::specta]
pub async fn tombstone_calendar_connection<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: crate::storage::TombstoneCalendarConnectionRequest,
) -> Result<(), String> {
    let runtime = app
        .try_state::<tauri_plugin_db::ManagedState>()
        .map(|state| state.inner().clone())
        .ok_or_else(|| "database is not ready yet".to_string())?;
    let _guard = runtime.synced_write_guard().await;
    crate::storage::tombstone_calendar_connection(runtime.pool(), request).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_calendar_enabled<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: crate::storage::SetCalendarEnabledRequest,
) -> Result<(), String> {
    let runtime = app
        .try_state::<tauri_plugin_db::ManagedState>()
        .map(|state| state.inner().clone())
        .ok_or_else(|| "database is not ready yet".to_string())?;
    let _guard = runtime.synced_write_guard().await;
    crate::storage::set_calendar_enabled(runtime.pool(), request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sync_calendar_connection_events<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: crate::storage::SyncCalendarConnectionEventsRequest,
) -> Result<(), String> {
    let runtime = app
        .try_state::<tauri_plugin_db::ManagedState>()
        .map(|state| state.inner().clone())
        .ok_or_else(|| "database is not ready yet".to_string())?;
    let _guard = runtime.synced_write_guard().await;
    crate::storage::sync_calendar_connection_events(runtime.pool(), request).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_ignored_calendar_item<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: crate::storage::UpdateIgnoredCalendarItemRequest,
) -> Result<(), String> {
    let runtime = app
        .try_state::<tauri_plugin_db::ManagedState>()
        .map(|state| state.inner().clone())
        .ok_or_else(|| "database is not ready yet".to_string())?;
    let _guard = runtime.synced_write_guard().await;
    crate::storage::update_ignored_calendar_item(runtime.pool(), request).await
}
