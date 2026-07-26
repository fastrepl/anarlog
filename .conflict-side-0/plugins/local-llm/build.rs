const COMMANDS: &[&str] = &[
    "models_dir",
    "is_model_downloaded",
    "is_model_downloading",
    "download_model",
    "cancel_download",
    "delete_model",
    "list_downloaded_model",
    "list_supported_model",
    "list_custom_models",
    "server_url",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
