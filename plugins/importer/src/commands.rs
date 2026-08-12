use crate::ext::ImporterPluginExt;
use crate::types::{
    ConnectedImportAuthorization, ConnectedImportCredentials, ConnectedImportSyncResult,
    ImportDataResult, ImportSourceInfo, ImportSourceKind, ImportStats, ImportTextFile,
};

const MAX_IMPORT_FILE_COUNT: usize = 1_000;
const MAX_IMPORT_FILE_BYTES: u64 = 20 * 1024 * 1024;
const MAX_TOTAL_IMPORT_BYTES: u64 = 100 * 1024 * 1024;
const SUPPORTED_EXTENSIONS: &[&str] = &["csv", "json", "md", "markdown", "srt", "txt", "vtt"];

#[tauri::command]
#[specta::specta]
pub async fn begin_connected_import(
    provider_id: String,
    state: tauri::State<'_, crate::connected_mcp::ConnectedImportOAuthState>,
) -> Result<ConnectedImportAuthorization, String> {
    crate::connected_mcp::begin_connection(&provider_id, &state).await
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_connected_import(
    provider_id: String,
    state: tauri::State<'_, crate::connected_mcp::ConnectedImportOAuthState>,
) -> Result<bool, String> {
    crate::connected_mcp::cancel_connection(&provider_id, &state).await
}

#[tauri::command]
#[specta::specta]
pub async fn complete_connected_import(
    provider_id: String,
    state: tauri::State<'_, crate::connected_mcp::ConnectedImportOAuthState>,
) -> Result<ConnectedImportCredentials, String> {
    crate::connected_mcp::complete_connection(&provider_id, &state).await
}

#[tauri::command]
#[specta::specta]
pub async fn sync_connected_import(
    provider_id: String,
    credentials: ConnectedImportCredentials,
    known_meeting_ids: Vec<String>,
) -> Result<ConnectedImportSyncResult, String> {
    crate::connected_mcp::sync(&provider_id, credentials, known_meeting_ids).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_available_sources<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<ImportSourceInfo>, String> {
    let sources = app.importer().list_available_sources();
    Ok(sources)
}

#[tauri::command]
#[specta::specta]
pub async fn run_import<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    source: ImportSourceKind,
    user_id: String,
) -> Result<ImportDataResult, String> {
    app.importer()
        .run_import(source, user_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn run_import_dry<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    source: ImportSourceKind,
) -> Result<ImportStats, String> {
    app.importer()
        .run_import_dry(source)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn read_text_files(paths: Vec<String>) -> Result<Vec<ImportTextFile>, String> {
    if paths.len() > MAX_IMPORT_FILE_COUNT {
        return Err(format!(
            "select at most {MAX_IMPORT_FILE_COUNT} files per import"
        ));
    }

    let mut total_bytes = 0_u64;
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let path_buf = std::path::PathBuf::from(&path);
        let name = path_buf
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("invalid import file path: {path}"))?
            .to_string();
        let extension = path_buf
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_lowercase)
            .ok_or_else(|| format!("{name} does not have a supported extension"))?;
        if !SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
            return Err(format!("{name} is not a supported meeting export"));
        }

        let metadata = std::fs::metadata(&path_buf)
            .map_err(|error| format!("could not inspect {name}: {error}"))?;
        if !metadata.is_file() {
            return Err(format!("{name} is not a file"));
        }
        if metadata.len() > MAX_IMPORT_FILE_BYTES {
            return Err(format!("{name} is larger than 20 MB"));
        }
        total_bytes = total_bytes.saturating_add(metadata.len());
        if total_bytes > MAX_TOTAL_IMPORT_BYTES {
            return Err("selected import files are larger than 100 MB total".to_string());
        }

        let content = std::fs::read_to_string(&path_buf)
            .map_err(|error| format!("could not read {name}: {error}"))?;
        files.push(ImportTextFile {
            path,
            name,
            content,
        });
    }

    Ok(files)
}
