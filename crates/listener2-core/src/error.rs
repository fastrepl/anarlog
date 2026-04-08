use crate::BatchErrorCode;

use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum BatchFailure {
    #[error("Failed to start transcription (internal task error).")]
    AudioMetadataJoinFailed,
    #[error("{message}")]
    AudioMetadataReadFailed { message: String },
    #[error("{provider} does not support batch transcription")]
    BatchCapabilityUnsupported { provider: String },
    #[error("{provider} requires progressive batch mode")]
    DirectBatchUnsupported { provider: String },
    #[error("{provider} does not support progressive batch mode")]
    ProgressiveBatchUnsupported { provider: String },
    #[error("{message}")]
    DirectRequestFailed { provider: String, message: String },
    #[error("{message}")]
    ProgressiveActorSpawnFailed { provider: String, message: String },
    #[error("Progressive batch stream start cancelled unexpectedly.")]
    ProgressiveStartCancelled,
    #[error("Progressive batch stream stopped without reporting completion.")]
    ProgressiveStoppedWithoutCompletionSignal,
    #[error("Progressive batch stream finished without reporting status.")]
    ProgressiveFinishedWithoutStatus,
    #[error("{message}")]
    ProgressiveStartFailed { provider: String, message: String },
    #[error("{message}")]
    ProgressiveStreamError { provider: String, message: String },
    #[error("Timed out waiting for progressive batch stream response.")]
    ProgressiveStreamTimeout,
}

impl BatchFailure {
    pub fn code(&self) -> BatchErrorCode {
        match self {
            Self::AudioMetadataJoinFailed => BatchErrorCode::AudioMetadataJoinFailed,
            Self::AudioMetadataReadFailed { .. } => BatchErrorCode::AudioMetadataReadFailed,
            Self::BatchCapabilityUnsupported { .. } => BatchErrorCode::BatchCapabilityUnsupported,
            Self::DirectBatchUnsupported { .. } => BatchErrorCode::DirectBatchUnsupported,
            Self::ProgressiveBatchUnsupported { .. } => BatchErrorCode::ProgressiveBatchUnsupported,
            Self::DirectRequestFailed { .. } => BatchErrorCode::DirectRequestFailed,
            Self::ProgressiveActorSpawnFailed { .. } => BatchErrorCode::ProgressiveActorSpawnFailed,
            Self::ProgressiveStartCancelled => BatchErrorCode::ProgressiveStartCancelled,
            Self::ProgressiveStoppedWithoutCompletionSignal => {
                BatchErrorCode::ProgressiveStoppedWithoutCompletionSignal
            }
            Self::ProgressiveFinishedWithoutStatus => {
                BatchErrorCode::ProgressiveFinishedWithoutStatus
            }
            Self::ProgressiveStartFailed { .. } => BatchErrorCode::ProgressiveStartFailed,
            Self::ProgressiveStreamError { .. } => BatchErrorCode::ProgressiveStreamError,
            Self::ProgressiveStreamTimeout => BatchErrorCode::ProgressiveStreamTimeout,
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
