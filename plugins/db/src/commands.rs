use tauri::ipc::Channel;

use crate::{ManagedState, QueryEvent};

#[tauri::command]
#[specta::specta]
pub(crate) async fn execute(
    state: tauri::State<'_, ManagedState>,
    sql: String,
    params: Vec<serde_json::Value>,
) -> Result<Vec<serde_json::Value>, String> {
    state
        .execute(sql, params)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn subscribe(
    state: tauri::State<'_, ManagedState>,
    sql: String,
    params: Vec<serde_json::Value>,
    on_event: Channel<QueryEvent>,
) -> Result<String, String> {
    state
        .subscribe(
            sql,
            params,
            crate::runtime::QueryEventChannel::new(on_event),
        )
        .await
        .map(|registration| registration.id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn unsubscribe(
    state: tauri::State<'_, ManagedState>,
    subscription_id: String,
) -> Result<(), String> {
    state
        .unsubscribe(&subscription_id)
        .await
        .map_err(|error| error.to_string())
}
