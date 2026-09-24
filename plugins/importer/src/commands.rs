use crate::types::{
    ConnectedImportAuthorization, ConnectedImportCredentials, ConnectedImportSyncResult,
    ImportTextFile,
};

const MAX_IMPORT_FILE_COUNT: usize = 1_000;
const MAX_IMPORT_FILE_BYTES: u64 = 20 * 1024 * 1024;
const MAX_TOTAL_IMPORT_BYTES: u64 = 100 * 1024 * 1024;
const SUPPORTED_EXTENSIONS: &[&str] = &["csv", "json", "md", "markdown", "srt", "txt", "vtt"];

#[tauri::command]
#[specta::specta]
pub async fn begin_connected_import(
    provider_id: String,
    mcp_state: tauri::State<'_, crate::connected_mcp::ConnectedImportOAuthState>,
    cli_state: tauri::State<'_, crate::connected_cli::ConnectedImportCliState>,
) -> Result<ConnectedImportAuthorization, String> {
    if crate::connected_cli::is_cli_provider(&provider_id) {
        crate::connected_cli::begin_connection(&provider_id, &cli_state).await
    } else {
        crate::connected_mcp::begin_connection(&provider_id, &mcp_state).await
    }
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_connected_import(
    provider_id: String,
    mcp_state: tauri::State<'_, crate::connected_mcp::ConnectedImportOAuthState>,
    cli_state: tauri::State<'_, crate::connected_cli::ConnectedImportCliState>,
) -> Result<bool, String> {
    if crate::connected_cli::is_cli_provider(&provider_id) {
        crate::connected_cli::cancel_connection(&provider_id, &cli_state).await
    } else {
        crate::connected_mcp::cancel_connection(&provider_id, &mcp_state).await
    }
}

#[tauri::command]
#[specta::specta]
pub async fn complete_connected_import(
    provider_id: String,
    mcp_state: tauri::State<'_, crate::connected_mcp::ConnectedImportOAuthState>,
    cli_state: tauri::State<'_, crate::connected_cli::ConnectedImportCliState>,
) -> Result<ConnectedImportCredentials, String> {
    if crate::connected_cli::is_cli_provider(&provider_id) {
        crate::connected_cli::complete_connection(&provider_id, &cli_state).await
    } else {
        crate::connected_mcp::complete_connection(&provider_id, &mcp_state).await
    }
}

#[tauri::command]
#[specta::specta]
pub async fn sync_connected_import(
    provider_id: String,
    credentials: ConnectedImportCredentials,
    known_meeting_ids: Vec<String>,
) -> Result<ConnectedImportSyncResult, String> {
    if crate::connected_cli::is_cli_provider(&provider_id) {
        crate::connected_cli::sync(&provider_id, credentials, known_meeting_ids).await
    } else {
        crate::connected_mcp::sync(&provider_id, credentials, known_meeting_ids).await
    }
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
