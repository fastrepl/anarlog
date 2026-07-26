mod client;
mod error;
mod types;

pub use client::*;
pub use error::*;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;

    fn client() -> Client {
        let api_key = std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY not set");
        Client::new(api_key)
    }

    macro_rules! test_non_streaming {
        ($name:ident, $model:expr) => {
            test_non_streaming!($name, $model, 50);
        };
        ($name:ident, $model:expr, $max_tokens:expr) => {
            #[tokio::test]
            #[ignore]
            async fn $name() {
                let req = ChatCompletionRequest {
                    model: Some($model.to_string()),
                    max_tokens: Some($max_tokens),
                    messages: vec![ChatMessage::new(Role::User, "Say hello in one word.")],
                    ..Default::default()
                };

                let resp = client().chat_completion(&req).await.unwrap();
                println!("[non-streaming] model={}", resp.model);

                assert!(!resp.choices.is_empty(), "no choices returned");
                let msg = &resp.choices[0].message;

                if let Some(reasoning) = msg.reasoning.as_deref() {
                    println!("[non-streaming] reasoning={reasoning}");
                }
                if let Some(content) = msg.content.as_ref().and_then(|c| c.as_text()) {
                    println!("[non-streaming] content={content}");
                }
                println!(
                    "[non-streaming] finish_reason={:?}",
                    resp.choices[0].finish_reason
                );
                println!("[non-streaming] usage={:?}", resp.usage);

                let has_output = msg
                    .content
                    .as_ref()
                    .and_then(|c| c.as_text())
                    .is_some_and(|t| !t.is_empty())
                    || msg.reasoning.as_ref().is_some_and(|r| !r.is_empty());
                assert!(has_output, "expected content or reasoning, got: {msg:?}");
            }
        };
    }

    macro_rules! test_streaming {
        ($name:ident, $model:expr) => {
            test_streaming!($name, $model, 50);
        };
        ($name:ident, $model:expr, $max_tokens:expr) => {
            #[tokio::test]
            #[ignore]
            async fn $name() {
                let req = ChatCompletionRequest {
                    model: Some($model.to_string()),
                    max_tokens: Some($max_tokens),
                    messages: vec![ChatMessage::new(Role::User, "Say hello in one word.")],
                    ..Default::default()
                };

                let mut stream = client().chat_completion_stream(&req).await.unwrap();
                let mut content = String::new();
                let mut reasoning = String::new();

                while let Some(chunk) = stream.next().await {
                    let chunk = chunk.unwrap();
                    if let Some(choice) = chunk.choices.first() {
                        if let Some(c) = choice.delta.content.as_deref() {
                            content.push_str(c);
                        }
                        if let Some(r) = choice.delta.reasoning.as_deref() {
                            reasoning.push_str(r);
                        }
                    }
                }

                if !reasoning.is_empty() {
                    println!("[streaming] reasoning={reasoning}");
                }
                println!("[streaming] content={content}");

                let has_output = !content.is_empty() || !reasoning.is_empty();
                assert!(has_output, "expected content or reasoning from stream");
            }
        };
    }

    macro_rules! test_tool_calling_non_streaming {
        ($name:ident, $model:expr) => {
            #[tokio::test]
            #[ignore]
            async fn $name() {
                let req = ChatCompletionRequest {
                    model: Some($model.to_string()),
                    max_tokens: Some(1000),
                    messages: vec![ChatMessage::new(
                        Role::User,
                        "What is the weather in Seoul?",
                    )],
                    tools: Some(vec![Tool::function(
                        "get_weather",
                        "Get the current weather for a location",
                        serde_json::json!({
                            "type": "object",
                            "properties": {
                                "location": {
                                    "type": "string",
                                    "description": "City name"
                                }
                            },
                            "required": ["location"]
                        }),
                    )]),
                    tool_choice: Some(ToolChoice::auto()),
                    ..Default::default()
                };

                let resp = client().chat_completion(&req).await.unwrap();
                println!("[tool non-streaming] model={}", resp.model);
                println!(
                    "[tool non-streaming] finish_reason={:?}",
                    resp.choices[0].finish_reason
                );

                assert!(!resp.choices.is_empty(), "no choices returned");
                let msg = &resp.choices[0].message;
                let tool_calls = msg.tool_calls.as_ref().expect("expected tool_calls");

                assert!(!tool_calls.is_empty(), "no tool calls returned");
                let tc = &tool_calls[0];
                println!(
                    "[tool non-streaming] id={:?} name={:?} args={:?}",
                    tc.id,
                    tc.function.name,
                    tc.function.arguments,
                );

                assert_eq!(tc.function.name.as_deref(), Some("get_weather"));
                let args: serde_json::Value =
                    serde_json::from_str(tc.function.arguments.as_deref().unwrap()).unwrap();
                assert!(args.get("location").is_some(), "missing location arg: {args}");
            }
        };
    }

    macro_rules! test_tool_calling_streaming {
        ($name:ident, $model:expr) => {
            #[tokio::test]
            #[ignore]
            async fn $name() {
                let req = ChatCompletionRequest {
                    model: Some($model.to_string()),
                    max_tokens: Some(1000),
                    messages: vec![ChatMessage::new(
                        Role::User,
                        "What is the weather in Seoul?",
                    )],
                    tools: Some(vec![Tool::function(
                        "get_weather",
                        "Get the current weather for a location",
                        serde_json::json!({
                            "type": "object",
                            "properties": {
                                "location": {
                                    "type": "string",
                                    "description": "City name"
                                }
                            },
                            "required": ["location"]
                        }),
                    )]),
                    tool_choice: Some(ToolChoice::auto()),
                    ..Default::default()
                };

                let mut stream = client().chat_completion_stream(&req).await.unwrap();
                let mut tool_call_name = String::new();
                let mut tool_call_args = String::new();
                let mut tool_call_id = String::new();

                while let Some(chunk) = stream.next().await {
                    let chunk = chunk.unwrap();
                    if let Some(choice) = chunk.choices.first() {
                        if let Some(tcs) = &choice.delta.tool_calls {
                            for tc in tcs {
                                if let Some(id) = &tc.id {
                                    tool_call_id.push_str(id);
                                }
                                if let Some(name) = &tc.function.name {
                                    tool_call_name.push_str(name);
                                }
                                if let Some(args) = &tc.function.arguments {
                                    tool_call_args.push_str(args);
                                }
                            }
                        }
                    }
                }

                println!("[tool streaming] id={tool_call_id}");
                println!("[tool streaming] name={tool_call_name}");
                println!("[tool streaming] args={tool_call_args}");

                assert_eq!(tool_call_name, "get_weather");
                assert!(!tool_call_id.is_empty(), "no tool call id");
                let args: serde_json::Value = serde_json::from_str(&tool_call_args).unwrap();
                assert!(args.get("location").is_some(), "missing location arg: {args}");
            }
        };
    }

    test_non_streaming!(non_streaming_lfm, "liquid/lfm-2.2-6b");
    test_non_streaming!(non_streaming_gpt_oss, "openai/gpt-oss-120b:exacto", 200);
    test_streaming!(streaming_lfm, "liquid/lfm-2.2-6b");
    test_streaming!(streaming_gpt_oss, "openai/gpt-oss-120b:exacto", 200);
    test_tool_calling_non_streaming!(
        tool_calling_non_streaming_haiku,
        "anthropic/claude-haiku-4.5"
    );
    test_tool_calling_streaming!(tool_calling_streaming_haiku, "anthropic/claude-haiku-4.5");
}
