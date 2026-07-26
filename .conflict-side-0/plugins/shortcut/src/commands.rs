use crate::{
    events::{HotKey, Options},
    ext::ShortcutPluginExt,
};

#[tauri::command]
#[specta::specta]
pub(crate) async fn register_hotkey<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    hotkey: HotKey,
    options: Options,
) -> Result<(), String> {
    app.shortcut()
        .register(hotkey, options)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn unregister_hotkey<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<(), String> {
    app.shortcut().unregister().map_err(|e| e.to_string())
}
