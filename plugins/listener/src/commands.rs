use owhisper_client::AdapterKind;
use std::str::FromStr;

use crate::ListenerPluginExt;
use hypr_listener_core::actors::SessionParams;

#[tauri::command]
#[specta::specta]
pub async fn list_microphone_devices<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<String>, String> {
    app.listener()
        .list_microphone_devices()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_current_microphone_device<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<String>, String> {
    app.listener()
        .get_current_microphone_device()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_mic_muted<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<bool, String> {
    Ok(app.listener().get_mic_muted().await)
}

#[tauri::command]
#[specta::specta]
pub async fn set_mic_muted<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    muted: bool,
) -> Result<(), String> {
    app.listener().set_mic_muted(muted).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn start_session<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    params: SessionParams,
) -> Result<(), String> {
    app.listener().start_session(params).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn stop_session<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.listener().stop_session().await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_state<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<hypr_listener_core::State, String> {
    Ok(app.listener().get_state().await)
}

#[tauri::command]
#[specta::specta]
pub async fn is_supported_languages_live<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
    provider: String,
    model: Option<String>,
    languages: Vec<String>,
) -> Result<bool, String> {
    if provider == "custom" {
        return Ok(true);
    }

    let languages_parsed = languages
        .iter()
        .map(|s| hypr_language::Language::from_str(s))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("unknown_language: {}", e))?;
    let adapter_kind =
        AdapterKind::from_str(&provider).map_err(|_| format!("unknown_provider: {}", provider))?;

    Ok(adapter_kind.is_supported_languages_live(&languages_parsed, model.as_deref()))
}

#[tauri::command]
#[specta::specta]
pub async fn suggest_providers_for_languages_live<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
    languages: Vec<String>,
) -> Result<Vec<String>, String> {
    let languages_parsed = languages
        .iter()
        .map(|s| hypr_language::Language::from_str(s))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("unknown_language: {}", e))?;

    let all_providers = [
        AdapterKind::Argmax,
        AdapterKind::Soniox,
        AdapterKind::Fireworks,
        AdapterKind::Deepgram,
        AdapterKind::AssemblyAI,
        AdapterKind::OpenAI,
        AdapterKind::Gladia,
        AdapterKind::ElevenLabs,
        AdapterKind::DashScope,
        AdapterKind::Mistral,
    ];

    let mut with_support: Vec<_> = all_providers
        .iter()
        .map(|kind| {
            let support = kind.language_support_live(&languages_parsed, None);
            (*kind, support)
        })
        .filter(|(_, support)| support.is_supported())
        .collect();

    with_support.sort_by(|(_, s1), (_, s2)| s2.cmp(s1));

    let supported: Vec<String> = with_support
        .into_iter()
        .map(|(kind, _)| format!("{:?}", kind).to_lowercase())
        .collect();

    Ok(supported)
}

#[tauri::command]
#[specta::specta]
pub async fn list_documented_language_codes_live<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
) -> Result<Vec<String>, String> {
    Ok(owhisper_client::documented_language_codes_live())
}
