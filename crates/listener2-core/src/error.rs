use crate::BatchErrorCode;

use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum BatchFailure {
    #[error("Failed to start transcription (internal task error).")]
    AudioMetadataJoinFailed,
    #[error("{message}")]
    AudioMetadataReadFailed { message: String },
    #[error("{message}")]
    ProviderRequestFailed { message: String },
    #[error("{message}")]
    ActorSpawnFailed { message: String },
    #[error("Batch stream start cancelled unexpectedly.")]
    StreamStartCancelled,
    #[error("Batch stream stopped without reporting completion.")]
    StreamStoppedWithoutCompletionSignal,
    #[error("Batch stream finished without reporting status.")]
    StreamFinishedWithoutStatus,
    #[error("{message}")]
    StreamStartFailed { message: String },
    #[error("{message}")]
    StreamError { message: String },
    #[error("Timed out waiting for batch stream response.")]
    StreamTimeout,
}

impl BatchFailure {
    pub fn code(&self) -> BatchErrorCode {
        match self {
            Self::AudioMetadataJoinFailed => BatchErrorCode::AudioMetadataJoinFailed,
            Self::AudioMetadataReadFailed { .. } => BatchErrorCode::AudioMetadataReadFailed,
            Self::ProviderRequestFailed { .. } => BatchErrorCode::ProviderRequestFailed,
            Self::ActorSpawnFailed { .. } => BatchErrorCode::ActorSpawnFailed,
            Self::StreamStartCancelled => BatchErrorCode::StreamStartCancelled,
            Self::StreamStoppedWithoutCompletionSignal => {
                BatchErrorCode::StreamStoppedWithoutCompletionSignal
            }
            Self::StreamFinishedWithoutStatus => BatchErrorCode::StreamFinishedWithoutStatus,
            Self::StreamStartFailed { .. } => BatchErrorCode::StreamStartFailed,
            Self::StreamError { .. } => BatchErrorCode::StreamError,
            Self::StreamTimeout => BatchErrorCode::StreamTimeout,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    Batch(#[from] owhisper_client::Error),
    #[error(transparent)]
    SpawnError(#[from] ractor::SpawnErr),
    #[error("batch start failed: {0}")]
    BatchStartFailed(String),
    #[error("batch error: {0}")]
    BatchError(String),
    #[error(transparent)]
    BatchFailed(#[from] BatchFailure),
    #[error("denoise error: {0}")]
    DenoiseError(String),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
