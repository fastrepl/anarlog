use crate::{InferenceProgress, batch, batch_stream, common_derives, stream};

pub const EVENT_NAME: &str = "batch";

common_derives! {
    #[serde(tag = "type", rename_all = "snake_case")]
    pub enum BatchSseMessage {
        Progress { progress: InferenceProgress },
        Segment { response: stream::StreamResponse },
        Result { response: batch::Response },
        Error { error: String, detail: String },
    }
}

impl From<BatchSseMessage> for batch_stream::BatchStreamEvent {
    fn from(value: BatchSseMessage) -> Self {
        match value {
            BatchSseMessage::Progress { progress } => batch_stream::BatchStreamEvent::Progress {
                percentage: progress.percentage,
                partial_text: progress.partial_text,
            },
            BatchSseMessage::Segment { response } => batch_stream::BatchStreamEvent::Segment {
                percentage: 0.0,
                response,
            },
            BatchSseMessage::Result { response } => {
                batch_stream::BatchStreamEvent::Result { response }
            }
            BatchSseMessage::Error { error, detail } => batch_stream::BatchStreamEvent::Error {
                error_code: None,
                error_message: detail,
                provider: error,
            },
        }
    }
}

impl BatchSseMessage {
    pub fn from_batch_stream_event(
        event: batch_stream::BatchStreamEvent,
        fallback_provider: &str,
    ) -> Option<Self> {
        match event {
            batch_stream::BatchStreamEvent::Progress {
                percentage,
                partial_text,
            } => Some(Self::Progress {
                progress: InferenceProgress {
                    percentage,
                    partial_text,
                    phase: crate::progress::InferencePhase::Transcribing,
                },
            }),
            batch_stream::BatchStreamEvent::Segment { .. }
            | batch_stream::BatchStreamEvent::Terminal { .. } => None,
            batch_stream::BatchStreamEvent::Result { response } => Some(Self::Result { response }),
            batch_stream::BatchStreamEvent::Error {
                error_message,
                provider,
                ..
            } => Some(Self::Error {
                error: if provider.is_empty() {
                    fallback_provider.to_string()
                } else {
                    provider
                },
                detail: error_message,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::batch;

    #[test]
    fn from_batch_stream_event_maps_progress() {
        let message = BatchSseMessage::from_batch_stream_event(
            batch_stream::BatchStreamEvent::Progress {
                percentage: 0.5,
                partial_text: Some("hello".to_string()),
            },
            "openai",
        )
        .expect("expected progress message");

        let BatchSseMessage::Progress { progress } = message else {
            panic!("expected progress");
        };

        assert_eq!(progress.percentage, 0.5);
        assert_eq!(progress.partial_text.as_deref(), Some("hello"));
        assert_eq!(
            progress.phase,
            crate::progress::InferencePhase::Transcribing
        );
    }

    #[test]
    fn from_batch_stream_event_maps_result() {
        let response = batch::Response {
            metadata: serde_json::json!({}),
            results: batch::Results { channels: vec![] },
        };

        let message = BatchSseMessage::from_batch_stream_event(
            batch_stream::BatchStreamEvent::Result {
                response: response.clone(),
            },
            "openai",
        )
        .expect("expected result message");

        let BatchSseMessage::Result {
            response: mapped_response,
        } = message
        else {
            panic!("expected result");
        };

        assert_eq!(mapped_response, response);
    }

    #[test]
    fn from_batch_stream_event_uses_fallback_provider_for_errors() {
        let message = BatchSseMessage::from_batch_stream_event(
            batch_stream::BatchStreamEvent::Error {
                error_code: None,
                error_message: "boom".to_string(),
                provider: String::new(),
            },
            "openai",
        )
        .expect("expected error message");

        let BatchSseMessage::Error { error, detail } = message else {
            panic!("expected error");
        };

        assert_eq!(error, "openai");
        assert_eq!(detail, "boom");
    }

    #[test]
    fn from_batch_stream_event_ignores_non_batch_sse_events() {
        assert!(
            BatchSseMessage::from_batch_stream_event(
                batch_stream::BatchStreamEvent::Terminal {
                    request_id: "req_123".to_string(),
                    created: "now".to_string(),
                    duration: 1.0,
                    channels: 1,
                },
                "openai",
            )
            .is_none()
        );
    }
}
