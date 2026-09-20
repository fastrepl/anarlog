use crate::types::{
    ConnectedImportAuthorization, ConnectedImportCredentials, ConnectedImportSyncResult,
    ImportDirectoryEntry, ImportTextFile,
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

/// Lists the immediate children of a user-selected import directory, so the
/// caller can discover sibling export bundles (one folder per meeting)
/// without knowing their names in advance.
#[tauri::command]
#[specta::specta]
pub async fn list_directory_entries(path: String) -> Result<Vec<ImportDirectoryEntry>, String> {
    let path_buf = std::path::PathBuf::from(&path);
    let metadata = std::fs::metadata(&path_buf)
        .map_err(|error| format!("could not inspect {path}: {error}"))?;
    if !metadata.is_dir() {
        return Err(format!("{path} is not a directory"));
    }

    let read_dir =
        std::fs::read_dir(&path_buf).map_err(|error| format!("could not read {path}: {error}"))?;
    let mut entries = Vec::new();
    for entry in read_dir {
        let entry = entry.map_err(|error| format!("could not read {path}: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("could not inspect {path}: {error}"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        entries.push(ImportDirectoryEntry {
            path: entry.path().to_string_lossy().into_owned(),
            is_dir: file_type.is_dir(),
            name,
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp_dir(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("anlg-importer-test-{label}-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn lists_files_and_subdirectories_sorted_by_name() {
        let dir = unique_temp_dir("list-entries");
        std::fs::write(dir.join("transcript.json"), "{}").unwrap();
        std::fs::create_dir(dir.join("call-a")).unwrap();

        let entries = list_directory_entries(dir.to_string_lossy().into_owned())
            .await
            .unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "call-a");
        assert!(entries[0].is_dir);
        assert_eq!(entries[1].name, "transcript.json");
        assert!(!entries[1].is_dir);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn rejects_a_path_that_is_not_a_directory() {
        let dir = unique_temp_dir("list-entries-file");
        let file_path = dir.join("transcript.json");
        std::fs::write(&file_path, "{}").unwrap();

        let error = list_directory_entries(file_path.to_string_lossy().into_owned())
            .await
            .unwrap_err();

        assert!(error.contains("is not a directory"));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
