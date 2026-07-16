use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use rayon::prelude::*;
use serde_json::Value;
use tauri_plugin_notify::NotifyPluginExt;
use tauri_plugin_settings::SettingsPluginExt;

use crate::FsSyncPluginExt;
use crate::frontmatter::ParsedDocument;
use crate::session::find_session_dir;
use crate::session_content::load_session_content as load_session_content_from_fs;
use crate::types::{
    ListFoldersResult, MoveSessionResult, RenameFolderResult, ScanResult, SessionContentData,
};

macro_rules! spawn_blocking {
    ($body:expr) => {
        tokio::task::spawn_blocking(move || $body)
            .await
            .map_err(|e| e.to_string())?
    };
}

fn resolve_session_dir<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    session_id: &str,
) -> Result<PathBuf, String> {
    let base = app.settings().vault_base().map_err(|e| e.to_string())?;
    find_session_dir(&base.join("sessions").into_std_path_buf(), session_id)
        .map_err(|e| e.to_string())
}

fn resolve_vault_path(base: &Path, path: &str) -> Result<PathBuf, String> {
    crate::path::resolve_path_inside_base(base, Path::new(path)).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn deserialize(input: String) -> Result<ParsedDocument, String> {
    ParsedDocument::from_str(&input).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn write_json_batch<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    items: Vec<(Value, String)>,
) -> Result<(), String> {
    let base = app.settings().vault_base().map_err(|e| e.to_string())?;
    let base_path = base
        .as_std_path()
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let items: Vec<(Value, PathBuf)> = items
        .into_iter()
        .map(|(json, path)| {
            resolve_vault_path(&base_path, &path)
                .map(|resolved_path| (json, resolved_path))
                .map_err(|e| format!("failed to resolve json path {path}: {e}"))
        })
        .collect::<Result<_, _>>()?;

    let relative_paths: Vec<String> = items
        .iter()
        .filter_map(|(_, path)| {
            path.strip_prefix(&base_path)
                .ok()
                .and_then(|p| p.to_str())
                .map(|s| s.to_string())
        })
        .collect();

    app.notify().mark_own_writes(&relative_paths);

    spawn_blocking!({
        items.into_par_iter().try_for_each(|(json, path)| {
            create_parent_dir_for_write(&path)?;
            let content = crate::json::serialize(json)
                .map_err(|e| format!("failed to serialize json for {}: {e}", path.display()))?;
            write_file_with_context(&path, content)
        })
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn write_document_batch<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    items: Vec<(ParsedDocument, String)>,
) -> Result<(), String> {
    let base = app.settings().vault_base().map_err(|e| e.to_string())?;
    let base_path = base
        .as_std_path()
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let items: Vec<(ParsedDocument, PathBuf)> = items
        .into_iter()
        .map(|(doc, path)| {
            resolve_vault_path(&base_path, &path)
                .map(|resolved_path| (doc, resolved_path))
                .map_err(|e| format!("failed to resolve document path {path}: {e}"))
        })
        .collect::<Result<_, _>>()?;

    let relative_paths: Vec<String> = items
        .iter()
        .filter_map(|(_, path)| {
            path.strip_prefix(&base_path)
                .ok()
                .and_then(|p| p.to_str())
                .map(|s| s.to_string())
        })
        .collect();

    app.notify().mark_own_writes(&relative_paths);

    spawn_blocking!({
        items.into_par_iter().try_for_each(|(doc, path)| {
            create_parent_dir_for_write(&path)?;
            let content = doc
                .render()
                .map_err(|e| format!("failed to render document for {}: {e}", path.display()))?;
            write_file_with_context(&path, content)
        })
    })
}

fn create_parent_dir_for_write(path: &Path) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };

    std::fs::create_dir_all(parent).map_err(|e| {
        format!(
            "failed to create parent directory {} for {}: {e}",
            parent.display(),
            path.display()
        )
    })
}

fn write_file_with_context(path: &Path, content: String) -> Result<(), String> {
    std::fs::write(path, content).map_err(|e| format!("failed to write {}: {e}", path.display()))
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn read_document_batch(
    dir_path: String,
) -> Result<HashMap<String, ParsedDocument>, String> {
    spawn_blocking!({
        let files = crate::session::list_uuid_files(&PathBuf::from(&dir_path), "md");
        let results: HashMap<_, _> = files
            .into_par_iter()
            .filter_map(|(id, path)| {
                let content = std::fs::read_to_string(&path).ok()?;
                let doc = ParsedDocument::from_str(&content).ok()?;
                Some((id, doc))
            })
            .collect();
        Ok(results)
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn list_folders<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<ListFoldersResult, String> {
    app.fs_sync().list_folders().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn move_session<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
    from_folder_path: String,
    target_folder_path: String,
) -> Result<MoveSessionResult, String> {
    app.fs_sync()
        .move_session(&session_id, &from_folder_path, &target_folder_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn create_folder<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    folder_path: String,
) -> Result<(), String> {
    app.fs_sync()
        .create_folder(&folder_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn rename_folder<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    old_path: String,
    new_path: String,
) -> Result<RenameFolderResult, String> {
    app.fs_sync()
        .rename_folder(&old_path, &new_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn delete_folder<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    folder_path: String,
) -> Result<(), String> {
    app.fs_sync()
        .delete_folder(&folder_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn audio_exist<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
) -> Result<bool, String> {
    let session_dir = resolve_session_dir(&app, &session_id)?;
    crate::audio::exists(&session_dir).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn audio_delete<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
) -> Result<bool, String> {
    let session_dir = resolve_session_dir(&app, &session_id)?;
    crate::audio::delete(&session_dir).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn audio_metadata<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
) -> Result<Option<crate::audio::AudioFileMetadata>, String> {
    let session_dir = resolve_session_dir(&app, &session_id)?;
    spawn_blocking!({ crate::audio::metadata(&session_dir).map_err(|e| e.to_string()) })
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn audio_delete_orphaned_expired<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    known_session_ids: Vec<String>,
    retention_ms: u64,
    now_ms: u64,
) -> Result<Vec<String>, String> {
    let base = app.settings().vault_base().map_err(|e| e.to_string())?;
    let sessions_dir = base.join("sessions").into_std_path_buf();
    spawn_blocking!({
        crate::audio::delete_orphaned_expired(
            &sessions_dir,
            &known_session_ids,
            retention_ms,
            now_ms,
        )
        .map_err(|e| e.to_string())
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn audio_import<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
    source_path: String,
) -> Result<String, String> {
    let session_dir = resolve_session_dir(&app, &session_id)?;
    let source_path = PathBuf::from(&source_path);
    let runtime = crate::runtime::TauriAudioImportRuntime::new(app);
    spawn_blocking!({
        crate::audio::import_to_session(&runtime, &session_id, &session_dir, &source_path)
            .map(|path| path.to_string_lossy().to_string())
            .map_err(|e| e.to_string())
    })
}

fn audio_import_source_extension(filename: &str) -> String {
    let extension = Path::new(filename)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);

    match extension.as_deref() {
        Some("wav" | "mp3" | "ogg" | "mp4" | "m4a" | "flac") => extension.unwrap(),
        _ => "mp3".to_string(),
    }
}

fn audio_import_source_path(session_dir: &Path, filename: &str) -> PathBuf {
    let extension = audio_import_source_extension(filename);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    session_dir.join(format!(
        "audio-upload-{}-{}.{}",
        std::process::id(),
        nonce,
        extension
    ))
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn audio_import_data<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
    data: Vec<u8>,
    filename: String,
) -> Result<String, String> {
    let session_dir = resolve_session_dir(&app, &session_id)?;
    let runtime = crate::runtime::TauriAudioImportRuntime::new(app);
    spawn_blocking!({
        std::fs::create_dir_all(&session_dir).map_err(|e| e.to_string())?;

        let source_path = audio_import_source_path(&session_dir, &filename);
        std::fs::write(&source_path, data).map_err(|e| e.to_string())?;

        let result =
            crate::audio::import_to_session(&runtime, &session_id, &session_dir, &source_path)
                .map(|path| path.to_string_lossy().to_string())
                .map_err(|e| e.to_string());

        if source_path.exists() {
            let _ = std::fs::remove_file(&source_path);
        }

        result
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn audio_source_metadata(
    source_path: String,
) -> Result<crate::audio::AudioSourceMetadata, String> {
    spawn_blocking!({
        crate::audio::source_metadata(&PathBuf::from(source_path)).map_err(|e| e.to_string())
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn audio_path<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
) -> Result<String, String> {
    let session_dir = resolve_session_dir(&app, &session_id)?;
    crate::audio::path(&session_dir)
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "audio_path_not_found".to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn session_dir<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
) -> Result<String, String> {
    resolve_session_dir(&app, &session_id).map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn load_session_content<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
) -> Result<SessionContentData, String> {
    let session_dir = resolve_session_dir(&app, &session_id)?;
    spawn_blocking!({ Ok(load_session_content_from_fs(&session_id, &session_dir)) })
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn delete_session_folder<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
) -> Result<(), String> {
    let session_dir = resolve_session_dir(&app, &session_id)?;
    crate::session::delete_session_dir(&session_dir).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn scan_and_read<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    scan_dir: String,
    file_patterns: Vec<String>,
    recursive: bool,
    path_filter: Option<String>,
) -> Result<ScanResult, String> {
    let base = app.settings().vault_base().map_err(|e| e.to_string())?;
    let base_path = base
        .as_std_path()
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let scan_dir = resolve_vault_path(&base_path, &scan_dir)?;
    spawn_blocking!({
        Ok(crate::scan::scan_and_read(
            &scan_dir,
            &base_path,
            &file_patterns,
            recursive,
            path_filter.as_deref(),
        ))
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn chat_dir<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    chat_group_id: String,
) -> Result<String, String> {
    let base = app.settings().vault_base().map_err(|e| e.to_string())?;
    Ok(base.join("chats").join(&chat_group_id).to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn entity_dir<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    dir_name: String,
) -> Result<String, String> {
    let base = app.settings().vault_base().map_err(|e| e.to_string())?;
    Ok(base.join(&dir_name).to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn attachment_save<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
    data: Vec<u8>,
    filename: String,
) -> Result<crate::AttachmentSaveResult, String> {
    spawn_blocking!({
        app.fs_sync()
            .attachment_save(&session_id, &data, &filename)
            .map_err(|e| e.to_string())
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn attachment_list<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
) -> Result<Vec<crate::AttachmentInfo>, String> {
    spawn_blocking!({
        app.fs_sync()
            .attachment_list(&session_id)
            .map_err(|e| e.to_string())
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn attachment_read<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
    attachment_id: String,
) -> Result<Vec<u8>, String> {
    spawn_blocking!({
        app.fs_sync()
            .attachment_read(&session_id, &attachment_id)
            .map_err(|e| e.to_string())
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn attachment_remove<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
    attachment_id: String,
) -> Result<(), String> {
    spawn_blocking!({
        app.fs_sync()
            .attachment_remove(&session_id, &attachment_id)
            .map_err(|e| e.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_parent_dir_error_includes_parent_and_target_paths() {
        let temp = tempfile::tempdir().unwrap();
        let blocker = temp.path().join("sessions");
        std::fs::write(&blocker, "not a directory").unwrap();

        let target = blocker.join("session-1").join("_meta.json");
        let error = create_parent_dir_for_write(&target).unwrap_err();

        assert!(error.contains("failed to create parent directory"));
        assert!(error.contains(&target.parent().unwrap().display().to_string()));
        assert!(error.contains(&target.display().to_string()));
    }
}
