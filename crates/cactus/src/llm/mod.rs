mod complete;
mod result;
mod stream;

pub use hypr_llm_types::Message;
pub use result::CompletionResult;
pub use stream::{CompletionStream, complete_stream};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CompleteOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence_threshold: Option<f32>,
}
