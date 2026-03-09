use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use tauri::Wry;
use tokio::sync::Mutex as TokioMutex;

use hypr_model_downloader::ModelDownloadManager;

mod commands;
mod error;
mod ext;
mod store;

pub use error::*;
pub use ext::*;
pub use hypr_local_llm_core::{
    CustomModelInfo, ModelIdentifier, ModelInfo, ModelSelection, SUPPORTED_MODELS, SupportedModel,
};
pub use store::*;

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
            commands::get_current_model::<Wry>,
            commands::set_current_model::<Wry>,
            commands::list_downloaded_model::<Wry>,
            commands::list_custom_models::<Wry>,
            commands::get_current_model_selection::<Wry>,
            commands::set_current_model_selection::<Wry>,
            commands::start_server::<Wry>,
            commands::stop_server::<Wry>,
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
            use tauri::path::BaseDirectory;
            use tauri_plugin_settings::SettingsPluginExt;

            specta_builder.mount_events(app);

            let data_dir = app.settings().global_base()?.into_std_path_buf();
            let models_dir = app.models_dir();

            // for backward compatibility
            {
                let _ = std::fs::create_dir_all(&models_dir);

                if let Ok(entries) = std::fs::read_dir(&data_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|ext| ext.to_str()) == Some("gguf") {
                            let new_path = models_dir.join(path.file_name().unwrap());
                            let _ = std::fs::rename(path, new_path);
                        }
                    }
                }
            }

            {
                let _model_path = if cfg!(debug_assertions) {
                    app.path()
                        .resolve("resources/llm.gguf", BaseDirectory::Resource)?
                } else {
                    app.path().resolve("llm.gguf", BaseDirectory::Resource)?
                };

                let download_channels = Arc::new(Mutex::new(HashMap::new()));
                let model_downloader =
                    ext::create_model_downloader(app.app_handle(), download_channels.clone());

                let state = State {
                    model_downloader,
                    download_channels,
                    server: None,
                };
                app.manage(Arc::new(TokioMutex::new(state)));
            }

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
