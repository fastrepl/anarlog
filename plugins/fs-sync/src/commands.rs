use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use rayon::prelude::*;
use serde_json::Value;
use tauri_plugin_notify::NotifyPluginExt;
use tauri_plugin_settings::SettingsPluginExt;

use crate::FsSyncPluginExt;
use crate::frontmatter::ParsedDocument;
use crate::session::find_session_dir;
use crate::session_content::load_session_content as load_session_content_from_fs;
use crate::types::{
    CleanupTarget, ListFoldersResult, MoveSessionResult, RenameFolderResult, ScanResult,
    SessionContentData,
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
    Ok(find_session_dir(
        &base.join("sessions").into_std_path_buf(),
        session_id,
    ))
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

    let relative_paths: Vec<String> = items
        .iter()
        .filter_map(|(_, path)| {
            std::path::Path::new(path)
                .strip_prefix(base.as_std_path())
                .ok()
                .and_then(|p| p.to_str())
                .map(|s| s.to_string())
        })
        .collect();

    app.notify().mark_own_writes(&relative_paths);

    spawn_blocking!({
        items.into_par_iter().try_for_each(|(json, path)| {
            let path = std::path::Path::new(&path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let content = crate::json::serialize(json)?;
            std::fs::write(path, content).map_err(|e| e.to_string())
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

    let relative_paths: Vec<String> = items
        .iter()
        .filter_map(|(_, path)| {
            std::path::Path::new(path)
                .strip_prefix(base.as_std_path())
                .ok()
                .and_then(|p| p.to_str())
                .map(|s| s.to_string())
        })
        .collect();

    app.notify().mark_own_writes(&relative_paths);

    spawn_blocking!({
        items.into_par_iter().try_for_each(|(doc, path)| {
            let path = std::path::Path::new(&path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let content = doc.render().map_err(|e| e.to_string())?;
            std::fs::write(path, content).map_err(|e| e.to_string())
        })
    })
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
pub(crate) async fn cleanup_orphan<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    target: CleanupTarget,
    valid_ids: Vec<String>,
) -> Result<u32, String> {
    app.fs_sync()
        .cleanup_orphan(target, valid_ids)
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
) -> Result<(), String> {
    let session_dir = resolve_session_dir(&app, &session_id)?;
    crate::audio::delete(&session_dir).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn audio_import<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
    source_path: String,
) -> Result<String, String> {
    let session_dir = resolve_session_dir(&app, &session_id)?;
    crate::audio::import_to_session(&session_dir, &PathBuf::from(&source_path))
        .map(|path| path.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
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
    spawn_blocking!({
        Ok(crate::scan::scan_and_read(
            &PathBuf::from(&scan_dir),
            base.as_std_path(),
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
