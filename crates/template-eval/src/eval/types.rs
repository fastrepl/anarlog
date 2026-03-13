use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvalCase {
    pub name: String,
    pub messages: Vec<EvalMessage>,
    pub prompt_fragments: Vec<PromptFragment>,
    pub smoke_outputs: Vec<String>,
    pub expectations: Vec<Expectation>,
    pub required_pass_rate: f64,
    pub samples: usize,
    pub max_tokens: u32,
    pub response_format: Option<CaseResponseFormat>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvalMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromptFragment {
    pub role: String,
    pub needle: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Expectation {
    ExactTrimmed(String),
    SingleLine,
    NotContains(String),
    JsonPatchEmpty,
    JsonPatchSingleReplace { path: String, value: String },
    MarkdownAtLeastHeadings(usize),
    MarkdownAllHeadingsAreH1,
    MarkdownHasHeadings(Vec<String>),
    MarkdownHasUnorderedList,
    MarkdownWordCountAtMost(usize),
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum CaseResponseFormat {
    JsonObject,
}

#[derive(Clone, Debug)]
pub struct LiveConfig {
    pub api_key: String,
    pub model: String,
    pub artifacts_dir: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
pub struct RunArtifact {
    pub case_name: String,
    pub model: String,
    pub required_pass_rate: f64,
    pub pass_rate: f64,
    pub pass_count: usize,
    pub sample_count: usize,
    pub passed: bool,
    pub created_at_ms: u128,
    pub messages: Vec<EvalMessage>,
    pub samples: Vec<SampleArtifact>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SampleArtifact {
    pub index: usize,
    pub output: String,
    pub passed: bool,
    pub failure: Option<String>,
}
