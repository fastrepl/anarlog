const COMMANDS: &[&str] = &[
    "models_dir",
    "is_model_downloaded",
    "is_model_downloading",
    "download_model",
    "cancel_download",
    "delete_model",
    "start_server",
    "stop_server",
    "get_server_for_model",
    "get_servers",
    "list_supported_models",
    "list_supported_languages",
    "inspect_custom_model_path",
    "start_server_for_path",
];

fn main() {
    println!("cargo:rerun-if-env-changed=AM_API_KEY");

    tauri_plugin::Builder::new(COMMANDS).build();
}
