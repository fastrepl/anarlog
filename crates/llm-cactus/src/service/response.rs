use axum::response::{IntoResponse, Response, sse};
use futures_util::{StreamExt, stream};
use hypr_llm_types::{Response as LlmResponse, StreamingParser};

pub(super) fn status_code_for_model_error(error: &hypr_cactus::Error) -> axum::http::StatusCode {
    if error.is_invalid_request() {
        axum::http::StatusCode::BAD_REQUEST
    } else {
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    }
}

fn model_name(model: &Option<String>) -> &str {
    model.as_deref().unwrap_or("cactus")
}

pub(super) fn build_streaming_response(
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

pub(super) async fn build_non_streaming_response(
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
            return (status_code_for_model_error(&e), e.to_string()).into_response();
        }
        Err(_) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
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
