pub use hypr_local_model::{AmModel, CactusSttModel, LocalModel, SoniqoModel, WhisperModel};

pub static SUPPORTED_MODELS: &[LocalModel] = &[
    LocalModel::Soniqo(SoniqoModel::ParakeetStreaming),
    LocalModel::Soniqo(SoniqoModel::ParakeetBatch),
    LocalModel::Soniqo(SoniqoModel::Omnilingual),
    LocalModel::Soniqo(SoniqoModel::Qwen3Small),
    LocalModel::Soniqo(SoniqoModel::Qwen3Large),
    LocalModel::Am(AmModel::ParakeetV2),
    LocalModel::Am(AmModel::ParakeetV3),
    LocalModel::Am(AmModel::WhisperLargeV3),
    LocalModel::Cactus(CactusSttModel::WhisperSmallInt4),
    LocalModel::Cactus(CactusSttModel::WhisperSmallInt4Apple),
    LocalModel::Cactus(CactusSttModel::WhisperSmallInt8),
    LocalModel::Cactus(CactusSttModel::WhisperSmallInt8Apple),
    LocalModel::Cactus(CactusSttModel::ParakeetTdt0_6bV3Int4),
    LocalModel::Cactus(CactusSttModel::ParakeetTdt0_6bV3Int4Apple),
    LocalModel::Cactus(CactusSttModel::ParakeetTdt0_6bV3Int8),
    LocalModel::Cactus(CactusSttModel::ParakeetTdt0_6bV3Int8Apple),
];

#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SttModelType {
    Soniqo,
    Cactus,
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
}

pub fn stt_model_info(model: &LocalModel) -> SttModelInfo {
    match model {
        LocalModel::Soniqo(value) => SttModelInfo {
            key: model.clone(),
            display_name: value.display_name().to_string(),
            description: value.description().to_string(),
            size_bytes: Some(value.size_bytes()),
            model_type: SttModelType::Soniqo,
        },
        LocalModel::Cactus(value) => SttModelInfo {
            key: model.clone(),
            display_name: value.display_name().to_string(),
            description: value.description().to_string(),
            size_bytes: value.model_size_bytes(),
            model_type: SttModelType::Cactus,
        },
        LocalModel::Whisper(value) => SttModelInfo {
            key: model.clone(),
            display_name: value.display_name().to_string(),
            description: value.description(),
            size_bytes: Some(value.model_size_bytes()),
            model_type: SttModelType::Whispercpp,
        },
        LocalModel::Am(value) => SttModelInfo {
            key: model.clone(),
            display_name: value.display_name().to_string(),
            description: value.description().to_string(),
            size_bytes: Some(value.model_size_bytes()),
            model_type: SttModelType::Argmax,
        },
        LocalModel::GgufLlm(_) | LocalModel::CactusLlm(_) => unreachable!(),
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

        assert_eq!(supported_soniqo_models, SoniqoModel::all());
    }

    #[test]
    fn soniqo_model_info_comes_from_soniqo_metadata() {
        for model in SoniqoModel::all() {
            let info = stt_model_info(&LocalModel::Soniqo(*model));

            assert_eq!(info.key, LocalModel::Soniqo(*model));
            assert_eq!(info.display_name, model.display_name());
            assert_eq!(info.description, model.description());
            assert_eq!(info.size_bytes, Some(model.size_bytes()));
            assert!(matches!(info.model_type, SttModelType::Soniqo));
        }
    }
}
