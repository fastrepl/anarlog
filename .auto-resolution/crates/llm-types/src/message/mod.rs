#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub role: String,
    pub content: MessageContent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

impl Message {
    pub fn system(content: impl Into<MessageContent>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
            name: None,
            audio: None,
            tool_calls: None,
        }
    }

    pub fn user(content: impl Into<MessageContent>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
            name: None,
            audio: None,
            tool_calls: None,
        }
    }

    pub fn assistant(content: impl Into<MessageContent>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
            name: None,
            audio: None,
            tool_calls: None,
        }
    }

    pub fn tool(content: impl Into<MessageContent>) -> Self {
        Self {
            role: "tool".into(),
            content: content.into(),
            name: None,
            audio: None,
            tool_calls: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_audio(mut self, audio: Vec<String>) -> Self {
        self.audio = Some(audio);
        self
    }

    pub fn with_tool_calls(mut self, tool_calls: Vec<ToolCall>) -> Self {
        self.tool_calls = Some(tool_calls);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<MessagePart>),
}

impl Default for MessageContent {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

impl MessageContent {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Parts(_) => None,
        }
    }
}

impl From<String> for MessageContent {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for MessageContent {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<&String> for MessageContent {
    fn from(value: &String) -> Self {
        Self::Text(value.clone())
    }
}

impl From<Vec<MessagePart>> for MessageContent {
    fn from(value: Vec<MessagePart>) -> Self {
        Self::Parts(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessagePart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
}

impl MessagePart {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    pub fn image_url(url: impl Into<String>) -> Self {
        Self::ImageUrl {
            image_url: ImageUrl {
                url: url.into(),
                detail: None,
            },
        }
    }

    pub fn image_url_with_detail(url: impl Into<String>, detail: ImageDetail) -> Self {
        Self::ImageUrl {
            image_url: ImageUrl {
                url: url.into(),
                detail: Some(detail),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<ImageDetail>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageDetail {
    Auto,
    Low,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

impl ToolCall {
    pub fn new(name: impl Into<String>, arguments: serde_json::Value) -> Self {
        Self {
            name: name.into(),
            arguments,
        }
    }
}

mod openai;

pub use openai::{FromOpenAI, FromOpenAIError};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_text_message_content_as_plain_string() {
        let message = Message::user("hello");
        let json = serde_json::to_value(message).unwrap();

        assert_eq!(
            json,
            serde_json::json!({
                "role": "user",
                "content": "hello"
            })
        );
    }

    #[test]
    fn serializes_optional_native_fields_when_present() {
        let message = Message::assistant("hello")
            .with_name("assistant-tool")
            .with_audio(vec!["/tmp/audio.wav".into()])
            .with_tool_calls(vec![ToolCall::new(
                "lookup",
                serde_json::json!({ "id": 1 }),
            )]);

        let json = serde_json::to_value(message).unwrap();

        assert_eq!(
            json,
            serde_json::json!({
                "role": "assistant",
                "content": "hello",
                "name": "assistant-tool",
                "audio": ["/tmp/audio.wav"],
                "tool_calls": [
                    {
                        "name": "lookup",
                        "arguments": { "id": 1 }
                    }
                ]
            })
        );
    }
}
