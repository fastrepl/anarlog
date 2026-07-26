use std::collections::HashMap;

use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

use super::content::AudioFormat;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponseRole {
    User,
    System,
    Assistant,
    Developer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ResponseInputContent {
    #[serde(rename = "input_text")]
    InputText { text: String },
    #[serde(rename = "input_image")]
    InputImage {
        image_url: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<ResponsesImageDetail>,
    },
    #[serde(rename = "input_file")]
    InputFile {
        #[serde(skip_serializing_if = "Option::is_none")]
        file_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        file_data: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        filename: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        file_url: Option<String>,
    },
    #[serde(rename = "input_audio")]
    InputAudio { input_audio: InputAudio },
    #[serde(rename = "input_video")]
    InputVideo { video_url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponsesImageDetail {
    Auto,
    High,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputAudio {
    pub data: String,
    pub format: AudioFormat,
}

impl ResponseInputContent {
    pub fn input_audio(data: impl Into<String>, format: AudioFormat) -> Self {
        Self::InputAudio {
            input_audio: InputAudio {
                data: data.into(),
                format,
            },
        }
    }

    pub fn input_audio_from_bytes(data: &[u8], format: AudioFormat) -> Self {
        Self::InputAudio {
            input_audio: InputAudio {
                data: STANDARD.encode(data),
                format,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseInputMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    pub role: ResponseRole,
    pub content: ResponseMessageContent,
}

impl ResponseInputMessage {
    pub fn user(content: impl Into<ResponseMessageContent>) -> Self {
        Self {
            id: None,
            r#type: Some("message".to_string()),
            role: ResponseRole::User,
            content: content.into(),
        }
    }

    pub fn system(content: impl Into<ResponseMessageContent>) -> Self {
        Self {
            id: None,
            r#type: Some("message".to_string()),
            role: ResponseRole::System,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<ResponseMessageContent>) -> Self {
        Self {
            id: None,
            r#type: Some("message".to_string()),
            role: ResponseRole::Assistant,
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseMessageContent {
    Text(String),
    Parts(Vec<ResponseInputContent>),
}

impl From<String> for ResponseMessageContent {
    fn from(s: String) -> Self {
        ResponseMessageContent::Text(s)
    }
}

impl From<&str> for ResponseMessageContent {
    fn from(s: &str) -> Self {
        ResponseMessageContent::Text(s.to_string())
    }
}

impl From<Vec<ResponseInputContent>> for ResponseMessageContent {
    fn from(parts: Vec<ResponseInputContent>) -> Self {
        ResponseMessageContent::Parts(parts)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFunctionCall {
    #[serde(rename = "type")]
    pub r#type: String,
    pub call_id: String,
    pub name: String,
    pub arguments: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ToolCallStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFunctionCallOutput {
    #[serde(rename = "type")]
    pub r#type: String,
    pub call_id: String,
    pub output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ToolCallStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    InProgress,
    Completed,
    Incomplete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseInputItem {
    Message(ResponseInputMessage),
    FunctionCall(ResponseFunctionCall),
    FunctionCallOutput(ResponseFunctionCallOutput),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseInput {
    Text(String),
    Items(Vec<ResponseInputItem>),
}

impl From<String> for ResponseInput {
    fn from(s: String) -> Self {
        ResponseInput::Text(s)
    }
}

impl From<&str> for ResponseInput {
    fn from(s: &str) -> Self {
        ResponseInput::Text(s.to_string())
    }
}

impl From<Vec<ResponseInputItem>> for ResponseInput {
    fn from(items: Vec<ResponseInputItem>) -> Self {
        ResponseInput::Items(items)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFunctionTool {
    #[serde(rename = "type")]
    pub r#type: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
}

impl ResponseFunctionTool {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            r#type: "function".to_string(),
            name: name.into(),
            description: Some(description.into()),
            parameters: Some(parameters),
            strict: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseTool {
    Function(ResponseFunctionTool),
    WebSearchPreview { r#type: String },
    WebSearch { r#type: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseToolChoice {
    String(String),
    Specific { r#type: String, name: String },
}

impl ResponseToolChoice {
    pub fn auto() -> Self {
        Self::String("auto".to_string())
    }

    pub fn none() -> Self {
        Self::String("none".to_string())
    }

    pub fn required() -> Self {
        Self::String("required".to_string())
    }

    pub fn function(name: impl Into<String>) -> Self {
        Self::Specific {
            r#type: "function".to_string(),
            name: name.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ResponseTextFormat {
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "json_object")]
    JsonObject,
    #[serde(rename = "json_schema")]
    JsonSchema {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        schema: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        strict: Option<bool>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseTextConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<ResponseTextFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verbosity: Option<ResponseTextVerbosity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponseTextVerbosity {
    High,
    Low,
    Medium,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponsesReasoningEffort {
    Xhigh,
    High,
    Medium,
    Low,
    Minimal,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponsesReasoningSummaryVerbosity {
    Auto,
    Concise,
    Detailed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesReasoningConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<ResponsesReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<ResponsesReasoningSummaryVerbosity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResponsesRequest {
    pub input: ResponseInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ResponseTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ResponseToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<ResponseTextConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ResponsesReasoningConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tool_calls: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modalities: Option<Vec<ResponsesModality>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ResponsesProviderPreferences>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponsesModality {
    Text,
    Image,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResponsesProviderPreferences {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_fallbacks: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_parameters: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_collection: Option<ResponsesDataCollection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantizations: Option<Vec<ResponsesQuantization>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<ResponsesProviderSort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_price: Option<ResponsesMaxPrice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_min_throughput: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_max_latency: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zdr: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enforce_distillable_text: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponsesDataCollection {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponsesQuantization {
    Int4,
    Int8,
    Fp4,
    Fp6,
    Fp8,
    Fp16,
    Bf16,
    Fp32,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponsesProviderSort {
    Price,
    Throughput,
    Latency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesMaxPrice {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesResponse {
    pub id: String,
    pub object: String,
    pub created_at: f64,
    pub model: String,
    pub status: ResponsesStatus,
    #[serde(default)]
    pub completed_at: Option<f64>,
    pub output: Vec<ResponsesOutputItem>,
    #[serde(default)]
    pub output_text: Option<String>,
    #[serde(default)]
    pub error: Option<ResponsesError>,
    #[serde(default)]
    pub usage: Option<ResponsesUsage>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponsesStatus {
    Completed,
    Incomplete,
    InProgress,
    Failed,
    Cancelled,
    Queued,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponsesOutputItem {
    #[serde(rename = "message")]
    Message(ResponsesOutputMessage),
    #[serde(rename = "reasoning")]
    Reasoning(ResponsesOutputReasoning),
    #[serde(rename = "function_call")]
    FunctionCall(ResponsesOutputFunctionCall),
    #[serde(rename = "web_search_call")]
    WebSearchCall(ResponsesOutputWebSearchCall),
    #[serde(rename = "file_search_call")]
    FileSearchCall(ResponsesOutputFileSearchCall),
    #[serde(rename = "image_generation_call")]
    ImageGenerationCall(ResponsesOutputImageGenerationCall),
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesOutputMessage {
    pub id: String,
    pub role: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub status: ResponsesOutputMessageStatus,
    pub content: Vec<ResponsesOutputContent>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponsesOutputMessageStatus {
    Completed,
    Incomplete,
    InProgress,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponsesOutputContent {
    #[serde(rename = "output_text")]
    OutputText {
        text: String,
        #[serde(default)]
        annotations: Vec<ResponsesAnnotation>,
    },
    #[serde(rename = "refusal")]
    Refusal { refusal: String },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponsesAnnotation {
    FileCitation {
        file_id: String,
        filename: String,
        index: f64,
    },
    UrlCitation {
        url: String,
        title: String,
        start_index: f64,
        end_index: f64,
    },
    FilePath {
        file_id: String,
        index: f64,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesOutputReasoning {
    pub id: String,
    #[serde(default)]
    pub content: Vec<ResponsesReasoningTextContent>,
    pub summary: Vec<ResponsesReasoningSummaryText>,
    #[serde(default)]
    pub encrypted_content: Option<String>,
    #[serde(default)]
    pub status: Option<ResponsesReasoningStatus>,
    #[serde(default)]
    pub signature: Option<String>,
    #[serde(default)]
    pub format: Option<ResponsesReasoningFormat>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesReasoningTextContent {
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesReasoningSummaryText {
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponsesReasoningStatus {
    Completed,
    Incomplete,
    InProgress,
}

#[derive(Debug, Clone, Deserialize)]
pub enum ResponsesReasoningFormat {
    #[serde(rename = "unknown")]
    Unknown,
    #[serde(rename = "openai-responses-v1")]
    OpenaiResponsesV1,
    #[serde(rename = "azure-openai-responses-v1")]
    AzureOpenaiResponsesV1,
    #[serde(rename = "xai-responses-v1")]
    XaiResponsesV1,
    #[serde(rename = "anthropic-claude-v1")]
    AnthropicClaudeV1,
    #[serde(rename = "google-gemini-v1")]
    GoogleGeminiV1,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesOutputFunctionCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
    pub call_id: String,
    #[serde(default)]
    pub status: Option<ResponsesFunctionCallStatus>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponsesFunctionCallStatus {
    Completed,
    Incomplete,
    InProgress,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesOutputWebSearchCall {
    pub id: String,
    pub status: ResponsesWebSearchStatus,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponsesWebSearchStatus {
    Completed,
    Searching,
    InProgress,
    Failed,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesOutputFileSearchCall {
    pub id: String,
    pub queries: Vec<String>,
    pub status: ResponsesWebSearchStatus,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesOutputImageGenerationCall {
    pub id: String,
    #[serde(default)]
    pub result: Option<String>,
    pub status: ResponsesImageGenerationStatus,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponsesImageGenerationStatus {
    InProgress,
    Completed,
    Generating,
    Failed,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesUsage {
    pub input_tokens: f64,
    pub input_tokens_details: ResponsesInputTokensDetails,
    pub output_tokens: f64,
    pub output_tokens_details: ResponsesOutputTokensDetails,
    pub total_tokens: f64,
    #[serde(default)]
    pub cost: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesInputTokensDetails {
    pub cached_tokens: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesOutputTokensDetails {
    pub reasoning_tokens: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesStreamEvent {
    pub event: Option<String>,
    pub data: Option<ResponsesStreamData>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesStreamData {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub output: Option<Vec<ResponsesStreamOutputItem>>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub delta: Option<ResponsesStreamDelta>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponsesStreamOutputItem {
    #[serde(rename = "response.output_text.delta")]
    OutputTextDelta {
        index: f64,
        #[serde(default)]
        delta: Option<String>,
    },
    #[serde(rename = "response.output_text.done")]
    OutputTextDone { index: f64, text: String },
    #[serde(rename = "response.created")]
    ResponseCreated { response: ResponsesResponseSummary },
    #[serde(rename = "response.completed")]
    ResponseCompleted { response: ResponsesResponse },
    #[serde(rename = "response.output_item.added")]
    OutputItemAdded {
        output_index: f64,
        item: ResponsesStreamItemSummary,
    },
    #[serde(rename = "response.output_item.done")]
    OutputItemDone {
        output_index: f64,
        item: ResponsesStreamItemSummary,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesResponseSummary {
    pub id: String,
    pub status: ResponsesStatus,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesStreamItemSummary {
    pub id: String,
    #[serde(rename = "type")]
    pub r#type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesStreamDelta {
    #[serde(default)]
    pub content: Option<String>,
}
