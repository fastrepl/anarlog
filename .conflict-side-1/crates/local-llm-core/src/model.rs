use std::path::{Path, PathBuf};

#[cfg(target_arch = "aarch64")]
pub static SUPPORTED_MODELS: &[SupportedModel] = &[
    SupportedModel::Llama3p2_3bQ4,
    SupportedModel::HyprLLM,
    SupportedModel::Gemma3_4bQ4,
];

#[cfg(not(target_arch = "aarch64"))]
pub static SUPPORTED_MODELS: &[SupportedModel] = &[];

pub use hypr_local_model::GgufLlmModel as SupportedModel;

#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ModelInfo {
    pub key: SupportedModel,
    pub name: String,
    pub description: String,
    pub size_bytes: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CustomModelInfo {
    pub path: String,
    pub name: String,
}

pub fn llm_models_dir(models_base: &Path) -> PathBuf {
    models_base.join("llm")
}

pub fn list_supported_models() -> Vec<ModelInfo> {
    vec![
        supported_model_info(&SupportedModel::HyprLLM),
        supported_model_info(&SupportedModel::Gemma3_4bQ4),
        supported_model_info(&SupportedModel::Llama3p2_3bQ4),
    ]
}

pub fn supported_model_info(model: &SupportedModel) -> ModelInfo {
    let description = match model {
        SupportedModel::HyprLLM => "Experimental model trained by the Char team.",
        SupportedModel::Gemma3_4bQ4 | SupportedModel::Llama3p2_3bQ4 => {
            "Deprecated. Exists only for backward compatibility."
        }
    };

    ModelInfo {
        key: model.clone(),
        name: model.display_name().to_string(),
        description: description.to_string(),
        size_bytes: model.model_size(),
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum ModelIdentifier {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "mock-onboarding")]
    MockOnboarding,
}
