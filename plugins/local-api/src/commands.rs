use tauri::Manager;

use anlg_local_api_core::{CreatedWebhook, WebhookDelivery, WebhookInfo, dispatch, export};

fn pool<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<sqlx::SqlitePool, String> {
    app.try_state::<tauri_plugin_db::ManagedState>()
        .map(|state| state.pool().clone())
        .ok_or_else(|| "database is not ready yet".to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn list_webhooks<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<WebhookInfo>, String> {
    let pool = pool(&app)?;
    Ok(anlg_db_app::list_webhook_endpoints(&pool)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(WebhookInfo::from)
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn create_webhook<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    url: String,
    events: Vec<String>,
) -> Result<CreatedWebhook, String> {
    let pool = pool(&app)?;
    dispatch::create_endpoint(&pool, &url, &events).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_webhook<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    id: String,
) -> Result<bool, String> {
    let pool = pool(&app)?;
    anlg_db_app::delete_webhook_endpoint(&pool, &id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn set_webhook_active<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    id: String,
    active: bool,
) -> Result<WebhookInfo, String> {
    let pool = pool(&app)?;
    anlg_db_app::set_webhook_endpoint_active(&pool, &id, active)
        .await
        .map_err(|e| e.to_string())?
        .map(WebhookInfo::from)
        .ok_or_else(|| format!("webhook '{id}' not found"))
}

#[tauri::command]
#[specta::specta]
pub async fn test_webhook<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    id: String,
) -> Result<WebhookDelivery, String> {
    let pool = pool(&app)?;
    let endpoint = anlg_db_app::get_webhook_endpoint(&pool, &id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("webhook '{id}' not found"))?;
    dispatch::send_test(&pool, &endpoint).await
}

#[tauri::command]
#[specta::specta]
pub async fn dispatch_event<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    event: String,
    meeting_id: String,
) -> Result<u32, String> {
    if !dispatch::KNOWN_EVENTS.contains(&event.as_str()) {
        return Err(format!(
            "unknown event '{event}'; known events: {}",
            dispatch::KNOWN_EVENTS.join(", ")
        ));
    }
    let pool = pool(&app)?;
    if event == dispatch::EVENT_NOTE_ENHANCED {
        export::run_markdown_export_automation(&pool, &meeting_id).await;
    }
    let targeted = dispatch::dispatch_event(&pool, &event, &meeting_id).await?;
    Ok(targeted as u32)
}

#[tauri::command]
#[specta::specta]
pub async fn export_meeting_markdown<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    meeting_id: String,
    directory: String,
) -> Result<String, String> {
    let pool = pool(&app)?;
    export::export_meeting_markdown(&pool, meeting_id, &directory).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_cloud_snapshot<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    meeting_id: String,
) -> Result<serde_json::Value, String> {
    let pool = pool(&app)?;
    let export = anlg_agent_access::get_meeting_export(&pool, meeting_id)
        .await
        .map_err(|error| error.to_string())?;
    export::prepare_cloud_snapshot(export)
}

#[tauri::command]
#[specta::specta]
pub async fn list_cloud_snapshot_ids<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<String>, String> {
    let pool = pool(&app)?;
    let mut offset = 0;
    let mut ids = Vec::new();
    loop {
        let page = anlg_agent_access::list_meetings(
            &pool,
            anlg_agent_access::ListMeetingsInput {
                query: None,
                series_id: None,
                limit: Some(anlg_agent_access::MAX_LIST_LIMIT),
                offset: Some(offset),
            },
        )
        .await
        .map_err(|error| error.to_string())?;
        ids.extend(page.meetings.into_iter().map(|meeting| meeting.id));
        let Some(next_offset) = page.pagination.next_offset else {
            break;
        };
        offset = next_offset;
    }
    Ok(ids)
}
