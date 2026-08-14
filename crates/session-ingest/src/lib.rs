#![forbid(unsafe_code)]

mod model;
mod protocol;

#[cfg(feature = "apply")]
mod apply;
#[cfg(feature = "apply")]
mod validate;

#[cfg(feature = "apply")]
pub use apply::{ApplyOutcome, Error, apply_session_envelope};
pub use model::{
    DocumentFormat, DocumentKind, IngestAttachment, IngestDocument, IngestParticipant,
    IngestSession, IngestSpeakerHint, IngestTranscript, IngestWord, SessionIngestEnvelope,
};
pub use protocol::{
    AcknowledgeRequest, AcknowledgeResponse, DeliveryItem, DeliveryPage, RecordingDownload,
    SessionRead,
};

pub const SESSION_INGEST_SCHEMA_VERSION: u32 = 1;
pub const MAX_SESSION_INGEST_BYTES: usize = 2 * 1024 * 1024;

#[cfg(test)]
mod tests;
