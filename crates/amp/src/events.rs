use std::path::PathBuf;
use std::pin::Pin;

use futures_util::Stream;
use serde::{Deserialize, Serialize};

use crate::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<u64>,
    pub output_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TextContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolUseContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub id: String,
    pub name: String,
    pub input: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolResultContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub tool_use_id: String,
    pub content: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Map<String, serde_json::Value>,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
    #[serde(rename = "image")]
    Image {
        source_path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<serde_json::Value>,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessageEnvelope {
    pub role: String,
    pub content: Vec<ContentBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub message_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpServer {
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemInitMessage {
    pub subtype: String,
    pub session_id: String,
    pub cwd: String,
    pub tools: Vec<String>,
    pub mcp_servers: Vec<McpServer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssistantMessage {
    pub session_id: String,
    pub message: MessageEnvelope,
    pub parent_tool_use_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserMessage {
    pub session_id: String,
    pub message: MessageEnvelope,
    pub parent_tool_use_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResultBase {
    pub session_id: String,
    pub duration_ms: u64,
    pub num_turns: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_denials: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResultMessage {
    #[serde(flatten)]
    pub base: ResultBase,
    pub result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorResult {
    #[serde(flatten)]
    pub base: ResultBase,
    pub error: String,
    pub subtype: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ThreadEvent {
    SystemInit(SystemInitMessage),
    Assistant(AssistantMessage),
    User(UserMessage),
    Result(ResultMessage),
    ErrorResult(ErrorResult),
    Unknown(serde_json::Value),
}

impl ThreadEvent {
    pub fn session_id(&self) -> Option<&str> {
        match self {
            Self::SystemInit(message) => Some(&message.session_id),
            Self::Assistant(message) => Some(&message.session_id),
            Self::User(message) => Some(&message.session_id),
            Self::Result(message) => Some(&message.base.session_id),
            Self::ErrorResult(message) => Some(&message.base.session_id),
            Self::Unknown(_) => None,
        }
    }
}

impl TryFrom<serde_json::Value> for ThreadEvent {
    type Error = Error;

    fn try_from(value: serde_json::Value) -> Result<Self, Self::Error> {
        let Some(event_type) = value.get("type").and_then(serde_json::Value::as_str) else {
            return Ok(Self::Unknown(value));
        };

        match event_type {
            "system" => {
                if value.get("subtype").and_then(serde_json::Value::as_str) == Some("init") {
                    Ok(Self::SystemInit(serde_json::from_value(value)?))
                } else {
                    Ok(Self::Unknown(value))
                }
            }
            "assistant" => Ok(Self::Assistant(serde_json::from_value(value)?)),
            "user" => Ok(Self::User(serde_json::from_value(value)?)),
            "result" => {
                if value.get("is_error").and_then(serde_json::Value::as_bool) == Some(true) {
                    Ok(Self::ErrorResult(serde_json::from_value(value)?))
                } else {
                    Ok(Self::Result(serde_json::from_value(value)?))
                }
            }
            _ => Ok(Self::Unknown(value)),
        }
    }
}

#[derive(Debug, Clone)]
pub enum UserInput {
    Text(String),
    LocalImage(PathBuf),
    Message(UserInputMessage),
}

impl From<&str> for UserInput {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<String> for UserInput {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserInputMessage {
    #[serde(rename = "type")]
    pub message_type: String,
    pub message: MessageEnvelope,
}

impl UserInputMessage {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            message_type: "user".to_string(),
            message: MessageEnvelope {
                role: "user".to_string(),
                content: vec![ContentBlock::Text { text: text.into() }],
                id: None,
                message_type: None,
                model: None,
                stop_reason: None,
                stop_sequence: None,
                usage: None,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub enum Input {
    Text(String),
    Items(Vec<UserInput>),
}

impl Input {
    pub(crate) fn normalize(self) -> Result<NormalizedInput, Error> {
        match self {
            Self::Text(text) => Ok(NormalizedInput::Text(text)),
            Self::Items(items) => {
                let mut lines = Vec::new();
                let mut content = Vec::new();

                for item in items {
                    match item {
                        UserInput::Text(text) => content.push(ContentBlock::Text { text }),
                        UserInput::LocalImage(path) => content.push(ContentBlock::Image {
                            source_path: format!("file://{}", path.display()),
                            source: None,
                        }),
                        UserInput::Message(message) => {
                            lines.push(serde_json::to_string(&message)?);
                        }
                    }
                }

                if !content.is_empty() {
                    lines.push(serde_json::to_string(&UserInputMessage {
                        message_type: "user".to_string(),
                        message: MessageEnvelope {
                            role: "user".to_string(),
                            content,
                            id: None,
                            message_type: None,
                            model: None,
                            stop_reason: None,
                            stop_sequence: None,
                            usage: None,
                        },
                    })?);
                }

                Ok(NormalizedInput::StreamJson(lines.join("\n")))
            }
        }
    }
}

impl From<String> for Input {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for Input {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<Vec<UserInput>> for Input {
    fn from(value: Vec<UserInput>) -> Self {
        Self::Items(value)
    }
}

pub(crate) enum NormalizedInput {
    Text(String),
    StreamJson(String),
}

#[derive(Debug, Clone)]
pub struct Turn {
    pub events: Vec<ThreadEvent>,
    pub final_response: String,
    pub usage: Option<Usage>,
}

pub type EventStream = Pin<Box<dyn Stream<Item = Result<ThreadEvent, Error>> + Send>>;

pub struct RunStreamedResult {
    pub events: EventStream,
}

pub type StreamMessage = ThreadEvent;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;

    use super::{ContentBlock, Input, ThreadEvent, UserInput, UserInputMessage};

    #[test]
    fn serializes_text_only_input_message() {
        let message = UserInputMessage::text("Analyze this code");
        let value = serde_json::to_value(message).expect("serialize");
        assert_eq!(value["type"], "user");
        assert_eq!(value["message"]["content"][0]["type"], "text");
        assert_eq!(value["message"]["content"][0]["text"], "Analyze this code");
    }

    #[test]
    fn normalizes_text_and_image_input_to_stream_json() {
        let input = Input::from(vec![
            UserInput::Text("what do you see?".to_string()),
            UserInput::LocalImage(PathBuf::from("/tmp/example.jpg")),
        ]);

        let normalized = input.normalize().expect("normalize");
        let crate::events::NormalizedInput::StreamJson(line) = normalized else {
            panic!("expected stream json");
        };

        let value: serde_json::Value = serde_json::from_str(&line).expect("json");
        assert_eq!(value["message"]["content"][0]["type"], "text");
        assert_eq!(value["message"]["content"][1]["type"], "image");
        assert_eq!(
            value["message"]["content"][1]["source_path"],
            "file:///tmp/example.jpg"
        );
    }

    #[test]
    fn preserves_unknown_events() {
        let event = ThreadEvent::try_from(json!({
            "type": "system",
            "subtype": "mystery",
            "session_id": "T-1"
        }))
        .expect("event");

        assert!(matches!(event, ThreadEvent::Unknown(_)));
    }

    #[test]
    fn content_block_text_serialization_matches_schema() {
        let value = serde_json::to_value(ContentBlock::Text {
            text: "hello".to_string(),
        })
        .expect("serialize");

        assert_eq!(value["type"], "text");
        assert_eq!(value["text"], "hello");
    }
}
