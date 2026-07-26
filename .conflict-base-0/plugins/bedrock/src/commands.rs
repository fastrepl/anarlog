use crate::BedrockPluginExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ListFoundationModelsRequest {
    pub by_provider: Option<String>,
    pub by_customization_type: Option<String>,
    pub by_output_modality: Option<String>,
    pub by_inference_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FoundationModelSummary {
    pub model_id: Option<String>,
    pub model_name: Option<String>,
    pub provider_name: Option<String>,
    pub input_modalities: Option<Vec<String>>,
    pub output_modalities: Option<Vec<String>>,
    pub response_streaming_supported: Option<bool>,
    pub customizations_supported: Option<Vec<String>>,
    pub inference_types_supported: Option<Vec<String>>,
    pub model_arn: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ListFoundationModelsResponse {
    pub model_summaries: Vec<FoundationModelSummary>,
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn list_foundation_models<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: ListFoundationModelsRequest,
) -> Result<ListFoundationModelsResponse, String> {
    app.bedrock()
        .list_foundation_models(request)
        .await
        .map_err(|e| e.to_string())
}
