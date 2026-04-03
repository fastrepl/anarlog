use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumString};

use super::model::{AudioModel, GptTranscriptionModel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultipartTextField {
    pub name: &'static str,
    pub value: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CommonTranscriptionOptions {
    pub chunking_strategy: Option<ChunkingStrategy>,
    pub known_speaker_names: Vec<String>,
    pub known_speaker_references: Vec<String>,
    pub language: Option<String>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateWhisperTranscriptionOptions {
    pub common: CommonTranscriptionOptions,
    pub prompt: Option<String>,
    pub response_format: Option<WhisperResponseFormat>,
    pub timestamp_granularities: Vec<TimestampGranularity>,
}

impl Default for CreateWhisperTranscriptionOptions {
    fn default() -> Self {
        Self {
            common: CommonTranscriptionOptions::default(),
            prompt: None,
            response_format: None,
            timestamp_granularities: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateGptTranscriptionOptions {
    pub model: GptTranscriptionModel,
    pub common: CommonTranscriptionOptions,
    pub include: Vec<TranscriptionInclude>,
    pub prompt: Option<String>,
    pub response_format: Option<GptResponseFormat>,
    pub stream: Option<bool>,
}

impl Default for CreateGptTranscriptionOptions {
    fn default() -> Self {
        Self {
            model: GptTranscriptionModel::Gpt4oTranscribe,
            common: CommonTranscriptionOptions::default(),
            include: Vec::new(),
            prompt: None,
            response_format: None,
            stream: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateDiarizedTranscriptionOptions {
    pub common: CommonTranscriptionOptions,
    pub response_format: Option<DiarizedResponseFormat>,
    pub stream: Option<bool>,
}

impl Default for CreateDiarizedTranscriptionOptions {
    fn default() -> Self {
        Self {
            common: CommonTranscriptionOptions {
                chunking_strategy: Some(ChunkingStrategy::auto()),
                ..Default::default()
            },
            response_format: None,
            stream: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CreateTranscriptionOptions {
    Whisper(CreateWhisperTranscriptionOptions),
    Gpt(CreateGptTranscriptionOptions),
    Diarize(CreateDiarizedTranscriptionOptions),
}

impl CreateTranscriptionOptions {
    pub fn for_model(model: AudioModel, use_response_format: bool, enable_streaming: bool) -> Self {
        match model {
            AudioModel::Whisper1 => Self::Whisper(CreateWhisperTranscriptionOptions {
                response_format: use_response_format.then_some(WhisperResponseFormat::VerboseJson),
                ..Default::default()
            }),
            AudioModel::Gpt4oTranscribe | AudioModel::Gpt4oMiniTranscribe => {
                let model = GptTranscriptionModel::try_from(model)
                    .expect("resolved OpenAI GPT transcription model should be typed");
                Self::Gpt(CreateGptTranscriptionOptions {
                    model,
                    response_format: use_response_format.then_some(GptResponseFormat::Json),
                    stream: enable_streaming.then_some(true),
                    ..Default::default()
                })
            }
            AudioModel::Gpt4oMiniTranscribe20251215 => Self::Gpt(CreateGptTranscriptionOptions {
                model: GptTranscriptionModel::Gpt4oMiniTranscribe20251215,
                response_format: use_response_format.then_some(GptResponseFormat::Json),
                stream: enable_streaming.then_some(true),
                ..Default::default()
            }),
            AudioModel::Gpt4oTranscribeDiarize => {
                Self::Diarize(CreateDiarizedTranscriptionOptions {
                    response_format: use_response_format
                        .then_some(DiarizedResponseFormat::DiarizedJson),
                    stream: enable_streaming.then_some(true),
                    ..Default::default()
                })
            }
        }
    }

    pub fn whisper() -> Self {
        Self::Whisper(CreateWhisperTranscriptionOptions::default())
    }

    pub fn gpt(model: GptTranscriptionModel) -> Self {
        Self::Gpt(CreateGptTranscriptionOptions {
            model,
            ..Default::default()
        })
    }

    pub fn diarize() -> Self {
        Self::Diarize(CreateDiarizedTranscriptionOptions::default())
    }

    pub fn model(&self) -> AudioModel {
        match self {
            Self::Whisper(_) => AudioModel::Whisper1,
            Self::Gpt(options) => options.model.into(),
            Self::Diarize(_) => AudioModel::Gpt4oTranscribeDiarize,
        }
    }

    pub fn common(&self) -> &CommonTranscriptionOptions {
        match self {
            Self::Whisper(options) => &options.common,
            Self::Gpt(options) => &options.common,
            Self::Diarize(options) => &options.common,
        }
    }

    pub fn common_mut(&mut self) -> &mut CommonTranscriptionOptions {
        match self {
            Self::Whisper(options) => &mut options.common,
            Self::Gpt(options) => &mut options.common,
            Self::Diarize(options) => &mut options.common,
        }
    }

    pub fn push_language(&mut self, language: impl Into<String>) {
        self.common_mut().language = Some(language.into());
    }

    pub fn multipart_text_fields(&self) -> Result<Vec<MultipartTextField>, serde_json::Error> {
        let mut fields = vec![MultipartTextField {
            name: "model",
            value: self.model().to_string(),
        }];

        append_common_fields(&mut fields, self.common())?;

        match self {
            Self::Whisper(options) => {
                if let Some(prompt) = &options.prompt {
                    fields.push(MultipartTextField {
                        name: "prompt",
                        value: prompt.clone(),
                    });
                }

                if let Some(response_format) = options.response_format {
                    fields.push(MultipartTextField {
                        name: "response_format",
                        value: AudioResponseFormat::from(response_format).to_string(),
                    });
                }

                for granularity in &options.timestamp_granularities {
                    fields.push(MultipartTextField {
                        name: "timestamp_granularities[]",
                        value: granularity.to_string(),
                    });
                }
            }
            Self::Gpt(options) => {
                for include in &options.include {
                    fields.push(MultipartTextField {
                        name: "include[]",
                        value: include.to_string(),
                    });
                }

                if let Some(prompt) = &options.prompt {
                    fields.push(MultipartTextField {
                        name: "prompt",
                        value: prompt.clone(),
                    });
                }

                if let Some(response_format) = options.response_format {
                    fields.push(MultipartTextField {
                        name: "response_format",
                        value: AudioResponseFormat::from(response_format).to_string(),
                    });
                }

                if let Some(stream) = options.stream {
                    fields.push(MultipartTextField {
                        name: "stream",
                        value: stream.to_string(),
                    });
                }
            }
            Self::Diarize(options) => {
                if let Some(response_format) = options.response_format {
                    fields.push(MultipartTextField {
                        name: "response_format",
                        value: AudioResponseFormat::from(response_format).to_string(),
                    });
                }

                if let Some(stream) = options.stream {
                    fields.push(MultipartTextField {
                        name: "stream",
                        value: stream.to_string(),
                    });
                }
            }
        }

        Ok(fields)
    }
}

impl Default for CreateTranscriptionOptions {
    fn default() -> Self {
        Self::gpt(GptTranscriptionModel::Gpt4oTranscribe)
    }
}

fn append_common_fields(
    fields: &mut Vec<MultipartTextField>,
    common: &CommonTranscriptionOptions,
) -> Result<(), serde_json::Error> {
    if let Some(chunking_strategy) = &common.chunking_strategy {
        let value = match chunking_strategy {
            ChunkingStrategy::Auto(_) => "auto".to_string(),
            strategy => serde_json::to_string(strategy)?,
        };
        fields.push(MultipartTextField {
            name: "chunking_strategy",
            value,
        });
    }

    for speaker_name in &common.known_speaker_names {
        fields.push(MultipartTextField {
            name: "known_speaker_names[]",
            value: speaker_name.clone(),
        });
    }

    for speaker_reference in &common.known_speaker_references {
        fields.push(MultipartTextField {
            name: "known_speaker_references[]",
            value: speaker_reference.clone(),
        });
    }

    if let Some(language) = &common.language {
        fields.push(MultipartTextField {
            name: "language",
            value: language.clone(),
        });
    }

    if let Some(temperature) = common.temperature {
        fields.push(MultipartTextField {
            name: "temperature",
            value: temperature.to_string(),
        });
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ChunkingStrategy {
    Auto(AutoChunkingStrategy),
    ServerVad(ServerVadConfig),
}

impl ChunkingStrategy {
    pub fn auto() -> Self {
        Self::Auto(AutoChunkingStrategy::Auto)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AutoChunkingStrategy {
    #[serde(rename = "auto")]
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerVadConfig {
    #[serde(rename = "type")]
    pub kind: ServerVadType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_padding_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silence_duration_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f32>,
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, EnumString, Display, AsRefStr,
)]
pub enum ServerVadType {
    #[serde(rename = "server_vad")]
    #[strum(serialize = "server_vad")]
    ServerVad,
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, EnumString, Display, AsRefStr,
)]
pub enum TranscriptionInclude {
    #[serde(rename = "logprobs")]
    #[strum(serialize = "logprobs")]
    Logprobs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, AsRefStr)]
pub enum WhisperResponseFormat {
    #[strum(serialize = "json")]
    Json,
    #[strum(serialize = "text")]
    Text,
    #[strum(serialize = "srt")]
    Srt,
    #[strum(serialize = "verbose_json")]
    VerboseJson,
    #[strum(serialize = "vtt")]
    Vtt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, AsRefStr)]
pub enum GptResponseFormat {
    #[strum(serialize = "json")]
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, AsRefStr)]
pub enum DiarizedResponseFormat {
    #[strum(serialize = "json")]
    Json,
    #[strum(serialize = "text")]
    Text,
    #[strum(serialize = "diarized_json")]
    DiarizedJson,
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, EnumString, Display, AsRefStr,
)]
pub enum AudioResponseFormat {
    #[serde(rename = "json")]
    #[strum(serialize = "json")]
    Json,
    #[serde(rename = "text")]
    #[strum(serialize = "text")]
    Text,
    #[serde(rename = "srt")]
    #[strum(serialize = "srt")]
    Srt,
    #[serde(rename = "verbose_json")]
    #[strum(serialize = "verbose_json")]
    VerboseJson,
    #[serde(rename = "vtt")]
    #[strum(serialize = "vtt")]
    Vtt,
    #[serde(rename = "diarized_json")]
    #[strum(serialize = "diarized_json")]
    DiarizedJson,
}

impl From<WhisperResponseFormat> for AudioResponseFormat {
    fn from(value: WhisperResponseFormat) -> Self {
        match value {
            WhisperResponseFormat::Json => Self::Json,
            WhisperResponseFormat::Text => Self::Text,
            WhisperResponseFormat::Srt => Self::Srt,
            WhisperResponseFormat::VerboseJson => Self::VerboseJson,
            WhisperResponseFormat::Vtt => Self::Vtt,
        }
    }
}

impl From<GptResponseFormat> for AudioResponseFormat {
    fn from(_: GptResponseFormat) -> Self {
        Self::Json
    }
}

impl From<DiarizedResponseFormat> for AudioResponseFormat {
    fn from(value: DiarizedResponseFormat) -> Self {
        match value {
            DiarizedResponseFormat::Json => Self::Json,
            DiarizedResponseFormat::Text => Self::Text,
            DiarizedResponseFormat::DiarizedJson => Self::DiarizedJson,
        }
    }
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, EnumString, Display, AsRefStr,
)]
pub enum TimestampGranularity {
    #[serde(rename = "word")]
    #[strum(serialize = "word")]
    Word,
    #[serde(rename = "segment")]
    #[strum(serialize = "segment")]
    Segment,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunking_strategy_auto_serializes_as_string() {
        let json = serde_json::to_string(&ChunkingStrategy::auto()).expect("serialize auto");

        assert_eq!(json, "\"auto\"");
    }

    #[test]
    fn request_variants_preserve_model_identity() {
        assert_eq!(
            CreateTranscriptionOptions::whisper().model(),
            AudioModel::Whisper1
        );
        assert_eq!(
            CreateTranscriptionOptions::gpt(GptTranscriptionModel::Gpt4oMiniTranscribe).model(),
            AudioModel::Gpt4oMiniTranscribe
        );
        assert_eq!(
            CreateTranscriptionOptions::diarize().model(),
            AudioModel::Gpt4oTranscribeDiarize
        );
    }

    #[test]
    fn for_model_applies_openai_defaults() {
        let whisper = CreateTranscriptionOptions::for_model(AudioModel::Whisper1, true, true);
        let diarize =
            CreateTranscriptionOptions::for_model(AudioModel::Gpt4oTranscribeDiarize, true, false);

        match whisper {
            CreateTranscriptionOptions::Whisper(options) => {
                assert_eq!(
                    options.response_format,
                    Some(WhisperResponseFormat::VerboseJson)
                );
            }
            other => panic!("expected whisper options, got {other:?}"),
        }

        match diarize {
            CreateTranscriptionOptions::Diarize(options) => {
                assert_eq!(
                    options.common.chunking_strategy,
                    Some(ChunkingStrategy::auto())
                );
                assert_eq!(
                    options.response_format,
                    Some(DiarizedResponseFormat::DiarizedJson)
                );
                assert_eq!(options.stream, None);
            }
            other => panic!("expected diarized options, got {other:?}"),
        }
    }

    #[test]
    fn multipart_text_fields_match_api_shape() {
        let mut options =
            CreateTranscriptionOptions::for_model(AudioModel::Gpt4oTranscribe, true, true);
        options.push_language("en");

        let CreateTranscriptionOptions::Gpt(gpt) = &mut options else {
            panic!("expected gpt options");
        };
        gpt.include.push(TranscriptionInclude::Logprobs);
        gpt.prompt = Some("expect domain terms".to_string());

        let fields = options
            .multipart_text_fields()
            .expect("serialize multipart");

        assert!(
            fields
                .iter()
                .any(|field| field.name == "model" && field.value == "gpt-4o-transcribe")
        );
        assert!(
            fields
                .iter()
                .any(|field| field.name == "language" && field.value == "en")
        );
        assert!(
            fields
                .iter()
                .any(|field| field.name == "include[]" && field.value == "logprobs")
        );
        assert!(
            fields
                .iter()
                .any(|field| field.name == "prompt" && field.value == "expect domain terms")
        );
        assert!(
            fields
                .iter()
                .any(|field| field.name == "response_format" && field.value == "json")
        );
        assert!(
            fields
                .iter()
                .any(|field| field.name == "stream" && field.value == "true")
        );
    }

    #[test]
    fn diarized_requests_emit_chunking_strategy_auto() {
        let options =
            CreateTranscriptionOptions::for_model(AudioModel::Gpt4oTranscribeDiarize, true, false);

        let fields = options
            .multipart_text_fields()
            .expect("serialize multipart");

        assert!(
            fields
                .iter()
                .any(|field| { field.name == "chunking_strategy" && field.value == "auto" })
        );
    }
}
