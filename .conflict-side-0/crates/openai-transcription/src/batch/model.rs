use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumString};

use super::request::AudioResponseFormat;

pub const MODEL_WHISPER_1: AudioModel = AudioModel::Whisper1;
pub const MODEL_GPT_4O_TRANSCRIBE: AudioModel = AudioModel::Gpt4oTranscribe;
pub const MODEL_GPT_4O_MINI_TRANSCRIBE: AudioModel = AudioModel::Gpt4oMiniTranscribe;
pub const MODEL_GPT_4O_MINI_TRANSCRIBE_2025_12_15: AudioModel =
    AudioModel::Gpt4oMiniTranscribe20251215;
pub const MODEL_GPT_4O_TRANSCRIBE_DIARIZE: AudioModel = AudioModel::Gpt4oTranscribeDiarize;

pub fn supports_timestamp_granularities(model: impl AsRef<str>) -> bool {
    model
        .as_ref()
        .parse()
        .is_ok_and(AudioModel::supports_timestamp_granularities)
}

pub fn default_response_format(model: impl AsRef<str>) -> AudioResponseFormat {
    model
        .as_ref()
        .parse()
        .map(AudioModel::default_response_format)
        .unwrap_or(AudioResponseFormat::Json)
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumString, Display, AsRefStr,
)]
pub enum AudioModel {
    #[serde(rename = "whisper-1")]
    #[strum(serialize = "whisper-1")]
    Whisper1,
    #[serde(rename = "gpt-4o-transcribe")]
    #[strum(serialize = "gpt-4o-transcribe")]
    Gpt4oTranscribe,
    #[serde(rename = "gpt-4o-mini-transcribe")]
    #[strum(serialize = "gpt-4o-mini-transcribe")]
    Gpt4oMiniTranscribe,
    #[serde(rename = "gpt-4o-mini-transcribe-2025-12-15")]
    #[strum(serialize = "gpt-4o-mini-transcribe-2025-12-15")]
    Gpt4oMiniTranscribe20251215,
    #[serde(rename = "gpt-4o-transcribe-diarize")]
    #[strum(serialize = "gpt-4o-transcribe-diarize")]
    Gpt4oTranscribeDiarize,
}

impl AudioModel {
    pub fn supports_timestamp_granularities(self) -> bool {
        matches!(self, Self::Whisper1)
    }

    pub fn supports_streaming(self) -> bool {
        !matches!(self, Self::Whisper1)
    }

    pub fn supports_prompt(self) -> bool {
        !matches!(self, Self::Gpt4oTranscribeDiarize)
    }

    pub fn supports_logprobs(self) -> bool {
        matches!(
            self,
            Self::Gpt4oTranscribe | Self::Gpt4oMiniTranscribe | Self::Gpt4oMiniTranscribe20251215
        )
    }

    pub fn default_response_format(self) -> AudioResponseFormat {
        match self {
            Self::Whisper1 => AudioResponseFormat::VerboseJson,
            Self::Gpt4oTranscribe
            | Self::Gpt4oMiniTranscribe
            | Self::Gpt4oMiniTranscribe20251215
            | Self::Gpt4oTranscribeDiarize => AudioResponseFormat::Json,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GptTranscriptionModel {
    Gpt4oTranscribe,
    Gpt4oMiniTranscribe,
    Gpt4oMiniTranscribe20251215,
}

impl From<GptTranscriptionModel> for AudioModel {
    fn from(value: GptTranscriptionModel) -> Self {
        match value {
            GptTranscriptionModel::Gpt4oTranscribe => Self::Gpt4oTranscribe,
            GptTranscriptionModel::Gpt4oMiniTranscribe => Self::Gpt4oMiniTranscribe,
            GptTranscriptionModel::Gpt4oMiniTranscribe20251215 => Self::Gpt4oMiniTranscribe20251215,
        }
    }
}

impl TryFrom<AudioModel> for GptTranscriptionModel {
    type Error = AudioModel;

    fn try_from(value: AudioModel) -> Result<Self, Self::Error> {
        match value {
            AudioModel::Gpt4oTranscribe => Ok(Self::Gpt4oTranscribe),
            AudioModel::Gpt4oMiniTranscribe => Ok(Self::Gpt4oMiniTranscribe),
            AudioModel::Gpt4oMiniTranscribe20251215 => Ok(Self::Gpt4oMiniTranscribe20251215),
            other => Err(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_response_format_matches_model_capabilities() {
        assert_eq!(
            default_response_format(MODEL_WHISPER_1),
            AudioResponseFormat::VerboseJson
        );
        assert_eq!(
            default_response_format(MODEL_GPT_4O_TRANSCRIBE),
            AudioResponseFormat::Json
        );
    }

    #[test]
    fn audio_model_capabilities_match_api_boundaries() {
        assert!(AudioModel::Whisper1.supports_timestamp_granularities());
        assert!(!AudioModel::Whisper1.supports_streaming());
        assert!(AudioModel::Whisper1.supports_prompt());

        assert!(AudioModel::Gpt4oTranscribe.supports_streaming());
        assert!(AudioModel::Gpt4oTranscribe.supports_logprobs());
        assert!(AudioModel::Gpt4oTranscribe.supports_prompt());

        assert!(!AudioModel::Gpt4oTranscribeDiarize.supports_timestamp_granularities());
        assert!(AudioModel::Gpt4oTranscribeDiarize.supports_streaming());
        assert!(!AudioModel::Gpt4oTranscribeDiarize.supports_logprobs());
        assert!(!AudioModel::Gpt4oTranscribeDiarize.supports_prompt());
    }

    #[test]
    fn serializes_model_enum_to_api_string() {
        let json = serde_json::to_string(&MODEL_GPT_4O_MINI_TRANSCRIBE_2025_12_15)
            .expect("serialize model");

        assert_eq!(json, "\"gpt-4o-mini-transcribe-2025-12-15\"");
    }
}
