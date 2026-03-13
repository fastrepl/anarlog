use hypr_model_downloader::ModelDownloadManager;
use hypr_supervisor::dynamic::DynamicSupervisorMsg;
use ractor::{ActorCell, ActorRef};
use tauri::{Manager, Wry};

mod commands;
mod error;
mod ext;
mod model;
mod server;
mod types;

pub use error::*;
pub use ext::*;
pub use model::*;
pub use server::supervisor::{SUPERVISOR_NAME, SupervisorRef};
pub use server::*;
pub use types::*;

pub type SharedState = std::sync::Arc<tokio::sync::Mutex<State>>;
pub type SupervisorHandle = tokio::task::JoinHandle<()>;

pub struct State {
    pub am_api_key: Option<String>,
    pub stt_supervisor: Option<ActorRef<DynamicSupervisorMsg>>,
    pub supervisor_handle: Option<SupervisorHandle>,
    pub model_downloader: ModelDownloadManager<LocalModel>,
}

#[derive(Default)]
pub struct InitOptions {
    pub parent_supervisor: Option<ActorCell>,
}

const PLUGIN_NAME: &str = "local-stt";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::models_dir::<Wry>,
            commands::cactus_models_dir::<Wry>,
            commands::is_model_downloaded::<Wry>,
            commands::is_model_downloading::<Wry>,
            commands::download_model::<Wry>,
            commands::cancel_download::<Wry>,
            commands::delete_model::<Wry>,
            commands::get_server_for_model::<Wry>,
            commands::get_servers::<Wry>,
            commands::start_server::<Wry>,
            commands::stop_server::<Wry>,
            commands::list_supported_models,
        ])
        .events(tauri_specta::collect_events![
            types::DownloadProgressPayload,
        ])
        .typ::<hypr_whisper_local_model::WhisperModel>()
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>(options: InitOptions) -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);

            let api_key = option_env!("AM_API_KEY").map(|s| s.to_string());

            let model_downloader = ext::create_model_downloader(app.app_handle());

            let state = std::sync::Arc::new(tokio::sync::Mutex::new(State {
                am_api_key: api_key,
                stt_supervisor: None,
                supervisor_handle: None,
                model_downloader,
            }));

            app.manage(state.clone());

            let parent = options.parent_supervisor.clone();
            tauri::async_runtime::spawn(async move {
                match server::supervisor::spawn_stt_supervisor(parent).await {
                    Ok((supervisor, handle)) => {
                        let mut guard = state.lock().await;
                        guard.stt_supervisor = Some(supervisor);
                        guard.supervisor_handle = Some(handle);
                        tracing::info!("stt_supervisor_spawned");
                    }
                    Err(e) => {
                        tracing::error!("failed_to_spawn_stt_supervisor: {:?}", e);
                    }
                }
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
