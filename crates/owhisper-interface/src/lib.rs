pub mod batch;
pub mod batch_sse;
pub mod batch_stream;
#[cfg(feature = "openapi")]
pub mod openapi;
pub mod progress;
pub mod stream;

#[cfg(feature = "openapi")]
pub use openapi::openapi;
pub use progress::{InferencePhase, InferenceProgress};

#[macro_export]
macro_rules! common_derives {
    ($item:item) => {
        #[derive(
            PartialEq,
            Debug,
            Clone,
            serde::Serialize,
            serde::Deserialize,
            specta::Type,
            schemars::JsonSchema,
        )]
        #[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
        #[schemars(deny_unknown_fields)]
        $item
    };
}

// TODO: this is legacy format, but it works, and we already stored them in user db
common_derives! {
    #[derive(Default)]
    pub struct Word2 {
        pub text: String,
        pub speaker: Option<SpeakerIdentity>,
        pub confidence: Option<f32>,
        pub start_ms: Option<u64>,
        pub end_ms: Option<u64>,
    }
}

impl From<stream::Word> for Word2 {
    fn from(word: stream::Word) -> Self {
        Word2 {
            text: word.punctuated_word.unwrap_or(word.word),
            speaker: word
                .speaker
                .map(|s| SpeakerIdentity::Unassigned { index: s as u8 }),
            confidence: Some(word.confidence as f32),
            start_ms: Some((word.start * 1000.0) as u64),
            end_ms: Some((word.end * 1000.0) as u64),
        }
    }
}

impl From<batch::Word> for Word2 {
    fn from(word: batch::Word) -> Self {
        Word2 {
            text: word.punctuated_word.unwrap_or(word.word),
            speaker: word
                .speaker
                .map(|s| SpeakerIdentity::Unassigned { index: s as u8 }),
            confidence: Some(word.confidence as f32),
            start_ms: Some((word.start * 1000.0) as u64),
            end_ms: Some((word.end * 1000.0) as u64),
        }
    }
}

common_derives! {
    #[serde(tag = "type", content = "value")]
    pub enum SpeakerIdentity {
        #[serde(rename = "unassigned")]
        Unassigned { index: u8 },
        #[serde(rename = "assigned")]
        Assigned { id: String, label: String },
    }
}

common_derives! {
    #[derive(Default)]
    pub struct ListenOutputChunk {
        #[cfg_attr(feature = "openapi", schema(value_type = Option<Object>))]
        pub meta: Option<serde_json::Value>,
        pub words: Vec<Word2>,
    }
}

common_derives! {
    #[serde(tag = "type", content = "value")]
    pub enum ListenInputChunk {
        #[serde(rename = "audio")]
        Audio {
            #[serde(serialize_with = "serde_bytes::serialize")]
            data: Vec<u8>,
        },
        #[serde(rename = "dual_audio")]
        DualAudio {
            #[serde(serialize_with = "serde_bytes::serialize")]
            mic: Vec<u8>,
            #[serde(serialize_with = "serde_bytes::serialize")]
            speaker: Vec<u8>,
        },
        #[serde(rename = "end")]
        End,
    }
}

#[derive(
    PartialEq,
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    specta::Type,
    schemars::JsonSchema,
)]
#[schemars(deny_unknown_fields)]
pub enum MixedMessage<A, C> {
    Audio(A),
    Control(C),
}

// https://github.com/deepgram/deepgram-rust-sdk/blob/d2f2723/src/listen/websocket.rs#L772-L778
common_derives! {
    #[serde(tag = "type")]
    pub enum ControlMessage {
        Finalize,
        KeepAlive,
        CloseStream,
    }
}

common_derives! {
    pub struct ListenParams {
        #[serde(default)]
        pub model: Option<String>,
        #[serde(default = "ListenParams::default_channels")]
        pub channels: u8,
        #[serde(default = "ListenParams::default_sample_rate")]
        pub sample_rate: u32,
        // https://docs.rs/axum-extra/0.10.1/axum_extra/extract/struct.Query.html#example-1
        #[serde(default, alias = "language")]
        #[cfg_attr(feature = "openapi", schema(value_type = Vec<String>))]
        pub languages: Vec<hypr_language::Language>,
        #[serde(default)]
        pub keywords: Vec<String>,
        #[serde(default)]
        #[cfg_attr(feature = "openapi", schema(value_type = Option<Object>))]
        pub custom_query: Option<std::collections::HashMap<String, String>>,
    }
}

impl Default for ListenParams {
    fn default() -> Self {
        Self {
            model: None,
            channels: Self::default_channels(),
            sample_rate: Self::default_sample_rate(),
            languages: Vec::new(),
            keywords: Vec::new(),
            custom_query: None,
        }
    }
}

impl ListenParams {
    fn default_channels() -> u8 {
        1
    }

    fn default_sample_rate() -> u32 {
        16000
    }
}
