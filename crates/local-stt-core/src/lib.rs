pub use hypr_local_model::{AmModel, CactusSttModel, LocalModel, WhisperModel};

pub static SUPPORTED_MODELS: &[LocalModel] = &[
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
    pub size_bytes: u64,
    pub model_type: SttModelType,
}

pub fn stt_model_info(model: &LocalModel) -> SttModelInfo {
    match model {
        LocalModel::Cactus(value) => SttModelInfo {
            key: model.clone(),
            display_name: value.display_name().to_string(),
            description: value.description().to_string(),
            size_bytes: 0,
            model_type: SttModelType::Cactus,
        },
        LocalModel::Whisper(value) => SttModelInfo {
            key: model.clone(),
            display_name: value.display_name().to_string(),
            description: value.description(),
            size_bytes: value.model_size_bytes(),
            model_type: SttModelType::Whispercpp,
        },
        LocalModel::Am(value) => SttModelInfo {
            key: model.clone(),
            display_name: value.display_name().to_string(),
            description: value.description().to_string(),
            size_bytes: value.model_size_bytes(),
            model_type: SttModelType::Argmax,
        },
        LocalModel::GgufLlm(_) | LocalModel::CactusLlm(_) => unreachable!(),
    }
}
