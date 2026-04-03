use std::sync::Arc;

use ractor::Actor;
use tauri::Manager;
use tokio::sync::Mutex;

mod error;
mod listener;
mod listener2;

pub use error::{DegradedError, Error, Result};
pub mod core {
    pub use hypr_transcription_core::{listener, listener2};
}
pub use core::listener::{
    InMemoryRecordingDisposition, ListenerRuntime, LiveTranscriptDelta, LiveTranscriptEngine,
    LiveTranscriptSegment, LiveTranscriptSegmentDelta, LiveTranscriptUpdate, RecordingMode,
    SessionDataEvent, SessionErrorEvent, SessionLifecycleEvent, SessionProgressEvent,
    StartSessionError, State, StopSessionParams, TranscriptionMode, actors::SessionParams,
};
pub use core::listener2::{
    BatchErrorCode, BatchEvent, BatchFailure, BatchParams, BatchProvider, BatchRunMode,
    BatchRunOutput, BatchRuntime, DenoiseEvent, DenoiseParams, DenoiseRuntime,
    Error as Listener2Error, Result as Listener2Result, Subtitle, Token, VttWord,
    export_words_to_vtt_file, is_supported_languages_batch, list_documented_language_codes_batch,
    parse_subtitle_from_path, run_batch, run_denoise, suggest_providers_for_languages_batch,
};
pub use listener::{Listener, ListenerPluginExt};
pub use listener2::{Listener2, Listener2PluginExt};

use hypr_audio::AudioProvider;
use hypr_transcription_core::listener::actors::{RootActor, RootArgs};

const PLUGIN_NAME: &str = "transcription";

pub type SharedState = Arc<Mutex<PluginState>>;

pub struct PluginState {
    pub app: tauri::AppHandle,
}

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            listener::commands::list_microphone_devices::<tauri::Wry>,
            listener::commands::get_current_microphone_device::<tauri::Wry>,
            listener::commands::get_mic_muted::<tauri::Wry>,
            listener::commands::set_mic_muted::<tauri::Wry>,
            listener::commands::start_session::<tauri::Wry>,
            listener::commands::stop_session::<tauri::Wry>,
            listener::commands::get_state::<tauri::Wry>,
            listener::commands::is_supported_languages_live::<tauri::Wry>,
            listener::commands::suggest_providers_for_languages_live::<tauri::Wry>,
            listener::commands::list_documented_language_codes_live::<tauri::Wry>,
            listener::commands::render_transcript_segments,
            listener2::commands::run_batch::<tauri::Wry>,
            listener2::commands::run_denoise::<tauri::Wry>,
            listener2::commands::parse_subtitle::<tauri::Wry>,
            listener2::commands::export_to_vtt::<tauri::Wry>,
            listener2::commands::is_supported_languages_batch::<tauri::Wry>,
            listener2::commands::suggest_providers_for_languages_batch::<tauri::Wry>,
            listener2::commands::list_documented_language_codes_batch::<tauri::Wry>,
        ])
        .events(tauri_specta::collect_events![
            SessionLifecycleEvent,
            SessionProgressEvent,
            SessionErrorEvent,
            SessionDataEvent,
            BatchEvent,
            DenoiseEvent
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);

            let app_handle = app.app_handle().clone();
            let state: SharedState = Arc::new(Mutex::new(PluginState {
                app: app_handle.clone(),
            }));
            app.manage(state);

            let audio = app.state::<Arc<dyn AudioProvider>>().inner().clone();
            let runtime = Arc::new(listener::TauriRuntime {
                app: app_handle.clone(),
            });

            tauri::async_runtime::spawn(async move {
                Actor::spawn(
                    Some(RootActor::name()),
                    RootActor,
                    RootArgs { runtime, audio },
                )
                .await
                .map(|_| tracing::info!("root_actor_spawned"))
                .map_err(|e| tracing::error!(?e, "failed_to_spawn_root_actor"))
            });

            Ok(())
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
