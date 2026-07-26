#[tauri::command]
#[specta::specta]
pub(crate) fn sanitize(name: String) -> String {
    crate::sanitize::sanitize(&name)
}
