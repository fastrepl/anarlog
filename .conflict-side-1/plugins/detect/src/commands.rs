use crate::DetectPluginExt;

fn intersect_mic_active_bundle_ids(
    requested_bundle_ids: &[String],
    current_mic_apps: &[hypr_detect::InstalledApp],
) -> Vec<String> {
    let requested = requested_bundle_ids
        .iter()
        .map(|bundle_id| bundle_id.trim())
        .filter(|bundle_id| !bundle_id.is_empty())
        .collect::<std::collections::HashSet<_>>();
    let mut verified = current_mic_apps
        .iter()
        .map(|app| app.id.trim())
        .filter(|bundle_id| requested.contains(bundle_id))
        .map(str::to_string)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    verified.sort();
    verified
}

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
) -> Result<Vec<hypr_detect::InstalledApp>, crate::Error> {
    app.detect().list_mic_using_applications()
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn list_default_ignored_bundle_ids<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<String>, String> {
    Ok(app.detect().list_default_ignored_bundle_ids())
}

#[cfg(target_os = "macos")]
#[tauri::command]
#[specta::specta]
pub(crate) async fn inspect_meeting_accessibility<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
) -> Result<Vec<hypr_detect::MeetingAccessibilityInspection>, String> {
    Ok(hypr_detect::inspect_meeting_accessibility())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn send_meeting_chat_message<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    message: String,
    mic_active_bundle_ids: Vec<String>,
) -> Result<hypr_detect::MeetingChatSendResult, String> {
    let current_mic_apps = app
        .detect()
        .list_mic_using_applications()
        .map_err(|error| error.to_string())?;
    let verified_bundle_ids =
        intersect_mic_active_bundle_ids(&mic_active_bundle_ids, &current_mic_apps);

    Ok(hypr_detect::send_meeting_chat_message(
        message,
        verified_bundle_ids,
    ))
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn capture_meeting_chat_messages<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    bundle_ids: Vec<String>,
) -> Result<hypr_detect::MeetingChatCaptureResult, String> {
    let current_mic_apps = app
        .detect()
        .list_mic_using_applications()
        .map_err(|error| error.to_string())?;
    let verified_bundle_ids = intersect_mic_active_bundle_ids(&bundle_ids, &current_mic_apps);

    Ok(hypr_detect::capture_meeting_chat_messages(
        verified_bundle_ids,
    ))
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
#[specta::specta]
pub(crate) async fn inspect_meeting_accessibility<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
) -> Result<Vec<hypr_detect::MeetingAccessibilityInspection>, String> {
    Ok(Vec::new())
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
pub(crate) async fn set_included_bundle_ids<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    bundle_ids: Vec<String>,
) -> Result<(), String> {
    app.detect().set_included_bundle_ids(bundle_ids);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str) -> hypr_detect::InstalledApp {
        hypr_detect::InstalledApp {
            id: id.to_string(),
            name: id.to_string(),
        }
    }

    #[test]
    fn meeting_ax_scope_uses_every_requested_current_mic_app_once() {
        let requested = vec![
            "com.microsoft.teams2".to_string(),
            "us.zoom.xos".to_string(),
            "com.google.Chrome".to_string(),
        ];
        let current = vec![
            app("us.zoom.xos"),
            app("com.microsoft.teams2"),
            app("com.google.Chrome"),
            app("us.zoom.xos"),
            app("com.tinyspeck.slackmacgap"),
        ];

        assert_eq!(
            intersect_mic_active_bundle_ids(&requested, &current),
            vec!["com.google.Chrome", "com.microsoft.teams2", "us.zoom.xos"]
        );
    }

    #[test]
    fn meeting_ax_scope_rejects_stale_or_forged_bundle_ids() {
        let requested = vec!["com.tinyspeck.slackmacgap".to_string()];
        let current = vec![app("us.zoom.xos")];

        assert!(intersect_mic_active_bundle_ids(&requested, &current).is_empty());
    }

    #[test]
    fn meeting_ax_scope_drops_empty_bundle_ids() {
        let requested = vec!["".to_string(), "  ".to_string(), "us.zoom.xos".to_string()];
        let current = vec![app(""), app("  "), app("us.zoom.xos")];

        assert_eq!(
            intersect_mic_active_bundle_ids(&requested, &current),
            vec!["us.zoom.xos"]
        );
    }
}
