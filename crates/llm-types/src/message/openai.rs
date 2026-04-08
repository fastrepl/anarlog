use async_openai::types::{
    ChatCompletionRequestAssistantMessageContent, ChatCompletionRequestAssistantMessageContentPart,
    ChatCompletionRequestDeveloperMessageContent, ChatCompletionRequestMessage,
    ChatCompletionRequestMessageContentPartText, ChatCompletionRequestSystemMessageContent,
    ChatCompletionRequestSystemMessageContentPart, ChatCompletionRequestToolMessageContent,
    ChatCompletionRequestToolMessageContentPart, ChatCompletionRequestUserMessageContent,
    ChatCompletionRequestUserMessageContentPart, ImageDetail as OpenAiImageDetail,
};

use super::{ImageDetail, ImageUrl, Message, MessageContent, MessagePart};

impl From<OpenAiImageDetail> for ImageDetail {
    fn from(value: OpenAiImageDetail) -> Self {
        match value {
            OpenAiImageDetail::Auto => Self::Auto,
            OpenAiImageDetail::Low => Self::Low,
            OpenAiImageDetail::High => Self::High,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FromOpenAIError {
    #[error("user input audio parts are not supported")]
    UnsupportedUserInputAudio,
    #[error("assistant refusal parts are not supported")]
    UnsupportedAssistantRefusal,
}

pub trait FromOpenAI {
    fn from_openai(message: &ChatCompletionRequestMessage) -> Result<Self, FromOpenAIError>
    where
        Self: Sized;
}

impl FromOpenAI for Message {
    fn from_openai(message: &ChatCompletionRequestMessage) -> Result<Self, FromOpenAIError> {
        match message {
            ChatCompletionRequestMessage::Developer(developer) => Ok(Message {
                role: "developer".into(),
                content: convert_developer_content(&developer.content),
            }),
            ChatCompletionRequestMessage::System(system) => {
                Ok(Message::system(convert_system_content(&system.content)))
            }
            ChatCompletionRequestMessage::Assistant(assistant) => {
                let content = match &assistant.content {
                    Some(content) => convert_assistant_content(content)?,
                    None => MessageContent::default(),
                };

                Ok(Message::assistant(content))
            }
            ChatCompletionRequestMessage::User(user) => {
                Ok(Message::user(convert_user_content(&user.content)?))
            }
            ChatCompletionRequestMessage::Tool(tool) => Ok(Message {
                role: "tool".into(),
                content: convert_tool_content(&tool.content),
            }),
            ChatCompletionRequestMessage::Function(function) => Ok(Message {
                role: "function".into(),
                content: function.content.clone().unwrap_or_default().into(),
            }),
        }
    }
}

fn convert_developer_content(
    content: &ChatCompletionRequestDeveloperMessageContent,
) -> MessageContent {
    match content {
        ChatCompletionRequestDeveloperMessageContent::Text(text) => text.clone().into(),
        ChatCompletionRequestDeveloperMessageContent::Array(parts) => MessageContent::Parts(
            parts
                .iter()
                .map(|text: &ChatCompletionRequestMessageContentPartText| {
                    MessagePart::text(text.text.clone())
                })
                .collect(),
        ),
    }
}

fn convert_system_content(content: &ChatCompletionRequestSystemMessageContent) -> MessageContent {
    match content {
        ChatCompletionRequestSystemMessageContent::Text(text) => text.clone().into(),
        ChatCompletionRequestSystemMessageContent::Array(parts) => MessageContent::Parts(
            parts
                .iter()
                .map(|part| match part {
                    ChatCompletionRequestSystemMessageContentPart::Text(text) => {
                        MessagePart::text(text.text.clone())
                    }
                })
                .collect(),
        ),
    }
}

fn convert_assistant_content(
    content: &ChatCompletionRequestAssistantMessageContent,
) -> Result<MessageContent, FromOpenAIError> {
    match content {
        ChatCompletionRequestAssistantMessageContent::Text(text) => Ok(text.clone().into()),
        ChatCompletionRequestAssistantMessageContent::Array(parts) => {
            let mut converted = Vec::with_capacity(parts.len());
            for part in parts {
                match part {
                    ChatCompletionRequestAssistantMessageContentPart::Text(text) => {
                        converted.push(MessagePart::text(text.text.clone()));
                    }
                    ChatCompletionRequestAssistantMessageContentPart::Refusal(_) => {
                        return Err(FromOpenAIError::UnsupportedAssistantRefusal);
                    }
                }
            }
            Ok(MessageContent::Parts(converted))
        }
    }
}

fn convert_user_content(
    content: &ChatCompletionRequestUserMessageContent,
) -> Result<MessageContent, FromOpenAIError> {
    match content {
        ChatCompletionRequestUserMessageContent::Text(text) => Ok(text.clone().into()),
        ChatCompletionRequestUserMessageContent::Array(parts) => {
            let mut converted = Vec::with_capacity(parts.len());
            for part in parts {
                match part {
                    ChatCompletionRequestUserMessageContentPart::Text(text) => {
                        converted.push(MessagePart::text(text.text.clone()));
                    }
                    ChatCompletionRequestUserMessageContentPart::ImageUrl(image) => {
                        converted.push(MessagePart::ImageUrl {
                            image_url: ImageUrl {
                                url: image.image_url.url.clone(),
                                detail: image.image_url.detail.clone().map(Into::into),
                            },
                        });
                    }
                    ChatCompletionRequestUserMessageContentPart::InputAudio(_) => {
                        return Err(FromOpenAIError::UnsupportedUserInputAudio);
                    }
                }
            }
            Ok(MessageContent::Parts(converted))
        }
    }
}

fn convert_tool_content(content: &ChatCompletionRequestToolMessageContent) -> MessageContent {
    match content {
        ChatCompletionRequestToolMessageContent::Text(text) => text.clone().into(),
        ChatCompletionRequestToolMessageContent::Array(parts) => MessageContent::Parts(
            parts
                .iter()
                .map(|part| match part {
                    ChatCompletionRequestToolMessageContentPart::Text(text) => {
                        MessagePart::text(text.text.clone())
                    }
                })
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_openai_user_parts_with_image() {
        let message: ChatCompletionRequestMessage = serde_json::from_value(serde_json::json!({
            "role": "user",
            "content": [
                { "type": "text", "text": "Describe this " },
                {
                    "type": "image_url",
                    "image_url": {
                        "url": "file:///tmp/test.png",
                        "detail": "high"
                    }
                }
            ]
        }))
        .unwrap();

        let converted = Message::from_openai(&message).unwrap();

        assert_eq!(
            converted,
            Message::user(vec![
                MessagePart::text("Describe this "),
                MessagePart::image_url_with_detail("file:///tmp/test.png", ImageDetail::High),
            ])
        );
    }

    #[test]
    fn rejects_unsupported_openai_assistant_refusal_parts() {
        let message: ChatCompletionRequestMessage = serde_json::from_value(serde_json::json!({
            "role": "assistant",
            "content": [
                { "type": "refusal", "refusal": "no" }
            ]
        }))
        .unwrap();

        let error = Message::from_openai(&message).unwrap_err();

        assert_eq!(error, FromOpenAIError::UnsupportedAssistantRefusal);
    }
}
