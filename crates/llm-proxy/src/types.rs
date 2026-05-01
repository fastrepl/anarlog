use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ToolChoice {
    String(String),
    Object {
        #[serde(rename = "type")]
        type_: String,
        function: serde_json::Value,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub messages: Vec<ChatMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

pub fn has_audio_content(messages: &[ChatMessage]) -> bool {
    messages.iter().any(|msg| {
        msg.content
            .as_ref()
            .and_then(|c| c.as_array())
            .is_some_and(|parts| {
                parts
                    .iter()
                    .any(|part| part.get("type").and_then(|t| t.as_str()) == Some("input_audio"))
            })
    })
}

#[derive(Debug, Deserialize)]
pub struct UsageInfo {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
}

impl UsageInfo {
    pub fn input_tokens(&self) -> u32 {
        self.prompt_tokens.unwrap_or(0)
    }

    pub fn output_tokens(&self) -> u32 {
        self.completion_tokens.unwrap_or(0)
    }
}
