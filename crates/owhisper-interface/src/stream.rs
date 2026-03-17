use crate::common_derives;

// https://github.com/deepgram/deepgram-rust-sdk/blob/0.7.0/src/common/stream_response.rs
// https://developers.deepgram.com/reference/speech-to-text-api/listen-streaming#receive.receiveTranscription

common_derives! {
    #[specta(rename = "StreamWord")]
    #[cfg_attr(feature = "openapi", schema(as = StreamWord))]
    pub struct Word {
        pub word: String,
        pub start: f64,
        pub end: f64,
        pub confidence: f64,
        pub speaker: Option<i32>,
        pub punctuated_word: Option<String>,
        pub language: Option<String>,
    }
}

common_derives! {
    #[specta(rename = "StreamAlternatives")]
    #[cfg_attr(feature = "openapi", schema(as = StreamAlternatives))]
    pub struct Alternatives {
        pub transcript: String,
        pub words: Vec<Word>,
        pub confidence: f64,
        #[serde(default)]
        pub languages: Vec<String>,
    }
}

common_derives! {
    #[specta(rename = "StreamChannel")]
    #[cfg_attr(feature = "openapi", schema(as = StreamChannel))]
    pub struct Channel {
        pub alternatives: Vec<Alternatives>,
    }
}

common_derives! {
    #[specta(rename = "StreamModelInfo")]
    #[cfg_attr(feature = "openapi", schema(as = StreamModelInfo))]
    pub struct ModelInfo {
        pub name: String,
        pub version: String,
        pub arch: String,
    }
}

common_derives! {
    #[specta(rename = "StreamMetadata")]
    #[cfg_attr(feature = "openapi", schema(as = StreamMetadata))]
    pub struct Metadata {
        pub request_id: String,
        pub model_info: ModelInfo,
        pub model_uuid: String,
        #[serde(default)]
        #[specta(type = Extra)]
        #[cfg_attr(feature = "openapi", schema(value_type = Option<Object>))]
        pub extra: Option<std::collections::HashMap<String, serde_json::Value>>,
    }
}

common_derives! {
    #[specta(rename = "StreamExtra")]
    pub struct Extra {
        pub started_unix_millis: u64,
    }
}

impl Default for Extra {
    fn default() -> Self {
        let started_unix_millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(u64::MAX as u128) as u64;

        Self {
            started_unix_millis,
        }
    }
}

impl From<Extra> for std::collections::HashMap<String, serde_json::Value> {
    fn from(extra: Extra) -> Self {
        let mut map = std::collections::HashMap::new();
        map.insert(
            "started_unix_millis".to_string(),
            serde_json::Value::Number(extra.started_unix_millis.into()),
        );
        map
    }
}

impl Default for Metadata {
    fn default() -> Self {
        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            model_uuid: uuid::Uuid::new_v4().to_string(),
            model_info: ModelInfo {
                name: "".to_string(),
                version: "".to_string(),
                arch: "".to_string(),
            },
            extra: None,
        }
    }
}

common_derives! {
    #[serde(tag = "type")]
    #[non_exhaustive]
    pub enum StreamResponse {
        #[serde(rename = "Results")]
        TranscriptResponse {
            start: f64,
            duration: f64,
            is_final: bool,
            speech_final: bool,
            from_finalize: bool,
            channel: Channel,
            metadata: Metadata,
            channel_index: Vec<i32>,
        },
        #[serde(rename = "Metadata")]
        TerminalResponse {
            request_id: String,
            created: String,
            duration: f64,
            channels: u32,
        },
        #[serde(rename = "SpeechStarted")]
        SpeechStartedResponse {
            channel: Vec<u8>,
            timestamp: f64,
        },
        #[serde(rename = "UtteranceEnd")]
        UtteranceEndResponse {
            channel: Vec<u8>,
            last_word_end: f64,
        },
        #[serde(rename = "Error")]
        ErrorResponse {
            error_code: Option<i32>,
            error_message: String,
            provider: String,
        },
    }
}

impl StreamResponse {
    pub fn text(&self) -> Option<&str> {
        match self {
            StreamResponse::TranscriptResponse { channel, .. } => {
                channel.alternatives.first().map(|a| a.transcript.as_str())
            }
            _ => None,
        }
    }

    pub fn apply_offset(&mut self, offset_secs: f64) {
        match self {
            StreamResponse::TranscriptResponse { start, channel, .. } => {
                *start += offset_secs;
                for alt in &mut channel.alternatives {
                    for word in &mut alt.words {
                        word.start += offset_secs;
                        word.end += offset_secs;
                    }
                }
            }
            StreamResponse::SpeechStartedResponse { timestamp, .. } => {
                *timestamp += offset_secs;
            }
            StreamResponse::UtteranceEndResponse { last_word_end, .. } => {
                *last_word_end += offset_secs;
            }
            _ => {}
        }
    }

    pub fn set_extra(&mut self, extra: &Extra) {
        if let StreamResponse::TranscriptResponse { metadata, .. } = self {
            let incoming: std::collections::HashMap<String, serde_json::Value> =
                extra.clone().into();
            match &mut metadata.extra {
                Some(existing) => existing.extend(incoming),
                slot => *slot = Some(incoming),
            }
        }
    }

    pub fn remap_channel_index(&mut self, from: i32, to: i32) {
        if let StreamResponse::TranscriptResponse { channel_index, .. } = self
            && !channel_index.is_empty()
            && channel_index[0] == from
        {
            channel_index[0] = to;
        }
    }

    pub fn set_channel_index(&mut self, channel_idx: i32, total_channels: i32) {
        if let StreamResponse::TranscriptResponse { channel_index, .. } = self {
            *channel_index = vec![channel_idx, total_channels];
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use deepgram::common::stream_response as DG;

    #[test]
    fn ensure_types() {
        let dg = DG::StreamResponse::TranscriptResponse {
            type_field: "Results".to_string(),
            start: 0.0,
            duration: 0.0,
            is_final: false,
            speech_final: false,
            from_finalize: false,
            channel: DG::Channel {
                alternatives: vec![],
            },
            metadata: DG::Metadata {
                request_id: "".to_string(),
                model_info: DG::ModelInfo {
                    name: "".to_string(),
                    version: "".to_string(),
                    arch: "".to_string(),
                },
                model_uuid: "".to_string(),
            },
            channel_index: vec![],
        };

        let serialized = serde_json::to_string(&dg).unwrap();
        let _: StreamResponse = serde_json::from_str(&serialized).unwrap();
    }
}
