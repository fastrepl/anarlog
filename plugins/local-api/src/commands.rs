use tauri::Manager;

use crate::{CreatedWebhook, WebhookDelivery, WebhookInfo, dispatch};

const MAX_CLOUD_SNAPSHOT_BYTES: usize = 2 * 1024 * 1024;

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
        run_markdown_export_automation(&pool, &meeting_id).await;
    }
    let targeted = dispatch::dispatch_event(&pool, &event, &meeting_id).await?;
    Ok(targeted as u32)
}

// The markdown export automation first runs on meeting.completed, before
// auto-enhance has generated the summary. The note.enhanced dispatch is the
// signal that the summary is persisted, so re-export here to rewrite the file
// with the summary included.
pub(crate) async fn run_markdown_export_automation(pool: &sqlx::SqlitePool, meeting_id: &str) {
    let enabled = load_setting(pool, "automation_markdown_export_enabled")
        .await
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let directory = load_setting(pool, "automation_markdown_export_directory")
        .await
        .and_then(|value| value.as_str().map(|value| value.trim().to_string()))
        .unwrap_or_default();
    if !enabled || directory.is_empty() {
        return;
    }

    let result = match anlg_agent_access::get_meeting_export(pool, meeting_id.to_string()).await {
        Ok(export) => write_markdown_export(std::path::Path::new(&directory), &export),
        Err(error) => Err(error.to_string()),
    };
    let (status, detail) = match result {
        Ok(path) => ("success", path.to_string_lossy().into_owned()),
        Err(error) => {
            tracing::warn!("[local-api] markdown re-export failed: {error}");
            ("error", error)
        }
    };
    let at: String = sqlx::query_scalar("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')")
        .fetch_one(pool)
        .await
        .unwrap_or_default();
    let record = serde_json::json!({ "at": at, "status": status, "detail": detail }).to_string();
    // The settings layer stores this value as a JSON-encoded string, so the
    // record is double-encoded to stay readable by the desktop app.
    let value_json = serde_json::Value::String(record).to_string();
    if let Err(error) = sqlx::query(
        "INSERT INTO app_settings (id, value_json, updated_at) \
         VALUES ('automation_markdown_export_last_run', ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
         ON CONFLICT(id) DO UPDATE SET \
           value_json = excluded.value_json, \
           updated_at = excluded.updated_at",
    )
    .bind(value_json)
    .execute(pool)
    .await
    {
        tracing::warn!("[local-api] could not record the markdown export run: {error}");
    }
}

async fn load_setting(pool: &sqlx::SqlitePool, id: &str) -> Option<serde_json::Value> {
    let raw: Option<String> =
        match sqlx::query_scalar("SELECT value_json FROM app_settings WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
        {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!("[local-api] could not load setting '{id}': {error}");
                None
            }
        };
    raw.and_then(|value| serde_json::from_str(&value).ok())
}

#[tauri::command]
#[specta::specta]
pub async fn export_meeting_markdown<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    meeting_id: String,
    directory: String,
) -> Result<String, String> {
    let directory = directory.trim();
    if directory.is_empty() {
        return Err("export directory is not set".to_string());
    }
    let pool = pool(&app)?;
    let export = anlg_agent_access::get_meeting_export(&pool, meeting_id)
        .await
        .map_err(|error| error.to_string())?;
    write_markdown_export(std::path::Path::new(directory), &export)
        .map(|path| path.to_string_lossy().into_owned())
}

pub(crate) fn markdown_export_filename(meeting: &anlg_agent_access::Meeting) -> String {
    let title = meeting.title.trim();
    let title = if title.is_empty() {
        "Untitled meeting"
    } else {
        title
    };
    let sanitized = title
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect::<String>();
    let sanitized = sanitized.trim_matches([' ', '.']);
    let sanitized = if sanitized.is_empty() {
        "Untitled meeting"
    } else {
        sanitized
    };
    let occurred_at = if meeting.started_at.is_empty() {
        &meeting.created_at
    } else {
        &meeting.started_at
    };
    let id_prefix = meeting.id.chars().take(8).collect::<String>();
    match occurred_at.get(..10) {
        Some(date) => format!("{date} {sanitized} [{id_prefix}].md"),
        None => format!("{sanitized} [{id_prefix}].md"),
    }
}

pub(crate) fn write_markdown_export(
    directory: &std::path::Path,
    export: &anlg_agent_access::MeetingExport,
) -> Result<std::path::PathBuf, String> {
    std::fs::create_dir_all(directory)
        .map_err(|error| format!("could not create export directory: {error}"))?;
    let filename = markdown_export_filename(&export.meeting);
    remove_stale_exports(directory, &export.meeting.id, &filename);
    let path = directory.join(&filename);
    let mut markdown = export.to_markdown();
    markdown.push('\n');
    std::fs::write(&path, markdown)
        .map_err(|error| format!("could not write markdown export: {error}"))?;
    Ok(path)
}

// A meeting is re-exported when its note is enhanced, and by then the title
// may have changed (e.g. auto-generated), renaming the export. Best-effort
// remove the meeting's previous file so only the latest export remains.
fn remove_stale_exports(directory: &std::path::Path, meeting_id: &str, keep_filename: &str) {
    let id_prefix = meeting_id.chars().take(8).collect::<String>();
    if id_prefix.is_empty() {
        return;
    }
    let marker = format!(" [{id_prefix}].md");
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name != keep_filename && name.ends_with(&marker) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
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
