use tauri::Manager;

use crate::{DetectPluginExt, ScreenShareSamplerState};

#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstalledApplicationIcon {
    id: String,
    data_url: String,
}

#[cfg(target_os = "macos")]
fn load_installed_application_icons(app_ids: Vec<String>) -> Vec<InstalledApplicationIcon> {
    use base64::Engine;
    use objc2::{AnyThread, msg_send};
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace};
    use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

    let workspace = NSWorkspace::sharedWorkspace();

    app_ids
        .into_iter()
        .filter_map(|id| {
            let bundle_id = NSString::from_str(&id);
            let app_url = workspace.URLForApplicationWithBundleIdentifier(&bundle_id)?;
            let app_path = app_url.path()?;
            let image = workspace.iconForFile(&app_path);

            unsafe {
                let size = NSSize::new(64.0, 64.0);
                image.setSize(size);

                let mut rect = NSRect::new(NSPoint::new(0.0, 0.0), size);
                let cg_image = image.CGImageForProposedRect_context_hints(
                    &mut rect as *mut NSRect as *mut _,
                    None,
                    None,
                )?;
                let bitmap =
                    NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &cg_image);
                let png_data = bitmap.representationUsingType_properties(
                    NSBitmapImageFileType::PNG,
                    &NSDictionary::new(),
                )?;
                let len: usize = msg_send![&*png_data, length];
                let ptr: *const u8 = msg_send![&*png_data, bytes];
                let png = std::slice::from_raw_parts(ptr, len);

                Some(InstalledApplicationIcon {
                    id,
                    data_url: format!(
                        "data:image/png;base64,{}",
                        base64::engine::general_purpose::STANDARD.encode(png)
                    ),
                })
            }
        })
        .collect()
}

fn intersect_mic_active_bundle_ids(
    requested_bundle_ids: &[String],
    current_mic_apps: &[anlg_detect::InstalledApp],
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
) -> Result<Vec<anlg_detect::InstalledApp>, String> {
    Ok(app.detect().list_installed_applications())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_installed_application_icons<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    app_ids: Vec<String>,
) -> Result<Vec<InstalledApplicationIcon>, String> {
    #[cfg(target_os = "macos")]
    {
        let (sender, receiver) = std::sync::mpsc::channel();
        app.run_on_main_thread(move || {
            let _ = sender.send(load_installed_application_icons(app_ids));
        })
        .map_err(|error| error.to_string())?;

        receiver.recv().map_err(|error| error.to_string())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, app_ids);
        Ok(Vec::new())
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn list_mic_using_applications<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<anlg_detect::InstalledApp>, crate::Error> {
    app.detect().list_mic_using_applications()
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
pub(crate) async fn inspect_meeting_accessibility<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
) -> Result<Vec<anlg_detect::MeetingAccessibilityInspection>, String> {
    Ok(anlg_detect::inspect_meeting_accessibility())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn send_meeting_chat_message<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    message: String,
    mic_active_bundle_ids: Vec<String>,
) -> Result<anlg_detect::MeetingChatSendResult, String> {
    let current_mic_apps = app
        .detect()
        .list_mic_using_applications()
        .map_err(|error| error.to_string())?;
    let verified_bundle_ids =
        intersect_mic_active_bundle_ids(&mic_active_bundle_ids, &current_mic_apps);

    Ok(anlg_detect::send_meeting_chat_message(
        message,
        verified_bundle_ids,
    ))
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn capture_meeting_chat_messages<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<anlg_detect::MeetingChatCaptureResult, String> {
    let current_mic_apps = app
        .detect()
        .list_mic_using_applications()
        .map_err(|error| error.to_string())?;
    let verified_bundle_ids = current_mic_apps
        .into_iter()
        .map(|application| application.id)
        .collect::<Vec<_>>();

    Ok(anlg_detect::capture_meeting_chat_messages(
        verified_bundle_ids,
    ))
}

#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MeetingScreenShareCapture {
    sharing: bool,
    app: Option<anlg_detect::MeetingApp>,
    platform: Option<anlg_detect::MeetingPlatform>,
    jpeg: Option<Vec<u8>>,
    width: u32,
    height: u32,
}

/// Samples the meeting window while someone else is sharing their screen and
/// returns a JPEG only when a new, settled frame appears.
#[tauri::command]
#[specta::specta]
pub(crate) async fn capture_meeting_screen_share<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    reset: bool,
) -> Result<MeetingScreenShareCapture, String> {
    let sampler = app.state::<ScreenShareSamplerState>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut sampler = sampler.lock().map_err(|error| error.to_string())?;
        if reset {
            *sampler = anlg_detect::ScreenShareSampler::default();
        }

        let Some(inspection) = anlg_detect::inspect_meeting_accessibility()
            .into_iter()
            .find(|inspection| inspection.remote_screen_share)
        else {
            sampler.pause();
            return Ok(MeetingScreenShareCapture {
                sharing: false,
                app: None,
                platform: None,
                jpeg: None,
                width: 0,
                height: 0,
            });
        };

        let image = anlg_detect::capture_meeting_window(
            inspection.pid,
            inspection.window_title.as_deref(),
        )?;
        let keep = sampler.observe(anlg_detect::FrameSignature::of(&image));
        let (jpeg, width, height) = if keep {
            let (jpeg, width, height) = anlg_detect::encode_capture_jpeg(&image)?;
            (Some(jpeg), width, height)
        } else {
            (None, 0, 0)
        };

        Ok(MeetingScreenShareCapture {
            sharing: true,
            app: Some(inspection.app),
            platform: Some(inspection.platform),
            jpeg,
            width,
            height,
        })
    })
    .await
    .map_err(|error| error.to_string())?
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

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_preferred_languages<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
) -> Result<Vec<String>, String> {
    Ok(anlg_detect::get_preferred_languages()
        .into_iter()
        .map(|l| l.bcp47_code())
        .collect())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_current_locale_identifier<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
) -> Result<String, String> {
    Ok(anlg_detect::get_current_locale_identifier())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str) -> anlg_detect::InstalledApp {
        anlg_detect::InstalledApp {
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
