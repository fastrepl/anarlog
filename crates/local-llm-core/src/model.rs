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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", content = "content")]
pub enum ModelSelection {
    Predefined { key: SupportedModel },
    Custom { path: String },
}

impl ModelSelection {
    pub fn file_path(&self, models_dir: &std::path::Path) -> std::path::PathBuf {
        match self {
            ModelSelection::Predefined { key } => models_dir.join(key.file_name()),
            ModelSelection::Custom { path } => std::path::PathBuf::from(path),
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            ModelSelection::Predefined { key } => match key {
                SupportedModel::Llama3p2_3bQ4 => "Llama 3.2 3B Q4".to_string(),
                SupportedModel::HyprLLM => "HyprLLM".to_string(),
                SupportedModel::Gemma3_4bQ4 => "Gemma 3 4B Q4".to_string(),
            },
            ModelSelection::Custom { path } => std::path::Path::new(path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Custom Model")
                .to_string(),
        }
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
