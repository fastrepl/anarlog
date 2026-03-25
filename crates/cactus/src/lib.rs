mod error;
mod ffi_utils;
mod llm;
pub mod log;
mod model;
mod stt;
mod vad;

pub use error::Error;
pub use hypr_language::Language;
pub use llm::{CompleteOptions, CompletionResult, CompletionStream, Message, complete_stream};
pub use model::{Model, ModelBuilder, ModelKind};
pub use stt::{
    CloudConfig, StreamResult, StreamSegment, TranscribeEvent, TranscribeOptions, Transcriber,
    TranscriptionResult, TranscriptionSession, constrain_to, transcribe_stream,
};
pub use vad::{VadOptions, VadResult, VadSegment};

pub use hypr_llm_types::{Response, StreamingParser};

#[cfg(feature = "model-manager")]
impl hypr_model_manager::ModelLoader for Model {
    type Error = Error;

    fn load(path: &std::path::Path) -> Result<Self, Self::Error> {
        Model::new(path)
    }
}
