use std::path::{Path, PathBuf};

use crate::Error;

#[cfg(target_arch = "aarch64")]
pub static SUPPORTED_MODELS: &[SupportedModel] = &[
    SupportedModel::Llama3p2_3bQ4,
    SupportedModel::HyprLLM,
    SupportedModel::Gemma3_4bQ4,
];

#[cfg(not(target_arch = "aarch64"))]
pub static SUPPORTED_MODELS: &[SupportedModel] = &[];

pub use hypr_local_model::GgufLlmModel as SupportedModel;
use hypr_local_model::{CactusLlmModel, CactusModelSource};

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

pub fn cactus_models_dir(models_base: &Path) -> PathBuf {
    models_base.join("cactus")
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", content = "content")]
pub enum ModelSelection {
    Predefined { key: SupportedModel },
    Cactus { key: CactusLlmModel },
    Custom { path: String },
}

impl ModelSelection {
    pub fn ensure_supported(&self) -> Result<(), Error> {
        if let ModelSelection::Cactus { key } = self
            && matches!(key.source(), CactusModelSource::Unavailable)
        {
            return Err(Error::Other(format!(
                "Cactus model {} is not available in this build",
                key.asset_id()
            )));
        }

        Ok(())
    }

    pub fn install_path(&self, models_base: &Path) -> PathBuf {
        match self {
            ModelSelection::Predefined { key } => llm_models_dir(models_base).join(key.file_name()),
            ModelSelection::Cactus { key } => cactus_models_dir(models_base).join(key.dir_name()),
            ModelSelection::Custom { path } => PathBuf::from(path),
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            ModelSelection::Predefined { key } => match key {
                SupportedModel::Llama3p2_3bQ4 => "Llama 3.2 3B Q4".to_string(),
                SupportedModel::HyprLLM => "HyprLLM".to_string(),
                SupportedModel::Gemma3_4bQ4 => "Gemma 3 4B Q4".to_string(),
            },
            ModelSelection::Cactus { key } => key.display_name().to_string(),
            ModelSelection::Custom { path } => std::path::Path::new(path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Custom Model")
                .to_string(),
        }
    }

    pub fn resolve_path(
        &self,
        models_base: &Path,
        mut resolve_resource: impl FnMut(&str) -> Result<Option<PathBuf>, Error>,
    ) -> Result<PathBuf, Error> {
        self.ensure_supported()?;

        match self {
            ModelSelection::Cactus { key } => match key.source() {
                CactusModelSource::BundledResource { relative_path } => {
                    resolve_resource(relative_path)?.ok_or_else(|| {
                        Error::Other(format!(
                            "Bundled Cactus model resource not found for {}",
                            key.asset_id()
                        ))
                    })
                }
                CactusModelSource::Downloadable { .. } | CactusModelSource::Unavailable => {
                    Ok(self.install_path(models_base))
                }
            },
            ModelSelection::Predefined { .. } | ModelSelection::Custom { .. } => {
                Ok(self.install_path(models_base))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cactus_selection_uses_cactus_models_directory() {
        let models_base = std::path::Path::new("/tmp/models");
        let selection = ModelSelection::Cactus {
            key: CactusLlmModel::Lfm2Vl450mApple,
        };

        assert_eq!(
            selection.install_path(models_base),
            std::path::PathBuf::from("/tmp/models/cactus/lfm2-vl-450m-apple")
        );
    }

    #[test]
    fn cactus_selection_display_name_uses_model_metadata() {
        let selection = ModelSelection::Cactus {
            key: CactusLlmModel::Lfm2Vl450mApple,
        };

        assert_eq!(selection.display_name(), "LFM2 VL (450M, Apple NPU)");
    }

    #[test]
    fn bundled_cactus_selection_uses_resolved_resource_path() {
        let models_base = std::path::Path::new("/tmp/models");
        let selection = ModelSelection::Cactus {
            key: CactusLlmModel::Lfm2Vl450mApple,
        };

        let path = selection
            .resolve_path(models_base, |relative_path| {
                Ok(Some(std::path::PathBuf::from(format!(
                    "/bundle/{relative_path}"
                ))))
            })
            .unwrap();

        assert_eq!(
            path,
            std::path::PathBuf::from("/bundle/models/cactus/char-vlm/weight")
        );
    }

    #[test]
    fn predefined_selection_install_path_uses_llm_directory() {
        let models_base = std::path::Path::new("/tmp/models");
        let selection = ModelSelection::Predefined {
            key: SupportedModel::HyprLLM,
        };

        assert_eq!(
            selection.install_path(models_base),
            std::path::PathBuf::from("/tmp/models/llm/hypr-llm.gguf")
        );
    }

    #[test]
    fn unavailable_cactus_selection_is_rejected() {
        let selection = ModelSelection::Cactus {
            key: CactusLlmModel::Lfm2_5Vl1_6bApple,
        };

        assert!(selection.ensure_supported().is_err());
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
