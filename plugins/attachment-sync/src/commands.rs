use crate::models::{
    PreparedUpload, RestoredAttachment, SharedAttachmentCacheResult, UploadDescriptor,
};
use tauri::Manager;

#[tauri::command]
#[specta::specta]
pub(crate) async fn begin_attachment_download(
    control: tauri::State<'_, crate::control::DownloadControl>,
    operation_id: String,
    scope_id: Option<String>,
) -> Result<(), String> {
    control
        .begin(&operation_id, scope_id.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn cancel_attachment_download(
    control: tauri::State<'_, crate::control::DownloadControl>,
    operation_id: String,
) -> Result<bool, String> {
    control
        .cancel(&operation_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn describe_upload(
    state: tauri::State<'_, tauri_plugin_db::ManagedState>,
    job_id: String,
    attempt_count: i64,
) -> Result<UploadDescriptor, String> {
    crate::runtime::describe_upload(state.inner(), &job_id, attempt_count)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn prepare_upload<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, tauri_plugin_db::ManagedState>,
    job_id: String,
    attempt_count: i64,
    object_id: String,
    object_key: String,
) -> Result<PreparedUpload, String> {
    crate::runtime::prepare_upload(
        &app,
        state.inner(),
        &job_id,
        attempt_count,
        &object_id,
        &object_key,
    )
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn read_upload_range<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, tauri_plugin_db::ManagedState>,
    job_id: String,
    attempt_count: i64,
    cache_id: String,
    start: u64,
    end: u64,
) -> Result<Vec<u8>, String> {
    crate::runtime::read_upload_range(
        &app,
        state.inner(),
        &job_id,
        attempt_count,
        &cache_id,
        start,
        end,
    )
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn read_attachment_range<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, tauri_plugin_db::ManagedState>,
    attachment_id: String,
    start: u64,
    end: u64,
) -> Result<Vec<u8>, String> {
    crate::runtime::read_attachment_range(&app, state.inner(), &attachment_id, start, end)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn verify_delete_source<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, tauri_plugin_db::ManagedState>,
    job_id: String,
    attempt_count: i64,
) -> Result<bool, String> {
    crate::runtime::verify_delete_source(&app, state.inner(), &job_id, attempt_count)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn download_and_restore<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, tauri_plugin_db::ManagedState>,
    operation_id: String,
    job_id: String,
    attempt_count: i64,
    object_id: String,
    signed_url: String,
    ciphertext_sha256: String,
    ciphertext_size_bytes: u64,
    format_version: i16,
) -> Result<RestoredAttachment, String> {
    let control = app.state::<crate::control::DownloadControl>();
    let operation = control
        .start(&operation_id, None)
        .map_err(|error| error.to_string())?;
    crate::runtime::download_and_restore(
        &app,
        state.inner(),
        &operation,
        &job_id,
        attempt_count,
        &object_id,
        &signed_url,
        &ciphertext_sha256,
        ciphertext_size_bytes,
        format_version,
    )
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn cleanup_transfer_cache<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, tauri_plugin_db::ManagedState>,
    job_id: String,
    attempt_count: i64,
    expected_cache_id: String,
) -> Result<bool, String> {
    crate::runtime::cleanup_transfer_cache(
        &app,
        state.inner(),
        &job_id,
        attempt_count,
        &expected_cache_id,
    )
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn download_shared_attachment<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    operation_id: String,
    scope_id: String,
    attachment_id: String,
    signed_url: String,
    expected_sha256: String,
    expected_size_bytes: u64,
) -> Result<SharedAttachmentCacheResult, String> {
    let control = app.state::<crate::control::DownloadControl>();
    let operation = control
        .start(&operation_id, Some(&scope_id))
        .map_err(|error| error.to_string())?;
    crate::runtime::download_shared_attachment(
        &app,
        &operation,
        &scope_id,
        &attachment_id,
        &signed_url,
        &expected_sha256,
        expected_size_bytes,
    )
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn shared_attachment_path<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    scope_id: String,
    attachment_id: String,
) -> Result<Option<String>, String> {
    crate::runtime::existing_shared_attachment_path(&app, &scope_id, &attachment_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn remove_shared_attachment<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    scope_id: String,
    attachment_id: String,
) -> Result<bool, String> {
    crate::runtime::remove_shared_attachment(&app, &scope_id, &attachment_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn clear_shared_attachment_scope<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    scope_id: String,
) -> Result<u64, String> {
    let control = app.state::<crate::control::DownloadControl>();
    let clear = control
        .begin_scope_clear(&scope_id)
        .map_err(|error| error.to_string())?;
    clear.wait().await;
    crate::runtime::clear_shared_attachment_scope(&app, &scope_id)
        .await
        .map_err(|error| error.to_string())
}
