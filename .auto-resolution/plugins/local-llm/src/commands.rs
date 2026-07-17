use crate::{CustomModelInfo, LocalLlmPluginExt, ModelInfo};

use tauri::ipc::Channel;

#[tauri::command]
#[specta::specta]
pub async fn models_dir<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<String, String> {
    Ok(app.local_llm().models_dir().to_string_lossy().to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn list_supported_model() -> Result<Vec<ModelInfo>, String> {
    Ok(hypr_local_llm_core::list_supported_models())
}

#[tauri::command]
#[specta::specta]
pub async fn is_model_downloaded<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    model: crate::SupportedModel,
) -> Result<bool, String> {
    app.local_llm()
        .is_model_downloaded(&model)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn is_model_downloading<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    model: crate::SupportedModel,
) -> Result<bool, String> {
    Ok(app.local_llm().is_model_downloading(&model).await)
}

#[tauri::command]
#[specta::specta]
pub async fn download_model<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    model: crate::SupportedModel,
    channel: Channel<i8>,
) -> Result<(), String> {
    app.local_llm()
        .download_model(model, channel)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_download<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    model: crate::SupportedModel,
) -> Result<bool, String> {
    app.local_llm()
        .cancel_download(model)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_model<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    model: crate::SupportedModel,
) -> Result<(), String> {
    app.local_llm()
        .delete_model(&model)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn list_downloaded_model<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<crate::SupportedModel>, String> {
    app.local_llm()
        .list_downloaded_model()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn list_custom_models<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<CustomModelInfo>, String> {
    app.local_llm()
        .list_custom_models()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn server_url<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<String>, String> {
    app.local_llm()
        .server_url()
        .await
        .map_err(|e| e.to_string())
}
