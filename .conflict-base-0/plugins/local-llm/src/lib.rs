use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use tauri::Wry;
use tokio::sync::Mutex as TokioMutex;

use hypr_model_downloader::ModelDownloadManager;

mod commands;
mod error;
mod ext;
mod migrate;

pub use error::*;
pub use ext::*;
pub use hypr_local_llm_core::{
    CustomModelInfo, ModelIdentifier, ModelInfo, SUPPORTED_MODELS, SupportedModel,
};

const PLUGIN_NAME: &str = "local-llm";

pub type SharedState = std::sync::Arc<TokioMutex<State>>;

pub struct State {
    pub model_downloader: ModelDownloadManager<SupportedModel>,
    pub download_channels: Arc<Mutex<HashMap<String, tauri::ipc::Channel<i8>>>>,
    pub server: Option<hypr_local_llm_core::LlmServer>,
}

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::models_dir::<Wry>,
            commands::list_supported_model,
            commands::is_model_downloaded::<Wry>,
            commands::is_model_downloading::<Wry>,
            commands::download_model::<Wry>,
            commands::cancel_download::<Wry>,
            commands::delete_model::<Wry>,
            commands::list_downloaded_model::<Wry>,
            commands::list_custom_models::<Wry>,
            commands::server_url::<Wry>,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            use tauri::Manager as _;
            use tauri_plugin_settings::SettingsPluginExt;

            specta_builder.mount_events(app);

            let data_dir = app.settings().global_base()?.into_std_path_buf();
            let models_dir = app.local_llm().models_dir();

            migrate::legacy_gguf_files(&data_dir, &models_dir);

            let download_channels = Arc::new(Mutex::new(HashMap::new()));
            let model_downloader =
                ext::create_model_downloader(app.app_handle(), download_channels.clone());

            let state = State {
                model_downloader,
                download_channels,
                server: None,
            };
            let state = Arc::new(TokioMutex::new(state));
            app.manage(state.clone());

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
