use std::path::PathBuf;

use crate::ExportPluginExt;

#[tauri::command]
#[specta::specta]
pub(crate) async fn export<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: PathBuf,
    input: crate::ExportInput,
) -> Result<(), String> {
    app.export()
        .export_pdf(&path, input)
        .map_err(|e| e.to_string())
}
