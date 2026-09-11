use tauri::Manager;

use crate::{CreatedWebhook, MarkdownExportOptions, WebhookDelivery, WebhookInfo, dispatch};

const MAX_CLOUD_SNAPSHOT_BYTES: usize = 2 * 1024 * 1024;
static MARKDOWN_EXPORT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
    options: Option<MarkdownExportOptions>,
) -> Result<String, String> {
    let directory = directory.trim();
    if directory.is_empty() {
        return Err("export directory is not set".to_string());
    }
    let pool = pool(&app)?;
    let export = anlg_agent_access::get_meeting_export(&pool, meeting_id)
        .await
        .map_err(|error| error.to_string())?;
    write_markdown_export_with_options(std::path::Path::new(directory), &export, options.as_ref())
        .map(|path| path.to_string_lossy().into_owned())
}

pub(crate) fn markdown_export_filename(meeting: &anlg_agent_access::Meeting) -> String {
    configured_markdown_filename(meeting, &MarkdownExportOptions::default())
}

pub(crate) fn configured_markdown_filename(
    meeting: &anlg_agent_access::Meeting,
    options: &MarkdownExportOptions,
) -> String {
    let title = meeting.title.trim();
    let title = if title.is_empty() {
        "Untitled meeting"
    } else {
        title
    };
    let occurred_at = if meeting.started_at.is_empty() {
        &meeting.created_at
    } else {
        &meeting.started_at
    };
    let date = occurred_at.get(..10).unwrap_or("");
    let custom = options.filename.trim();
    let base = if custom.is_empty() {
        format!("{date} {title}").trim().to_string()
    } else {
        custom.replace("{title}", title).replace("{date}", date)
    };
    let base = base.trim();
    let base = if !custom.is_empty() && base.to_ascii_lowercase().ends_with(".md") {
        &base[..base.len() - 3]
    } else {
        base
    };
    let mut sanitized = sanitize_filename_part(base);
    if sanitized.is_empty() {
        sanitized = "Untitled meeting".to_string();
    }
    let stem = sanitized
        .split('.')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    {
        sanitized.insert(0, '_');
    }
    let suffix = if options.include_id_suffix {
        let prefix = meeting.id.chars().take(8).collect::<String>();
        format!(" [{}]", sanitize_filename_part(&prefix))
    } else {
        String::new()
    };
    format!("{sanitized}{suffix}.md")
}

fn sanitize_filename_part(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect::<String>();
    // Leave room for the suffix and extension on filesystems with a 255-byte limit.
    let mut end = sanitized.len().min(180);
    while !sanitized.is_char_boundary(end) {
        end -= 1;
    }
    sanitized[..end].trim_matches([' ', '.']).to_string()
}

pub(crate) fn write_markdown_export(
    directory: &std::path::Path,
    export: &anlg_agent_access::MeetingExport,
) -> Result<std::path::PathBuf, String> {
    write_markdown_export_with_options(directory, export, None)
}

pub(crate) fn write_markdown_export_with_options(
    directory: &std::path::Path,
    export: &anlg_agent_access::MeetingExport,
    options: Option<&MarkdownExportOptions>,
) -> Result<std::path::PathBuf, String> {
    use std::io::Write;

    // Native dispatch and desktop workflows can export the same meeting concurrently.
    let _guard = MARKDOWN_EXPORT_LOCK
        .lock()
        .map_err(|error| error.to_string())?;
    let defaults = MarkdownExportOptions::default();
    let selected = options.unwrap_or(&defaults);
    if !(selected.include_memo
        || selected.include_summary
        || selected.include_transcript
        || selected.include_action_items)
    {
        return Err("choose at least one element to export".to_string());
    }
    let mut filtered = export.clone();
    if !selected.include_memo {
        filtered.meeting.note = None;
    }
    if !selected.include_summary {
        filtered.meeting.summaries.clear();
    }
    if !selected.include_transcript {
        filtered.transcripts.clear();
    }
    if !selected.include_action_items {
        filtered.meeting.action_items.clear();
    }

    std::fs::create_dir_all(directory)
        .map_err(|error| format!("could not create export directory: {error}"))?;
    let filename = if options.is_none() {
        markdown_export_filename(&export.meeting)
    } else {
        configured_markdown_filename(&export.meeting, selected)
    };
    let path = directory.join(&filename);
    let mut markdown = filtered.to_markdown();
    markdown.push('\n');
    let legacy_prefix = legacy_export_prefix(&export.meeting.id);
    if options.is_none() {
        markdown.insert_str(0, &legacy_prefix);
    }
    let existing = match std::fs::read_to_string(&path) {
        Ok(content) => {
            let marker = format!("- ID: `{}`", export.meeting.id);
            let existing_id = content
                .strip_prefix(&legacy_prefix)
                .unwrap_or(&content)
                .split("\n\n")
                .nth(1)
                .and_then(|metadata| metadata.lines().next());
            if existing_id != Some(marker.as_str()) {
                return Err(format!(
                    "{filename} already exists for another file; choose a different filename or include the meeting ID suffix"
                ));
            }
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(format!("could not read existing export: {error}")),
    };
    persist_markdown_export(&path, existing, |file| file.write_all(markdown.as_bytes()))
        .map_err(|error| format!("could not write markdown export: {error}"))?;
    // Configured actions can export different content for the same meeting to
    // the same folder. Only the legacy export owns its old filename cleanup.
    if options.is_none() {
        remove_stale_exports(directory, &export.meeting.id, &filename);
    }
    Ok(path)
}

pub(crate) fn persist_markdown_export(
    path: &std::path::Path,
    replace_existing: bool,
    write: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> std::io::Result<()> {
    use std::io::Write;

    let directory = path
        .parent()
        .ok_or_else(|| std::io::Error::other("export has no parent folder"))?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".anlg-export-")
        .tempfile_in(directory)?;
    write(temporary.as_file_mut())?;
    if replace_existing {
        let metadata = std::fs::metadata(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, fchown};
            let temporary_metadata = temporary.as_file().metadata()?;
            if metadata.uid() != temporary_metadata.uid()
                || metadata.gid() != temporary_metadata.gid()
            {
                fchown(
                    temporary.as_file(),
                    Some(metadata.uid()),
                    Some(metadata.gid()),
                )?;
            }
        }
        // Apply modes after ownership, since changing ownership can clear mode bits.
        temporary
            .as_file()
            .set_permissions(metadata.permissions())?;
    }
    temporary.as_file_mut().flush()?;
    temporary.as_file().sync_all()?;
    if replace_existing {
        temporary.persist(path).map_err(|error| error.error)?;
    } else {
        temporary
            .persist_noclobber(path)
            .map_err(|error| error.error)?;
    }
    Ok(())
}

fn legacy_export_prefix(meeting_id: &str) -> String {
    format!("<!-- anarlog:legacy-markdown-export {meeting_id:?} -->\n\n")
}

// Only remove files explicitly owned by the legacy exporter. Unmarked files
// may belong to users or configured actions, even when their ID suffix matches.
fn remove_stale_exports(directory: &std::path::Path, meeting_id: &str, keep_filename: &str) {
    let id_prefix = meeting_id.chars().take(8).collect::<String>();
    if id_prefix.is_empty() {
        return;
    }
    let marker = format!(" [{id_prefix}].md");
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let owner = legacy_export_prefix(meeting_id);
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name != keep_filename
            && name.ends_with(&marker)
            && std::fs::read_to_string(entry.path())
                .is_ok_and(|content| content.starts_with(&owner))
        {
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
