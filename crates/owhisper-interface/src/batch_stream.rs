use crate::{batch, common_derives, stream};

common_derives! {
    #[serde(tag = "type", rename_all = "snake_case")]
    pub enum BatchStreamEvent {
        Progress {
            percentage: f64,
            #[serde(default)]
            partial_text: Option<String>,
        },
        Segment {
            response: stream::StreamResponse,
            percentage: f64,
        },
        Terminal {
            request_id: String,
            created: String,
            duration: f64,
            channels: u32,
        },
        Result {
            response: batch::Response,
        },
        Error {
            error_code: Option<i32>,
            error_message: String,
            provider: String,
        },
    }
}

impl BatchStreamEvent {
    pub fn percentage(&self) -> f64 {
        match self {
            Self::Progress { percentage, .. } | Self::Segment { percentage, .. } => *percentage,
            Self::Terminal { .. } | Self::Result { .. } => 1.0,
            Self::Error { .. } => 0.0,
        }
    }

    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Progress { partial_text, .. } => partial_text.as_deref(),
            Self::Segment { response, .. } => response.text(),
            Self::Result { response } => response
                .results
                .channels
                .first()
                .and_then(|channel| channel.alternatives.first())
                .map(|alternative| alternative.transcript.as_str()),
            Self::Terminal { .. } | Self::Error { .. } => None,
        }
    }
}
