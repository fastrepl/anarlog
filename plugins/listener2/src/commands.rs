use std::str::FromStr;

use hypr_listener2_core as core;

use crate::Listener2PluginExt;

#[tauri::command]
#[specta::specta]
pub async fn run_batch<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    params: core::BatchParams,
) -> Result<core::BatchRunOutput, String> {
    app.listener2()
        .run_batch(params)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn parse_subtitle<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: String,
) -> Result<core::Subtitle, String> {
    app.listener2().parse_subtitle(path)
}

#[tauri::command]
#[specta::specta]
pub async fn export_to_vtt<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
    words: Vec<core::VttWord>,
) -> Result<String, String> {
    app.listener2().export_to_vtt(session_id, words)
}

#[tauri::command]
#[specta::specta]
pub async fn run_denoise<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    params: core::DenoiseParams,
) -> Result<(), String> {
    app.listener2()
        .run_denoise(params)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn is_supported_languages_batch<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
    provider: String,
    model: Option<String>,
    languages: Vec<String>,
) -> Result<bool, String> {
    let languages_parsed = languages
        .iter()
        .map(|s| hypr_language::Language::from_str(s))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("unknown_language: {}", e))?;

    core::is_supported_languages_batch(&provider, model.as_deref(), &languages_parsed)
}

#[tauri::command]
#[specta::specta]
pub async fn suggest_providers_for_languages_batch<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
    languages: Vec<String>,
) -> Result<Vec<String>, String> {
    let languages_parsed = languages
        .iter()
        .map(|s| hypr_language::Language::from_str(s))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("unknown_language: {}", e))?;

    Ok(core::suggest_providers_for_languages_batch(
        &languages_parsed,
    ))
}

#[tauri::command]
#[specta::specta]
pub async fn list_documented_language_codes_batch<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
) -> Result<Vec<String>, String> {
    Ok(core::list_documented_language_codes_batch())
}
