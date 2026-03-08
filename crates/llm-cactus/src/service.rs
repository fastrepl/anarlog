use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::{IntoResponse, Response, sse},
};
use futures_util::{StreamExt, stream};
use hypr_llm_types::{Response as LlmResponse, StreamingParser};
use tower::Service;

use crate::ModelManager;

#[derive(Clone)]
pub struct CompleteService {
    manager: ModelManager,
}

impl CompleteService {
    pub fn new(manager: ModelManager) -> Self {
        Self { manager }
    }
}

impl Service<Request<Body>> for CompleteService {
    type Response = Response;
    type Error = crate::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let manager = self.manager.clone();

        Box::pin(async move {
            let body_bytes = match axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024).await {
                Ok(b) => b,
                Err(e) => {
                    return Ok((StatusCode::BAD_REQUEST, e.to_string()).into_response());
                }
            };

            let request: ChatCompletionRequest = match serde_json::from_slice(&body_bytes) {
                Ok(r) => r,
                Err(e) => {
                    return Ok((StatusCode::BAD_REQUEST, e.to_string()).into_response());
                }
            };

            let model = match manager.get(request.model.as_deref()).await {
                Ok(m) => m,
                Err(e) => {
                    return Ok((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response());
                }
            };

            let messages = convert_messages(&request.messages);
            let options = build_options(&request);

            if request.stream.unwrap_or(false) {
                let completion_stream =
                    match hypr_cactus::complete_stream(&model, messages, options) {
                        Ok(s) => s,
                        Err(e) => {
                            return Ok(
                                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                            );
                        }
                    };

                Ok(build_streaming_response(completion_stream, &request.model))
            } else {
                Ok(build_non_streaming_response(&model, messages, options, &request.model).await)
            }
        })
    }
}

#[derive(serde::Deserialize)]
struct ChatCompletionRequest {
    #[serde(default)]
    model: Option<String>,
    messages: Vec<ChatMessage>,
    #[serde(default)]
    stream: Option<bool>,
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
struct ChatMessage {
    role: String,
    #[serde(default)]
    content: Option<String>,
}

fn convert_messages(messages: &[ChatMessage]) -> Vec<hypr_llm_types::Message> {
    messages
        .iter()
        .map(|m| hypr_llm_types::Message {
            role: m.role.clone(),
            content: m.content.clone().unwrap_or_default(),
        })
        .collect()
}

fn build_options(request: &ChatCompletionRequest) -> hypr_cactus::CompleteOptions {
    hypr_cactus::CompleteOptions {
        temperature: request.temperature,
        top_p: request.top_p,
        max_tokens: request.max_completion_tokens.or(request.max_tokens),
        ..Default::default()
    }
}

fn model_name(model: &Option<String>) -> &str {
    model.as_deref().unwrap_or("cactus")
}

fn build_streaming_response(
    completion_stream: hypr_cactus::CompletionStream,
    model: &Option<String>,
) -> Response {
    let id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let model_name = model_name(model).to_string();

    let id_for_events = id.clone();
    let model_for_events = model_name.clone();

    let data_events = completion_stream.filter_map(move |item| {
        let id = id_for_events.clone();
        let model_name = model_for_events.clone();

        async move {
            let delta = match item {
                LlmResponse::TextDelta(text) => {
                    serde_json::json!({ "content": text, "role": "assistant" })
                }
                LlmResponse::ToolCall { name, arguments } => {
                    serde_json::json!({
                        "tool_calls": [{
                            "index": 0,
                            "id": format!("call_{}", uuid::Uuid::new_v4()),
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": serde_json::to_string(&arguments).unwrap_or_default()
                            }
                        }]
                    })
                }
                LlmResponse::Reasoning(_) => return None,
            };

            let chunk = serde_json::json!({
                "id": id,
                "object": "chat.completion.chunk",
                "created": created,
                "model": model_name,
                "choices": [{
                    "index": 0,
                    "delta": delta,
                    "finish_reason": serde_json::Value::Null
                }]
            });

            Some(Ok::<_, std::convert::Infallible>(
                sse::Event::default().data(serde_json::to_string(&chunk).unwrap_or_default()),
            ))
        }
    });

    let stop_chunk = serde_json::json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model_name,
        "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]
    });

    let stop_event = stream::once(futures_util::future::ready(
        Ok::<_, std::convert::Infallible>(
            sse::Event::default().data(serde_json::to_string(&stop_chunk).unwrap_or_default()),
        ),
    ));

    let done_event = stream::once(futures_util::future::ready(
        Ok::<_, std::convert::Infallible>(sse::Event::default().data("[DONE]")),
    ));

    let event_stream = data_events.chain(stop_event).chain(done_event);

    sse::Sse::new(event_stream).into_response()
}

async fn build_non_streaming_response(
    model: &std::sync::Arc<hypr_cactus::Model>,
    messages: Vec<hypr_llm_types::Message>,
    options: hypr_cactus::CompleteOptions,
    model_label: &Option<String>,
) -> Response {
    let model = std::sync::Arc::clone(model);

    let result = tokio::task::spawn_blocking(move || model.complete(&messages, &options)).await;

    let completion = match result {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "worker task panicked".to_string(),
            )
                .into_response();
        }
    };

    let mut parser = StreamingParser::new();
    let mut responses = parser.process_chunk(&completion.text);
    if let Some(r) = parser.flush() {
        responses.push(r);
    }

    let mut content = String::new();
    let mut tool_calls: Vec<serde_json::Value> = Vec::new();

    for item in responses {
        match item {
            LlmResponse::TextDelta(text) => content.push_str(&text),
            LlmResponse::ToolCall { name, arguments } => {
                tool_calls.push(serde_json::json!({
                    "id": format!("call_{}", uuid::Uuid::new_v4()),
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": serde_json::to_string(&arguments).unwrap_or_default()
                    }
                }));
            }
            LlmResponse::Reasoning(_) => {}
        }
    }

    let id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut message = serde_json::json!({ "role": "assistant" });
    if !content.is_empty() {
        message["content"] = serde_json::Value::String(content);
    }
    if !tool_calls.is_empty() {
        message["tool_calls"] = serde_json::Value::Array(tool_calls);
    }

    let response = serde_json::json!({
        "id": id,
        "object": "chat.completion",
        "created": created,
        "model": model_name(model_label),
        "choices": [{
            "index": 0,
            "message": message,
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": completion.prefill_tokens,
            "completion_tokens": completion.decode_tokens,
            "total_tokens": completion.total_tokens
        }
    });

    axum::Json(response).into_response()
}
