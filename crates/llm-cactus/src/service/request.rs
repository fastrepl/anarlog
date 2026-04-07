use hypr_llm_types::{ImageDetail, MessageContent, MessagePart};

#[derive(serde::Deserialize)]
pub(super) struct ChatCompletionRequest {
    #[serde(default)]
    pub(super) model: Option<String>,
    pub(super) messages: Vec<ChatMessage>,
    #[serde(default)]
    pub(super) stream: Option<bool>,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    top_p: Option<f32>,
    #[serde(default)]
    max_tokens: Option<u32>,
    #[serde(default)]
    max_completion_tokens: Option<u32>,
}

#[derive(serde::Deserialize)]
pub(super) struct ChatMessage {
    role: String,
    #[serde(default)]
    content: Option<ChatMessageContent>,
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum ChatMessageContent {
    Text(String),
    Parts(Vec<ChatMessagePart>),
}

#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ChatMessagePart {
    Text { text: String },
    ImageUrl { image_url: ChatImageUrl },
}

#[derive(serde::Deserialize)]
struct ChatImageUrl {
    url: String,
    #[serde(default)]
    detail: Option<ImageDetail>,
}

pub(super) fn convert_messages(
    messages: &[ChatMessage],
) -> Result<Vec<hypr_llm_types::Message>, String> {
    messages.iter().map(convert_message).collect()
}

fn convert_message(message: &ChatMessage) -> Result<hypr_llm_types::Message, String> {
    let content = match &message.content {
        Some(ChatMessageContent::Text(text)) => MessageContent::Text(text.clone()),
        Some(ChatMessageContent::Parts(parts)) => {
            let mut converted = Vec::with_capacity(parts.len());
            for part in parts {
                match part {
                    ChatMessagePart::Text { text } => {
                        converted.push(MessagePart::text(text.clone()))
                    }
                    ChatMessagePart::ImageUrl { image_url } => {
                        if message.role != "user" {
                            return Err(
                                "image_url parts are only supported for user messages".into()
                            );
                        }
                        converted.push(MessagePart::image_url_with_detail(
                            image_url.url.clone(),
                            image_url.detail.clone().unwrap_or(ImageDetail::Auto),
                        ));
                    }
                }
            }
            MessageContent::Parts(converted)
        }
        None => MessageContent::default(),
    };

    Ok(hypr_llm_types::Message {
        role: message.role.clone(),
        content,
    })
}

pub(super) fn build_options(request: &ChatCompletionRequest) -> hypr_cactus::CompleteOptions {
    hypr_cactus::CompleteOptions {
        temperature: request.temperature,
        top_p: request.top_p,
        max_tokens: request.max_completion_tokens.or(request.max_tokens),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::{ModelManager, service::CompleteService};
    use axum::{body::Body, body::to_bytes, http::Request, http::StatusCode};
    use tower::Service;

    struct TestImagePath {
        path: PathBuf,
    }

    impl TestImagePath {
        fn create_file() -> Self {
            let path = unique_temp_path("png");
            std::fs::write(&path, b"image").unwrap();
            Self { path }
        }

        fn create_dir() -> Self {
            let path = unique_temp_path("dir");
            std::fs::create_dir(&path).unwrap();
            Self { path }
        }

        fn url(&self) -> String {
            file_url(&self.path)
        }
    }

    impl Drop for TestImagePath {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
            let _ = std::fs::remove_dir(&self.path);
        }
    }

    fn unique_temp_path(suffix: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "llm-cactus-test-{}-{timestamp}-{suffix}",
            std::process::id()
        ))
    }

    fn make_request(content: serde_json::Value, stream: bool) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/v1/chat/completions")
            .body(Body::from(
                serde_json::json!({
                    "stream": stream,
                    "messages": [content]
                })
                .to_string(),
            ))
            .unwrap()
    }

    fn file_url(path: &std::path::Path) -> String {
        format!("file://{}", path.to_string_lossy())
    }

    async fn assert_bad_request(content: serde_json::Value, stream: bool, needle: &str) {
        let manager = ModelManager::<hypr_cactus::Model>::builder().build();
        let mut service = CompleteService::new(manager);
        let response = service.call(make_request(content, stream)).await.unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains(needle), "{text}");
    }

    #[test]
    fn converts_text_only_messages() {
        let request = vec![ChatMessage {
            role: "user".into(),
            content: Some(ChatMessageContent::Text("hello".into())),
        }];

        let messages = convert_messages(&request).unwrap();

        assert_eq!(messages, vec![hypr_llm_types::Message::user("hello")]);
    }

    #[test]
    fn converts_openai_style_image_parts() {
        let request = vec![ChatMessage {
            role: "user".into(),
            content: Some(ChatMessageContent::Parts(vec![
                ChatMessagePart::Text {
                    text: "describe ".into(),
                },
                ChatMessagePart::ImageUrl {
                    image_url: ChatImageUrl {
                        url: "file:///tmp/test.png".into(),
                        detail: Some(ImageDetail::High),
                    },
                },
            ])),
        }];

        let messages = convert_messages(&request).unwrap();

        assert_eq!(
            messages,
            vec![hypr_llm_types::Message::user(vec![
                MessagePart::text("describe "),
                MessagePart::image_url_with_detail("file:///tmp/test.png", ImageDetail::High),
            ])]
        );
    }

    #[test]
    fn rejects_image_parts_for_non_user_messages() {
        let request = vec![ChatMessage {
            role: "assistant".into(),
            content: Some(ChatMessageContent::Parts(vec![ChatMessagePart::ImageUrl {
                image_url: ChatImageUrl {
                    url: "file:///tmp/test.png".into(),
                    detail: None,
                },
            }])),
        }];

        let error = convert_messages(&request).unwrap_err();

        assert!(error.contains("only supported for user messages"));
    }

    #[tokio::test]
    async fn returns_bad_request_for_invalid_non_user_image_part() {
        assert_bad_request(
            serde_json::json!({
                "role": "assistant",
                "content": [{
                    "type": "image_url",
                    "image_url": { "url": "file:///tmp/test.png" }
                }]
            }),
            false,
            "only supported for user messages",
        )
        .await;
    }

    #[tokio::test]
    async fn returns_bad_request_for_non_file_image_urls_in_streaming_and_non_streaming() {
        for stream in [false, true] {
            assert_bad_request(
                serde_json::json!({
                    "role": "user",
                    "content": [{
                        "type": "image_url",
                        "image_url": { "url": "https://example.com/test.png" }
                    }]
                }),
                stream,
                "local file:// URL",
            )
            .await;
        }
    }

    #[tokio::test]
    async fn returns_bad_request_for_malformed_file_urls_in_streaming_and_non_streaming() {
        for stream in [false, true] {
            assert_bad_request(
                serde_json::json!({
                    "role": "user",
                    "content": [{
                        "type": "image_url",
                        "image_url": { "url": "file://remote-host/tmp/test.png" }
                    }]
                }),
                stream,
                "must resolve to a local path",
            )
            .await;
        }
    }

    #[tokio::test]
    async fn returns_bad_request_for_missing_local_image_files_in_streaming_and_non_streaming() {
        let missing = unique_temp_path("missing.png");
        let url = file_url(&missing);

        for stream in [false, true] {
            assert_bad_request(
                serde_json::json!({
                    "role": "user",
                    "content": [{
                        "type": "image_url",
                        "image_url": { "url": url }
                    }]
                }),
                stream,
                "image file is not accessible",
            )
            .await;
        }
    }

    #[tokio::test]
    async fn returns_bad_request_for_non_file_targets_in_streaming_and_non_streaming() {
        let dir = TestImagePath::create_dir();

        for stream in [false, true] {
            assert_bad_request(
                serde_json::json!({
                    "role": "user",
                    "content": [{
                        "type": "image_url",
                        "image_url": { "url": dir.url() }
                    }]
                }),
                stream,
                "must point to a file",
            )
            .await;
        }
    }

    #[test]
    fn accepts_existing_local_file_urls() {
        let image = TestImagePath::create_file();
        let messages = convert_messages(&[ChatMessage {
            role: "user".into(),
            content: Some(ChatMessageContent::Parts(vec![ChatMessagePart::ImageUrl {
                image_url: ChatImageUrl {
                    url: image.url(),
                    detail: Some(ImageDetail::Low),
                },
            }])),
        }])
        .unwrap();

        hypr_cactus::validate_messages(&messages).unwrap();
    }
}
