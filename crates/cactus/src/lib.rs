mod error;
mod ffi_utils;
mod llm;
pub mod log;
mod model;
mod stt;
mod vad;

pub use error::Error;
pub use hypr_language::Language;
pub use llm::{
    CompleteOptions, CompletionResult, CompletionStream, Message, complete_stream,
    validate_messages,
};
pub use model::{Model, ModelBuilder, ModelKind};
pub use stt::{
    CloudConfig, StreamResult, StreamSegment, TranscribeEvent, TranscribeOptions, Transcriber,
    TranscriptionResult, TranscriptionSession, constrain_to, transcribe_stream,
};
pub use vad::{VadOptions, VadResult, VadSegment};

pub use hypr_llm_types::{Response, StreamingParser};
