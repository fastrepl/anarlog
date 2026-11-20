//! Anarlog Camera (`tools/camera-watermark`): an opt-in virtual camera that shows the person's
//! camera with an "anarlog is taking notes" badge while Anarlog records, and an automatic switch
//! to it in Zoom, Teams and Webex. macOS direct distribution only; other builds report
//! `unavailable`.

use std::path::PathBuf;
use std::sync::Arc;

use tauri::Manager;
use tauri_specta::Event;

mod camera;
mod commands;
mod error;
mod ext;
mod helper;
mod prefs;
mod receipt;
mod switch;

pub use camera::{
    AutomationSupport, CameraAvailability, CameraWatermark, CameraWatermarkState, ExtensionSetup,
    UnavailableReason,
};
pub use error::{Error, Result};
pub use ext::*;
pub use helper::{AgentRegistration, CameraAccess};
pub use prefs::{CameraBackground, CameraPrefs, CameraScene};
pub use switch::{CAMERA_NAME, CameraApp, CameraGuidance, CameraGuidanceKind, MeetCameraOutcome};

const PLUGIN_NAME: &str = "camera-watermark";
const PREFS_FILE: &str = "camera-watermark.json";

/// One notice for the person, rendered by the UI in its own words.
#[derive(Debug, Clone, serde::Serialize, specta::Type, tauri_specta::Event)]
pub struct CameraGuidanceEvent(pub CameraGuidance);

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::get_state::<tauri::Wry>,
            commands::set_enabled::<tauri::Wry>,
            commands::set_auto_switch::<tauri::Wry>,
            commands::set_background::<tauri::Wry>,
            commands::add_extension::<tauri::Wry>,
            commands::allow_camera::<tauri::Wry>,
            commands::open_extension_settings::<tauri::Wry>,
        ])
        .events(tauri_specta::collect_events![CameraGuidanceEvent])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

/// `Anarlog.app/Contents/Helpers/Anarlog Camera.app/Contents/MacOS/anarlog-camera`. A debug build
/// may point `ANARLOG_CAMERA_HELPER` at a dev bundle instead
/// (`tools/camera-watermark/bundle.noindex/...`).
fn helper_executable() -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os("ANARLOG_CAMERA_HELPER") {
        return Some(PathBuf::from(path));
    }
    let contents = std::env::current_exe()
        .ok()?
        .parent()?
        .parent()?
        .to_path_buf();
    Some(contents.join("Helpers/Anarlog Camera.app/Contents/MacOS/anarlog-camera"))
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);

            let handle = app.app_handle().clone();
            let guide = Arc::new(move |guidance: CameraGuidance| {
                if let Err(error) = CameraGuidanceEvent(guidance).emit(&handle) {
                    tracing::warn!(%error, "camera_watermark_guidance_emit_failed");
                }
            });
            let prefs_path = app
                .path()
                .app_data_dir()
                .ok()
                .map(|dir| dir.join(PREFS_FILE));
            let agent_directory = helper::agent_directory()
                .unwrap_or_else(|| std::env::temp_dir().join("so.anarlog.camera"));
            let camera =
                CameraWatermark::new(prefs_path, helper_executable(), agent_directory, guide);
            camera.launch();
            app.manage(camera);
            Ok(())
        })
        .on_event(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.camera_watermark().close();
            }
        })
        .build()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn export_types() {
        const OUTPUT_FILE: &str = "./js/bindings.gen.ts";

        make_specta_builder::<tauri::Wry>()
            .export(
                specta_typescript::Typescript::default()
                    .formatter(specta_typescript::formatter::prettier)
                    .bigint(specta_typescript::BigIntExportBehavior::Number),
                OUTPUT_FILE,
            )
            .unwrap();

        let content = std::fs::read_to_string(OUTPUT_FILE).unwrap();
        std::fs::write(OUTPUT_FILE, format!("// @ts-nocheck\n{content}")).unwrap();
    }
}
