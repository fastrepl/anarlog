const COMMANDS: &[&str] = &[
    "models_dir",
    "cactus_models_dir",
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
];

fn main() {
    println!("cargo:rerun-if-env-changed=AM_API_KEY");

    tauri_plugin::Builder::new(COMMANDS).build();
}
