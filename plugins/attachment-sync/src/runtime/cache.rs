use std::path::PathBuf;

use tauri::{Manager, Runtime};

pub(super) use anlg_attachment_sync_core::{
    CacheFileGuard, cached_shared_attachment_path, cleanup_shared_upload_path,
    clear_attachment_cache_directory, commit_shared_cache_entry, create_shared_upload_cache_root,
    ensure_file_size, file_matches, file_matches_async, file_matches_cancellable,
    file_matches_cancellable_async, private_cache_path, read_range, read_shared_cache_metadata,
    shared_cache_id, shared_cache_matches_async, shared_cache_metadata_path,
    shared_upload_cache_path, snapshot_verified_file, sync_destination_directory, valid_cache_id,
    valid_sha256, validate_range,
};
use anlg_attachment_sync_core::{Error, Result, TransferPaths};
#[cfg(test)]
pub(super) use anlg_attachment_sync_core::{
    MAX_RANGE_BYTES, hash_identifier, hex_digest, shared_cache_matches, write_shared_cache_metadata,
};

pub(super) fn private_cache_root<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf> {
    Ok(cache_paths(app)?.private_cache_root())
}

pub(super) fn shared_upload_cache_root<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf> {
    Ok(cache_paths(app)?.shared_upload_cache_root())
}

pub(super) fn shared_scope_path<R: Runtime>(
    app: &tauri::AppHandle<R>,
    scope_id: &str,
) -> Result<PathBuf> {
    cache_paths(app)?.shared_scope_path(scope_id)
}

pub(super) fn shared_cache_root<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf> {
    Ok(cache_paths(app)?.shared_cache_root())
}

pub(super) fn shared_preview_cache_root<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf> {
    Ok(cache_paths(app)?.shared_preview_cache_root())
}

pub(super) fn delete_guard_root<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf> {
    let data_base = app
        .path()
        .app_data_dir()
        .map_err(|_| Error::CacheUnavailable)?;
    Ok(TransferPaths::new(PathBuf::new(), data_base).delete_guard_root())
}

fn cache_paths<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<TransferPaths> {
    let cache_base = app
        .path()
        .app_cache_dir()
        .map_err(|_| Error::CacheUnavailable)?;
    Ok(TransferPaths::new(cache_base, PathBuf::new()))
}
