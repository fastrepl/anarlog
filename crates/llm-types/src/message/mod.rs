#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub role: String,
    pub content: MessageContent,
}

impl Message {
    pub fn system(content: impl Into<MessageContent>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<MessageContent>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<MessageContent>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
        }
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
}
