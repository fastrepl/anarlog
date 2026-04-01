use hypr_apple_todo::types::{CreateReminderInput, Reminder, ReminderFilter, ReminderList};
use hypr_ticket_interface::{CollectionPage, TicketPage};

use tauri::Manager;
use tauri_plugin_auth::AuthPluginExt;

use crate::error::Error;

#[tauri::command]
#[specta::specta]
pub fn authorization_status() -> Result<String, Error> {
    #[cfg(target_os = "macos")]
    {
        let status = hypr_apple_todo::Handle::authorization_status();
        Ok(format!("{:?}", status))
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err(Error::UnsupportedPlatform)
    }
}

#[tauri::command]
#[specta::specta]
pub fn request_full_access() -> Result<bool, Error> {
    #[cfg(target_os = "macos")]
    {
        Ok(hypr_apple_todo::Handle::request_full_access())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err(Error::UnsupportedPlatform)
    }
}

#[tauri::command]
#[specta::specta]
pub fn list_todo_lists() -> Result<Vec<ReminderList>, Error> {
    #[cfg(target_os = "macos")]
    {
        let handle = hypr_apple_todo::Handle;
        handle.list_reminder_lists().map_err(Into::into)
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err(Error::UnsupportedPlatform)
    }
}

#[tauri::command]
#[specta::specta]
pub fn fetch_todos(filter: ReminderFilter) -> Result<Vec<Reminder>, Error> {
    #[cfg(target_os = "macos")]
    {
        let handle = hypr_apple_todo::Handle;
        handle.fetch_reminders(filter).map_err(Into::into)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = filter;
        Err(Error::UnsupportedPlatform)
    }
}

#[tauri::command]
#[specta::specta]
pub fn create_todo(input: CreateReminderInput) -> Result<String, Error> {
    #[cfg(target_os = "macos")]
    {
        let handle = hypr_apple_todo::Handle;
        handle.create_reminder(input).map_err(Into::into)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = input;
        Err(Error::UnsupportedPlatform)
    }
}

#[tauri::command]
#[specta::specta]
pub fn complete_todo(id: String) -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    {
        let handle = hypr_apple_todo::Handle;
        handle.complete_reminder(&id).map_err(Into::into)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = id;
        Err(Error::UnsupportedPlatform)
    }
}

#[tauri::command]
#[specta::specta]
pub fn delete_todo(id: String) -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    {
        let handle = hypr_apple_todo::Handle;
        handle.delete_reminder(&id).map_err(Into::into)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = id;
        Err(Error::UnsupportedPlatform)
    }
}

#[tauri::command]
#[specta::specta]
pub async fn linear_list_teams<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    connection_id: String,
    limit: Option<u32>,
    cursor: Option<String>,
) -> Result<CollectionPage, Error> {
    let config = app.state::<crate::PluginConfig>();
    let token = require_access_token(&app)?;
    crate::fetch::linear_list_teams(&config.api_base_url, &token, &connection_id, limit, cursor)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn linear_list_tickets<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    connection_id: String,
    team_id: String,
    query: Option<String>,
    limit: Option<u32>,
    cursor: Option<String>,
) -> Result<TicketPage, Error> {
    let config = app.state::<crate::PluginConfig>();
    let token = require_access_token(&app)?;
    crate::fetch::linear_list_tickets(
        &config.api_base_url,
        &token,
        &connection_id,
        &team_id,
        query,
        limit,
        cursor,
    )
    .await
}

fn require_access_token<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<String, Error> {
    let token = app.access_token().map_err(|e| Error::Auth(e.to_string()))?;
    match token {
        Some(t) if !t.is_empty() => Ok(t),
        _ => Err(Error::Auth("not authenticated".to_string())),
    }
}
