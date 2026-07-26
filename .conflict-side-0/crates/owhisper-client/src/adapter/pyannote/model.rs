use super::language;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum PyannoteDiarizationModel {
    #[default]
    #[serde(rename = "precision-2")]
    Precision2,
    #[serde(rename = "community-1")]
    Community1,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    strum::EnumString,
    strum::AsRefStr,
)]
pub enum PyannoteTranscriptionModel {
    #[default]
    #[serde(rename = "parakeet-tdt-0.6b-v3")]
    #[strum(serialize = "parakeet-tdt-0.6b-v3")]
    ParakeetTdt06bV3,
    #[serde(rename = "faster-whisper-large-v3-turbo")]
    #[strum(serialize = "faster-whisper-large-v3-turbo")]
    FasterWhisperLargeV3Turbo,
}

impl PyannoteTranscriptionModel {
    pub fn supported_languages(&self) -> &'static [&'static str] {
        match self {
            Self::ParakeetTdt06bV3 => language::PARAKEET_TDT_06B_V3_LANGUAGES,
            Self::FasterWhisperLargeV3Turbo => language::FASTER_WHISPER_LARGE_V3_TURBO_LANGUAGES,
        }
    }

    pub fn supports_language(&self, lang: &hypr_language::Language) -> bool {
        lang.matches_any_code(self.supported_languages())
    }
}

pub(super) const TRANSCRIPTION_MODELS: &[PyannoteTranscriptionModel] = &[
    PyannoteTranscriptionModel::ParakeetTdt06bV3,
    PyannoteTranscriptionModel::FasterWhisperLargeV3Turbo,
];
