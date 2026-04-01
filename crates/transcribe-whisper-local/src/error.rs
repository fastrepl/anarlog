const UNSUPPORTED_WEBSOCKET_TEXT_PAYLOAD: &str = "unsupported websocket text payload";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Audio(#[from] hypr_audio_utils::Error),

    #[error(transparent)]
    Whisper(#[from] hypr_whisper_local::Error),

    #[error(transparent)]
    Vad(#[from] hypr_vad_chunking::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("{message}")]
    Protocol { message: String },
}

impl Error {
    pub(crate) fn protocol(message: impl Into<String>) -> Self {
        Self::Protocol {
            message: message.into(),
        }
    }

    pub(crate) fn unsupported_websocket_text_payload() -> Self {
        Self::protocol(UNSUPPORTED_WEBSOCKET_TEXT_PAYLOAD)
    }
}
