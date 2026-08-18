use std::sync::{Arc, Mutex};

use tauri::Manager;

mod commands;
mod competitor_monitor;
mod dnd;
mod error;
mod events;
mod ext;
mod manifest;
mod mic_usage_tracker;
mod timer_registry;

#[cfg(feature = "test-support")]
pub mod env;
#[cfg(not(feature = "test-support"))]
mod env;

#[cfg(feature = "test-support")]
pub mod handler;
#[cfg(not(feature = "test-support"))]
mod handler;

#[cfg(feature = "test-support")]
pub mod policy;
#[cfg(not(feature = "test-support"))]
mod policy;

pub use dnd::*;
pub use error::*;
pub use events::*;
pub use ext::*;
pub use mic_usage_tracker::MicUsageTracker;
pub use policy::*;

const PLUGIN_NAME: &str = "detect";

pub(crate) type DetectorState = Mutex<anlg_detect::Detector>;

#[cfg(feature = "test-support")]
pub type ProcessorState = Arc<Mutex<Processor>>;
#[cfg(not(feature = "test-support"))]
pub(crate) type ProcessorState = Arc<Mutex<Processor>>;

pub struct Processor {
    pub policy: policy::MicNotificationPolicy,
    pub mic_usage_tracker: mic_usage_tracker::MicUsageTracker,
    pub mic_active_threshold_secs: u64,
}

impl Default for Processor {
    fn default() -> Self {
        Self {
            policy: Default::default(),
            mic_usage_tracker: Default::default(),
            mic_active_threshold_secs: mic_usage_tracker::DEFAULT_MIC_ACTIVE_THRESHOLD_SECS,
        }
    }
}

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::list_installed_applications::<tauri::Wry>,
            commands::get_installed_application_icons::<tauri::Wry>,
            commands::terminate_competing_applications::<tauri::Wry>,
            commands::set_competing_application_termination_paused::<tauri::Wry>,
            commands::list_mic_using_applications::<tauri::Wry>,
            commands::set_respect_do_not_disturb::<tauri::Wry>,
            commands::set_ignored_bundle_ids::<tauri::Wry>,
            commands::set_included_bundle_ids::<tauri::Wry>,
            commands::list_default_ignored_bundle_ids::<tauri::Wry>,
            commands::inspect_meeting_accessibility::<tauri::Wry>,
            commands::send_meeting_chat_message::<tauri::Wry>,
            commands::capture_meeting_chat_messages::<tauri::Wry>,
            commands::get_preferred_languages::<tauri::Wry>,
            commands::get_current_locale_identifier::<tauri::Wry>,
            commands::set_mic_active_threshold::<tauri::Wry>,
        ])
        .events(tauri_specta::collect_events![DetectEvent])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);

            app.manage(DetectorState::default());
            app.manage(ProcessorState::default());

            let competitor_termination_state =
                competitor_monitor::CompetitorTerminationState::default();
            app.manage(competitor_termination_state.clone());

            #[cfg(not(any(test, feature = "test-support")))]
            competitor_monitor::start(competitor_termination_state);

            let app_handle = app.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                handler::setup(&app_handle).unwrap();
            });

            Ok(())
        })
        .build()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn command_manifest_is_single_source() {
        let mut manifest_commands: Vec<String> = crate::manifest::COMMANDS
            .iter()
            .map(|command| command.to_string())
            .collect();
        manifest_commands.sort_unstable();

        // Export to a private path: export_types rewrites js/bindings.gen.ts
        // concurrently, so reading the shared file here would race with it.
        let bindings_path = std::env::temp_dir().join(format!(
            "anlg-{PLUGIN_NAME}-manifest-sync-{}.ts",
            std::process::id()
        ));
        make_specta_builder::<tauri::Wry>()
            .export(
                specta_typescript::Typescript::default()
                    .bigint(specta_typescript::BigIntExportBehavior::Number),
                &bindings_path,
            )
            .unwrap();
        let bindings = std::fs::read_to_string(&bindings_path).unwrap();
        let _ = std::fs::remove_file(&bindings_path);
        let prefix = format!("plugin:{PLUGIN_NAME}|");
        let mut binding_commands: Vec<String> = bindings
            .split(prefix.as_str())
            .skip(1)
            .filter_map(|rest| rest.split('"').next().map(str::to_string))
            .collect();
        binding_commands.sort_unstable();
        binding_commands.dedup();
        assert_eq!(
            binding_commands, manifest_commands,
            "generated bindings drifted from src/manifest.rs"
        );

        let mut permission_commands: Vec<String> =
            std::fs::read_dir("./permissions/autogenerated/commands")
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
                .filter_map(|path| {
                    path.file_stem()
                        .map(|stem| stem.to_string_lossy().into_owned())
                })
                .collect();
        permission_commands.sort_unstable();
        assert_eq!(
            permission_commands, manifest_commands,
            "generated permissions drifted from src/manifest.rs"
        );
    }

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

    fn create_app<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::App<R> {
        builder
            .plugin(init())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap()
    }

    #[test]
    fn test_detect() {
        let _app = create_app(tauri::test::mock_builder());
    }
}
