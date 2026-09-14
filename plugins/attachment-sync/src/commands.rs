use crate::models::{
    PreparedDeleteGuard, PreparedSharedUpload, PreparedUpload, RestoredAttachment,
    SharedAttachmentCacheResult, SharedUploadVersion, UploadDescriptor,
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
pub(crate) async fn begin_shared_upload_operation(
    control: tauri::State<'_, crate::control::DownloadControl>,
    operation_id: String,
) -> Result<(), String> {
    control
        .begin(&operation_id, None)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn cancel_shared_upload_operation(
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
#[allow(clippy::too_many_arguments)]
pub(crate) async fn prepare_shared_upload<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    control: tauri::State<'_, crate::control::DownloadControl>,
    state: tauri::State<'_, tauri_plugin_db::ManagedState>,
    operation_id: String,
    attachment_id: String,
    expected: SharedUploadVersion,
) -> Result<PreparedSharedUpload, String> {
    let operation = control
        .start(&operation_id, None)
        .map_err(|error| error.to_string())?;
    crate::runtime::prepare_shared_upload(
        &app,
        state.inner(),
        &operation,
        &attachment_id,
        &expected.sha256,
        expected.size_bytes,
        &expected.filename,
        &expected.content_type,
        &expected.cloud_object_key,
    )
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn read_shared_upload_range<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, tauri_plugin_db::ManagedState>,
    attachment_id: String,
    cache_id: String,
    expected: SharedUploadVersion,
    start: u64,
    end: u64,
) -> Result<Vec<u8>, String> {
    crate::runtime::read_shared_upload_range(
        &app,
        state.inner(),
        &attachment_id,
        &cache_id,
        &expected.sha256,
        expected.size_bytes,
        &expected.filename,
        &expected.content_type,
        &expected.cloud_object_key,
        start,
        end,
    )
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn validate_shared_upload<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    control: tauri::State<'_, crate::control::DownloadControl>,
    state: tauri::State<'_, tauri_plugin_db::ManagedState>,
    operation_id: String,
    attachment_id: String,
    cache_id: String,
    expected: SharedUploadVersion,
) -> Result<bool, String> {
    let operation = control
        .start(&operation_id, None)
        .map_err(|error| error.to_string())?;
    crate::runtime::validate_shared_upload(
        &app,
        state.inner(),
        &operation,
        &attachment_id,
        &cache_id,
        &expected.sha256,
        expected.size_bytes,
        &expected.filename,
        &expected.content_type,
        &expected.cloud_object_key,
    )
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn cleanup_shared_upload<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    cache_id: String,
) -> Result<bool, String> {
    crate::runtime::cleanup_shared_upload(&app, &cache_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn prepare_delete_guard<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    control: tauri::State<'_, crate::control::DownloadControl>,
    state: tauri::State<'_, tauri_plugin_db::ManagedState>,
    operation_id: String,
    job_id: String,
    attempt_count: i64,
    create_guard: bool,
) -> Result<PreparedDeleteGuard, String> {
    let operation = control
        .start(&operation_id, None)
        .map_err(|error| error.to_string())?;
    crate::runtime::prepare_delete_guard(
        &app,
        state.inner(),
        &operation,
        &job_id,
        attempt_count,
        create_guard,
    )
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn commit_delete_guard<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    control: tauri::State<'_, crate::control::DownloadControl>,
    state: tauri::State<'_, tauri_plugin_db::ManagedState>,
    operation_id: String,
    job_id: String,
    attempt_count: i64,
    guard_id: Option<String>,
) -> Result<(), String> {
    let operation = control
        .start(&operation_id, None)
        .map_err(|error| error.to_string())?;
    crate::runtime::commit_delete_guard(
        &app,
        state.inner(),
        &operation,
        &job_id,
        attempt_count,
        guard_id.as_deref(),
    )
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn reconcile_delete_guards<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, tauri_plugin_db::ManagedState>,
) -> Result<u64, String> {
    crate::runtime::reconcile_delete_guards(&app, state.inner())
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
    let result = crate::runtime::download_shared_attachment(
        &app,
        &operation,
        &scope_id,
        &attachment_id,
        &signed_url,
        &expected_sha256,
        expected_size_bytes,
    )
    .await
    .map_err(|error| error.to_string())?;
    // The webview loads this path through the asset protocol via
    // `convertFileSrc`. On Linux the cache lives under `~/.local/share/...`,
    // and Tauri's asset-protocol scope glob (`**/*` in tauri.conf.json)
    // never matches a path with a dot-leading component there; the
    // `$APPDATA/**` entry in the same config only covers the
    // bundle-identifier folder, which this app does not use for its data.
    // Allow this exact file instead of widening the static scope config.
    app.asset_protocol_scope()
        .allow_file(&result.local_path)
        .map_err(|error| error.to_string())?;
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn shared_attachment_path<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    scope_id: String,
    attachment_id: String,
) -> Result<Option<String>, String> {
    let path = crate::runtime::existing_shared_attachment_path(&app, &scope_id, &attachment_id)
        .await
        .map_err(|error| error.to_string())?;
    if let Some(path) = &path {
        // See download_shared_attachment above: widen the asset-protocol
        // scope for this cached attachment path explicitly, since it sits
        // under a dot-leading Linux data directory the static scope config
        // cannot match.
        app.asset_protocol_scope()
            .allow_file(path)
            .map_err(|error| error.to_string())?;
    }
    Ok(path)
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

#[tauri::command]
#[specta::specta]
pub(crate) async fn clear_shared_attachment_preview_scopes<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<bool, String> {
    let control = app.state::<crate::control::DownloadControl>();
    let clear = control
        .begin_scope_prefix_clear(crate::runtime::SHARED_PREVIEW_SCOPE_PREFIX)
        .map_err(|error| error.to_string())?;
    clear.wait().await;
    crate::runtime::clear_shared_attachment_preview_scopes(&app)
        .await
        .map_err(|error| error.to_string())
}
