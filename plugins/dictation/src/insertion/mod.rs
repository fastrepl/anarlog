#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::{capture, insert};
#[cfg(target_os = "windows")]
pub use windows::{capture, insert};

#[cfg(target_os = "macos")]
pub async fn capture() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(anlg_dictation_ui_macos::capture_target)
        .await
        .map_err(|error| error.to_string())?
}

#[cfg(target_os = "macos")]
pub async fn insert(target: String, text: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        anlg_dictation_ui_macos::insert_text(&target, &text)
    })
    .await
    .map_err(|error| error.to_string())?
}
