use std::path::PathBuf;

use crate::GitPluginExt;
use crate::types::{CommitInfo, ConflictInfo, PullResult, PushResult, RemoteInfo, StatusInfo};

#[tauri::command]
#[specta::specta]
pub(crate) async fn is_repo<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: PathBuf,
) -> Result<bool, String> {
    Ok(app.git().is_repo(&path))
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn init<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: PathBuf,
) -> Result<(), String> {
    app.git().init(&path).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn status<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: PathBuf,
) -> Result<StatusInfo, String> {
    app.git().status(&path).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn add<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: PathBuf,
    patterns: Vec<String>,
) -> Result<(), String> {
    app.git().add(&path, patterns).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn reset<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: PathBuf,
    files: Vec<String>,
) -> Result<(), String> {
    app.git().reset(&path, files).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn commit<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: PathBuf,
    message: String,
) -> Result<String, String> {
    app.git().commit(&path, &message).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn log<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: PathBuf,
    limit: u32,
) -> Result<Vec<CommitInfo>, String> {
    app.git().log(&path, limit).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn add_remote<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: PathBuf,
    name: String,
    url: String,
) -> Result<(), String> {
    app.git()
        .add_remote(&path, &name, &url)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn list_remotes<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: PathBuf,
) -> Result<Vec<RemoteInfo>, String> {
    app.git().list_remotes(&path).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn fetch<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: PathBuf,
    remote_name: String,
) -> Result<(), String> {
    app.git()
        .fetch(&path, &remote_name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn push<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: PathBuf,
    remote_name: String,
    branch: String,
) -> Result<PushResult, String> {
    app.git()
        .push(&path, &remote_name, &branch)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn pull<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: PathBuf,
    remote_name: String,
    branch: String,
) -> Result<PullResult, String> {
    app.git()
        .pull(&path, &remote_name, &branch)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn check_conflicts<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: PathBuf,
) -> Result<Option<ConflictInfo>, String> {
    app.git().check_conflicts(&path).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn abort_merge<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: PathBuf,
) -> Result<(), String> {
    app.git().abort_merge(&path).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_current_branch<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: PathBuf,
) -> Result<String, String> {
    app.git()
        .get_current_branch(&path)
        .map_err(|e| e.to_string())
}
