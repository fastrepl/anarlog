#[cfg(target_os = "macos")]
mod ext;
#[cfg(target_os = "macos")]
mod menu_items;

#[cfg(target_os = "macos")]
static APP_HANDLE: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

const PLUGIN_NAME: &str = "dock";

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::<tauri::Wry>::new(PLUGIN_NAME)
        .setup(|_app, _api| {
            #[cfg(target_os = "macos")]
            ext::setup_dock_menu(_app)?;

            Ok(())
        })
        .build()
}
