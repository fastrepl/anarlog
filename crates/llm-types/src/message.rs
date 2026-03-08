use async_openai::types::{
    ChatCompletionRequestAssistantMessageContent, ChatCompletionRequestMessage,
    ChatCompletionRequestSystemMessageContent, ChatCompletionRequestToolMessageContent,
    ChatCompletionRequestUserMessageContent,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
        }
    }
}

pub trait FromOpenAI {
    fn from_openai(message: &ChatCompletionRequestMessage) -> Self;
}

impl FromOpenAI for Message {
    fn from_openai(message: &ChatCompletionRequestMessage) -> Self {
        match message {
            ChatCompletionRequestMessage::System(system) => {
                let content = match &system.content {
                    ChatCompletionRequestSystemMessageContent::Text(text) => text,
                    _ => todo!(),
                };

                Message::system(content.clone())
            }
            ChatCompletionRequestMessage::Assistant(assistant) => {
                let content = match &assistant.content {
                    Some(ChatCompletionRequestAssistantMessageContent::Text(text)) => text,
                    _ => todo!(),
                };

                Message::assistant(content.clone())
            }
            ChatCompletionRequestMessage::User(user) => {
                let content = match &user.content {
                    ChatCompletionRequestUserMessageContent::Text(text) => text,
                    _ => todo!(),
                };

                Message::user(content.clone())
            }
            ChatCompletionRequestMessage::Tool(tool) => {
                let content = match &tool.content {
                    ChatCompletionRequestToolMessageContent::Text(text) => text,
                    _ => todo!(),
                };

                Message {
                    role: "tool".into(),
                    content: content.clone(),
                }
            }
            _ => todo!(),
        }
    }
}
