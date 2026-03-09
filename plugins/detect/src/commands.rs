use crate::DetectPluginExt;

#[tauri::command]
#[specta::specta]
pub(crate) async fn list_installed_applications<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<hypr_detect::InstalledApp>, String> {
    Ok(app.detect().list_installed_applications())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn list_mic_using_applications<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<hypr_detect::InstalledApp>, String> {
    Ok(app.detect().list_mic_using_applications())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn list_default_ignored_bundle_ids<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<String>, String> {
    Ok(app.detect().list_default_ignored_bundle_ids())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn set_ignored_bundle_ids<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    bundle_ids: Vec<String>,
) -> Result<(), String> {
    app.detect().set_ignored_bundle_ids(bundle_ids);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn set_respect_do_not_disturb<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    enabled: bool,
) -> Result<(), String> {
    app.detect().set_respect_do_not_disturb(enabled);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn set_mic_active_threshold<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    secs: u64,
) -> Result<(), String> {
    app.detect().set_mic_active_threshold(secs);
    Ok(())
}

#[cfg(target_os = "macos")]
#[tauri::command]
#[specta::specta]
pub(crate) async fn get_preferred_languages<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
) -> Result<Vec<String>, String> {
    Ok(hypr_detect::get_preferred_languages()
        .into_iter()
        .map(|l| l.bcp47_code())
        .collect())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
#[specta::specta]
pub(crate) async fn get_preferred_languages<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
) -> Result<Vec<String>, String> {
    Ok(Vec::new())
}

#[cfg(target_os = "macos")]
#[tauri::command]
#[specta::specta]
pub(crate) async fn get_current_locale_identifier<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
) -> Result<String, String> {
    Ok(hypr_detect::get_current_locale_identifier())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
#[specta::specta]
pub(crate) async fn get_current_locale_identifier<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
) -> Result<String, String> {
    Ok(String::new())
}
