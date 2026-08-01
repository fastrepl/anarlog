pub use anlg_local_model::{AmModel, AppleSpeechModel, LocalModel, SoniqoModel, WhisperModel};

pub static SUPPORTED_MODELS: &[LocalModel] = &[
    LocalModel::Soniqo(SoniqoModel::ParakeetStreaming),
    LocalModel::Soniqo(SoniqoModel::ParakeetBatch),
    LocalModel::AppleSpeech(AppleSpeechModel::Default),
    LocalModel::Am(AmModel::ParakeetV2),
    LocalModel::Am(AmModel::ParakeetV3),
    LocalModel::Am(AmModel::WhisperLargeV3),
];

#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SttModelType {
    Soniqo,
    AppleSpeech,
    Whispercpp,
    Argmax,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SttModelInfo {
    pub key: LocalModel,
    pub display_name: String,
    pub description: String,
    pub size_bytes: Option<u64>,
    pub model_type: SttModelType,
    pub supports_realtime: bool,
    pub recommended_memory_bytes: u64,
}

const GIB: u64 = 1024 * 1024 * 1024;

const fn recommended_memory_bytes(model: &LocalModel) -> u64 {
    match model {
        LocalModel::Soniqo(SoniqoModel::ParakeetStreaming) => 8 * GIB,
        LocalModel::Soniqo(SoniqoModel::ParakeetBatch | SoniqoModel::Omnilingual) => 16 * GIB,
        LocalModel::Soniqo(SoniqoModel::Qwen3Small) => 16 * GIB,
        LocalModel::Soniqo(SoniqoModel::Qwen3Large) => 32 * GIB,
        LocalModel::AppleSpeech(_) => 8 * GIB,
        LocalModel::Whisper(_) => 8 * GIB,
        LocalModel::Am(AmModel::WhisperLargeV3) => 16 * GIB,
        LocalModel::Am(AmModel::ParakeetV2 | AmModel::ParakeetV3) => 8 * GIB,
        LocalModel::GgufLlm(_) => unreachable!(),
    }
}

pub fn stt_model_info(model: &LocalModel) -> SttModelInfo {
    match model {
        LocalModel::Soniqo(value) => SttModelInfo {
            key: model.clone(),
            display_name: value.display_name().to_string(),
            description: value.description().to_string(),
            size_bytes: Some(value.size_bytes()),
            model_type: SttModelType::Soniqo,
            supports_realtime: value.supports_live(),
            recommended_memory_bytes: recommended_memory_bytes(model),
        },
        LocalModel::AppleSpeech(value) => SttModelInfo {
            key: model.clone(),
            display_name: value.display_name().to_string(),
            description: value.description().to_string(),
            // macOS installs and shares the assets, so there is nothing for us to download.
            size_bytes: None,
            model_type: SttModelType::AppleSpeech,
            supports_realtime: value.supports_live(),
            recommended_memory_bytes: recommended_memory_bytes(model),
        },
        LocalModel::Whisper(value) => SttModelInfo {
            key: model.clone(),
            display_name: value.display_name().to_string(),
            description: value.description(),
            size_bytes: Some(value.model_size_bytes()),
            model_type: SttModelType::Whispercpp,
            supports_realtime: false,
            recommended_memory_bytes: recommended_memory_bytes(model),
        },
        LocalModel::Am(value) => SttModelInfo {
            key: model.clone(),
            display_name: value.display_name().to_string(),
            description: value.description().to_string(),
            size_bytes: Some(value.model_size_bytes()),
            model_type: SttModelType::Argmax,
            supports_realtime: false,
            recommended_memory_bytes: recommended_memory_bytes(model),
        },
        LocalModel::GgufLlm(_) => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_models_include_soniqo_models_from_rust_source_of_truth() {
        let supported_soniqo_models = SUPPORTED_MODELS
            .iter()
            .filter_map(|model| match model {
                LocalModel::Soniqo(value) => Some(*value),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(supported_soniqo_models, SoniqoModel::selectable());
    }

    #[test]
    fn soniqo_model_info_comes_from_soniqo_metadata() {
        for model in SoniqoModel::all() {
            let info = stt_model_info(&LocalModel::Soniqo(*model));

            assert_eq!(info.key, LocalModel::Soniqo(*model));
            assert_eq!(info.display_name, model.display_name());
            assert_eq!(info.description, model.description());
            assert_eq!(info.size_bytes, Some(model.size_bytes()));
            assert_eq!(info.supports_realtime, model.supports_live());
            assert!(info.recommended_memory_bytes >= 8 * GIB);
            assert!(matches!(info.model_type, SttModelType::Soniqo));
        }
    }

    #[test]
    fn every_supported_model_has_unique_hardware_and_runtime_metadata() {
        let mut keys = std::collections::HashSet::new();

        for model in SUPPORTED_MODELS {
            let info = stt_model_info(model);
            let supports_realtime = match model {
                LocalModel::Soniqo(model) => model.supports_live(),
                LocalModel::AppleSpeech(model) => model.supports_live(),
                LocalModel::Whisper(_) | LocalModel::Am(_) => false,
                LocalModel::GgufLlm(_) => unreachable!(),
            };

            assert!(keys.insert(info.key.cli_name()));
            assert!(!info.display_name.trim().is_empty());
            assert!(!info.description.trim().is_empty());
            assert_eq!(info.supports_realtime, supports_realtime);
            assert!(info.recommended_memory_bytes > 0);
        }
    }
}
