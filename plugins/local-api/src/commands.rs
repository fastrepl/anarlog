use tauri::Manager;

use crate::{
    ApiKeyInfo, CreatedApiKey, CreatedWebhook, LocalApiStatus, PluginState, WebhookDelivery,
    WebhookInfo, dispatch, server, settings,
};

const MAX_CLOUD_SNAPSHOT_BYTES: usize = 2 * 1024 * 1024;

fn pool<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<sqlx::SqlitePool, String> {
    app.try_state::<tauri_plugin_db::ManagedState>()
        .map(|state| state.pool().clone())
        .ok_or_else(|| "database is not ready yet".to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_status<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<LocalApiStatus, String> {
    let state = app.state::<PluginState>();
    let running_port = state.server.lock().await.as_ref().map(|handle| handle.port);
    let settings = match pool(&app) {
        Ok(pool) => settings::load(&pool).await.unwrap_or_default(),
        Err(_) => settings::LocalApiSettings::default(),
    };
    Ok(LocalApiStatus {
        enabled: settings.enabled,
        port: running_port.unwrap_or(settings.port),
        running: running_port.is_some(),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn set_enabled<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    enabled: bool,
) -> Result<LocalApiStatus, String> {
    let pool = pool(&app)?;
    let state = app.state::<PluginState>();
    let mut server = state.server.lock().await;
    let mut settings = settings::load(&pool).await.map_err(|e| e.to_string())?;
    if enabled {
        if server.is_none() {
            let handle = match server::start(pool.clone(), settings.port).await {
                Ok(handle) => handle,
                Err(error) => {
                    if settings.enabled {
                        settings.enabled = false;
                        settings::store(&pool, &settings)
                            .await
                            .map_err(|rollback| {
                                format!(
                                    "{error}; could not clear enabled setting after bind failure: {rollback}"
                                )
                            })?;
                    }
                    return Err(error);
                }
            };
            settings.enabled = true;
            if let Err(error) = settings::store(&pool, &settings).await {
                handle.shutdown().await;
                return Err(error.to_string());
            }
            *server = Some(handle);
        } else {
            settings.enabled = true;
            settings::store(&pool, &settings)
                .await
                .map_err(|e| e.to_string())?;
        }
    } else {
        settings.enabled = false;
        settings::store(&pool, &settings)
            .await
            .map_err(|e| e.to_string())?;
        if let Some(handle) = server.take() {
            handle.shutdown().await;
        }
    }

    let running_port = server.as_ref().map(|handle| handle.port);
    Ok(LocalApiStatus {
        enabled: settings.enabled,
        port: running_port.unwrap_or(settings.port),
        running: running_port.is_some(),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn list_api_keys<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<ApiKeyInfo>, String> {
    let pool = pool(&app)?;
    Ok(anlg_db_app::list_api_keys(&pool)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(ApiKeyInfo::from)
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn create_api_key<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    name: String,
) -> Result<CreatedApiKey, String> {
    let pool = pool(&app)?;
    let generated = anlg_db_app::generate_api_key();
    let row = anlg_db_app::insert_api_key(
        &pool,
        &uuid::Uuid::new_v4().to_string(),
        name.trim(),
        &generated.key_hash,
        &generated.key_prefix,
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(CreatedApiKey {
        info: ApiKeyInfo::from(row),
        key: generated.key,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn revoke_api_key<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    id: String,
) -> Result<bool, String> {
    let pool = pool(&app)?;
    anlg_db_app::revoke_api_key(&pool, &id)
        .await
        .map_err(|e| e.to_string())
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
    server::create_endpoint(&pool, &url, &events).await
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
pub async fn test_webhook<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    id: String,
) -> Result<WebhookDelivery, String> {
    let pool = pool(&app)?;
    let endpoint = anlg_db_app::get_webhook_endpoint(&pool, &id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("webhook '{id}' not found"))?;
    Ok(dispatch::send_test(&pool, &endpoint).await)
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
    let targeted = dispatch::dispatch_event(&pool, &event, &meeting_id).await?;
    Ok(targeted as u32)
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
    prepare_cloud_snapshot(export)
}

pub(crate) fn prepare_cloud_snapshot(
    mut export: anlg_agent_access::MeetingExport,
) -> Result<serde_json::Value, String> {
    let serialized = serde_json::to_vec(&export).map_err(|error| error.to_string())?;
    if serialized.len() > MAX_CLOUD_SNAPSHOT_BYTES {
        for transcript in &mut export.transcripts {
            transcript.words.clear();
            transcript.speaker_hints.clear();
        }
    }
    let serialized = serde_json::to_vec(&export).map_err(|error| error.to_string())?;
    if serialized.len() > MAX_CLOUD_SNAPSHOT_BYTES {
        return Err(format!(
            "meeting snapshot exceeds the {MAX_CLOUD_SNAPSHOT_BYTES}-byte limit"
        ));
    }
    serde_json::to_value(export).map_err(|error| error.to_string())
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
