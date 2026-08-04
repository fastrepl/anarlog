const UNSUPPORTED_WEBSOCKET_TEXT_PAYLOAD: &str = "unsupported websocket text payload";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Audio(#[from] anlg_audio_utils::Error),

    #[error(transparent)]
    Wav(#[from] hound::Error),

    #[error(transparent)]
    Whisper(#[from] anlg_whisper_local::Error),

    #[error(transparent)]
    Chunking(#[from] anlg_audio_chunking::Error),

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
