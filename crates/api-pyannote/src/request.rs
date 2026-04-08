use hypr_pyannote_cloud::types;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DiarizeRequest {
    #[serde(default)]
    pub confidence: bool,
    #[serde(default)]
    pub exclusive: bool,
    #[serde(
        rename = "maxSpeakers",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_speakers: Option<f64>,
    #[serde(
        rename = "minSpeakers",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub min_speakers: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<DiarizeRequestModel>,
    #[serde(
        rename = "numSpeakers",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub num_speakers: Option<f64>,
    #[serde(default)]
    pub transcription: bool,
    #[serde(
        rename = "transcriptionConfig",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub transcription_config: Option<TranscriptionConfiguration>,
    #[serde(
        rename = "turnLevelConfidence",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub turn_level_confidence: Option<bool>,
    pub url: String,
}

impl From<DiarizeRequest> for types::DiarizeRequest {
    fn from(value: DiarizeRequest) -> Self {
        Self {
            confidence: value.confidence,
            exclusive: value.exclusive,
            max_speakers: value.max_speakers,
            min_speakers: value.min_speakers,
            model: value.model.map(Into::into),
            num_speakers: value.num_speakers,
            transcription: value.transcription,
            transcription_config: value.transcription_config.map(Into::into),
            turn_level_confidence: value.turn_level_confidence,
            url: value.url,
            webhook: None,
            webhook_status_only: false,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub enum DiarizeRequestModel {
    #[serde(rename = "precision-2")]
    Precision2,
    #[serde(rename = "community-1")]
    Community1,
}

impl From<DiarizeRequestModel> for types::DiarizeRequestModel {
    fn from(value: DiarizeRequestModel) -> Self {
        match value {
            DiarizeRequestModel::Precision2 => Self::Precision2,
            DiarizeRequestModel::Community1 => Self::Community1,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct IdentifyRequest {
    #[serde(default)]
    pub confidence: bool,
    #[serde(default)]
    pub exclusive: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matching: Option<MatchingOptions>,
    #[serde(
        rename = "maxSpeakers",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_speakers: Option<f64>,
    #[serde(
        rename = "minSpeakers",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub min_speakers: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<IdentifyRequestModel>,
    #[serde(
        rename = "numSpeakers",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub num_speakers: Option<f64>,
    #[serde(
        rename = "turnLevelConfidence",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub turn_level_confidence: Option<bool>,
    pub url: String,
    pub voiceprints: Vec<Voiceprint>,
}

impl From<IdentifyRequest> for types::IdentifyRequest {
    fn from(value: IdentifyRequest) -> Self {
        Self {
            confidence: value.confidence,
            exclusive: value.exclusive,
            matching: value.matching.map(Into::into),
            max_speakers: value.max_speakers,
            min_speakers: value.min_speakers,
            model: value.model.map(Into::into),
            num_speakers: value.num_speakers,
            turn_level_confidence: value.turn_level_confidence,
            url: Some(value.url),
            voiceprints: value.voiceprints.into_iter().map(Into::into).collect(),
            webhook: None,
            webhook_status_only: false,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub enum IdentifyRequestModel {
    #[serde(rename = "precision-2")]
    Precision2,
}

impl From<IdentifyRequestModel> for types::IdentifyRequestModel {
    fn from(value: IdentifyRequestModel) -> Self {
        match value {
            IdentifyRequestModel::Precision2 => Self::Precision2,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct MatchingOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclusive: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f32>,
}

impl From<MatchingOptions> for types::MatchingOptions {
    fn from(value: MatchingOptions) -> Self {
        Self {
            exclusive: value.exclusive,
            threshold: value.threshold,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct TranscriptionConfiguration {
    pub model: TranscriptionConfigurationModel,
}

impl From<TranscriptionConfiguration> for types::TranscriptionConfiguration {
    fn from(value: TranscriptionConfiguration) -> Self {
        Self {
            model: value.model.into(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub enum TranscriptionConfigurationModel {
    #[serde(rename = "parakeet-tdt-0.6b-v3")]
    ParakeetTdt06bV3,
    #[serde(rename = "faster-whisper-large-v3-turbo")]
    FasterWhisperLargeV3Turbo,
}

impl From<TranscriptionConfigurationModel> for types::TranscriptionConfigurationModel {
    fn from(value: TranscriptionConfigurationModel) -> Self {
        match value {
            TranscriptionConfigurationModel::ParakeetTdt06bV3 => Self::ParakeetTdt06bV3,
            TranscriptionConfigurationModel::FasterWhisperLargeV3Turbo => {
                Self::FasterWhisperLargeV3Turbo
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct Voiceprint {
    pub label: String,
    pub voiceprint: String,
}

impl From<Voiceprint> for types::Voiceprint {
    fn from(value: Voiceprint) -> Self {
        Self {
            label: value.label.try_into().expect("validated voiceprint label"),
            voiceprint: value.voiceprint,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VoiceprintRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<VoiceprintRequestModel>,
    pub url: String,
}

impl From<VoiceprintRequest> for types::VoiceprintRequest {
    fn from(value: VoiceprintRequest) -> Self {
        Self {
            model: value.model.map(Into::into),
            url: value.url,
            webhook: None,
            webhook_status_only: false,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub enum VoiceprintRequestModel {
    #[serde(rename = "precision-2")]
    Precision2,
}

impl From<VoiceprintRequestModel> for types::VoiceprintRequestModel {
    fn from(value: VoiceprintRequestModel) -> Self {
        match value {
            VoiceprintRequestModel::Precision2 => Self::Precision2,
        }
    }
}
