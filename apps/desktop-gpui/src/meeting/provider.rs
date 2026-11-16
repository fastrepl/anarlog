use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
    time::Duration,
};

use desktop_runtime::{CancellationToken, Result, ServiceError};
use futures::{
    StreamExt,
    future::BoxFuture,
    stream::{self, BoxStream},
};
use serde_json::{Value, json};

use super::{
    ai::{Message, Part, ProviderAdapter, ProviderEvent, ProviderResolver, Request, Role},
    config::{Connection, ProviderKind, ProviderServices, preferences},
    model::{MAX_TEXT, failure},
};

#[derive(Clone, Copy)]
enum Protocol {
    OpenAi,
    Responses,
    Anthropic,
    Google,
}

pub struct HttpProvider {
    client: reqwest::Client,
    connection: Connection,
    protocol: Protocol,
}

impl ProviderServices {
    pub fn ai_resolver(&self) -> ProviderResolver {
        let services = self.clone();
        Arc::new(move |_| {
            let services = services.clone();
            Box::pin(async move {
                let settings = preferences(&services.runtime).await?;
                if settings.text("current_llm_provider") == "apple_foundation" {
                    return Ok(
                        Arc::new(super::foundation::FoundationModel) as Arc<dyn ProviderAdapter>
                    );
                }
                let mut connection = services
                    .connection(&settings, ProviderKind::Llm, false)
                    .await?;
                connection.api_key = super::subscription::resolve(
                    &services,
                    &connection.provider,
                    connection.api_key,
                )
                .await?;
                Ok(Arc::new(HttpProvider::new(connection)?) as Arc<dyn ProviderAdapter>)
            })
        })
    }
}

impl HttpProvider {
    pub fn new(connection: Connection) -> Result<Self> {
        let protocol = match connection.provider.as_str() {
            "anthropic" | "claude" => Protocol::Anthropic,
            "google_generative_ai" => Protocol::Google,
            "openai" | "chatgpt" | "azure_openai" => Protocol::Responses,
            _ => Protocol::OpenAi,
        };
        Ok(Self {
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(15))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(failure)?,
            connection,
            protocol,
        })
    }
}

impl ProviderAdapter for HttpProvider {
    fn stream(
        &self,
        request: Request,
    ) -> BoxFuture<'static, Result<BoxStream<'static, Result<ProviderEvent>>>> {
        let protocol = self.protocol;
        let base = self.connection.base_url.trim_end_matches('/');
        let model = &self.connection.model;
        let tools = tool_definitions();
        let subscription = super::subscription::parse(&self.connection.api_key);
        let key = subscription
            .as_ref()
            .map_or(self.connection.api_key.as_str(), |s| &s.access);
        let systems = request
            .messages
            .iter()
            .filter(|m| m.role == Role::System)
            .flat_map(|m| &m.parts)
            .filter_map(|p| {
                if let Part::Text { text } = p {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        let (url, mut body) = match protocol {
            Protocol::OpenAi => (
                format!("{base}/chat/completions"),
                json!({
                    "model": model, "stream": true, "messages": openai_messages(&request.messages),
                }),
            ),
            Protocol::Responses => (
                format!("{base}/responses"),
                json!({
                    "model": model, "stream": true, "store": false,
                    "instructions": systems, "input": responses_messages(&request.messages),
                }),
            ),
            Protocol::Anthropic => (
                format!("{base}/messages"),
                json!({
                    "model": model, "stream": true, "max_tokens": 8192, "system": systems,
                    "messages": anthropic_messages(&request.messages),
                }),
            ),
            Protocol::Google => (
                format!("{base}/models/{model}:streamGenerateContent?alt=sse"),
                json!({
                    "systemInstruction": {"parts": [{"text": systems}]},
                    "contents": google_messages(&request.messages),
                }),
            ),
        };
        if request.tools_allowed {
            body["tools"] = match protocol {
                Protocol::OpenAi => json!(tools.iter().map(|tool| json!({"type": "function", "function": tool})).collect::<Vec<_>>()),
                Protocol::Responses => json!(tools.iter().map(|tool| json!({"type":"function", "name":tool["name"], "description":tool["description"], "parameters":tool["parameters"], "strict":false})).collect::<Vec<_>>()),
                Protocol::Anthropic => json!(tools.iter().map(|tool| json!({"name": tool["name"], "description": tool["description"], "input_schema": tool["parameters"]})).collect::<Vec<_>>()),
                Protocol::Google => json!([{"functionDeclarations": tools}]),
            };
        }
        if self.connection.provider == "claude" && subscription.is_some() {
            body["system"] = json!([
                {"type":"text","text":"You are Claude Code, Anthropic's official CLI for Claude."},
                {"type":"text","text":systems}
            ]);
        }
        let mut builder = self.client.post(url).json(&body);
        match protocol {
            Protocol::Anthropic => {
                builder = builder.header("anthropic-version", "2023-06-01");
                builder = if subscription.is_some() {
                    builder.bearer_auth(key).header(
                        "anthropic-beta",
                        "oauth-2025-04-20,interleaved-thinking-2025-05-14",
                    )
                } else {
                    builder.header("x-api-key", key)
                };
            }
            Protocol::Google => {
                builder = builder.header("x-goog-api-key", key);
            }
            Protocol::OpenAi | Protocol::Responses => {
                if !key.is_empty() {
                    builder = builder.bearer_auth(key);
                }
                if matches!(
                    self.connection.provider.as_str(),
                    "azure_ai" | "azure_openai"
                ) {
                    builder = builder.header("api-key", key);
                }
            }
        }
        if self.connection.provider == "chatgpt"
            && let Some(subscription) = subscription
        {
            builder = builder
                .header("originator", "codex_cli_rs")
                .header("OpenAI-Beta", "responses=experimental")
                .header("User-Agent", "codex_cli_rs")
                .header("session_id", uuid::Uuid::new_v4().to_string());
            if let Some(account) = subscription.account_id {
                builder = builder.header("ChatGPT-Account-ID", account);
            }
        }
        if self.connection.provider == "github_copilot" {
            builder = builder
                .header("User-Agent", "GitHubCopilotChat/0.26.7")
                .header("Editor-Version", "vscode/1.99.3")
                .header("Editor-Plugin-Version", "copilot-chat/0.26.7")
                .header("Copilot-Integration-Id", "vscode-chat");
        }
        if self.connection.provider == "ollama"
            && let Ok(url) = reqwest::Url::parse(base)
        {
            builder = builder.header("Origin", url.origin().ascii_serialization());
        }
        if self.connection.provider == "anarlog" {
            builder = builder.header(
                "x-char-task",
                if request.tools_allowed {
                    "chat"
                } else {
                    "enhance"
                },
            );
        }
        Box::pin(async move {
            if body.to_string().len() > MAX_TEXT {
                return Err(failure("Provider input exceeds context limit."));
            }
            let response = tokio::select! {
                _ = request.cancellation.cancelled() => return Err(ServiceError::Cancelled),
                result = tokio::time::timeout(Duration::from_secs(60), builder.send()) =>
                    result.map_err(|_| failure("Provider connection timed out."))?
                        .map_err(|_| failure("Provider connection failed."))?,
            };
            if !response.status().is_success() {
                return Err(failure(format!(
                    "Provider returned HTTP {}.",
                    response.status().as_u16()
                )));
            }
            let state = StreamState {
                source: response
                    .bytes_stream()
                    .map(|result| {
                        result
                            .map(|bytes| bytes.to_vec())
                            .map_err(|_| failure("Provider stream interrupted."))
                    })
                    .boxed(),
                decoder: Decoder::new(protocol),
                pending: VecDeque::new(),
                cancellation: request.cancellation,
                ended: false,
            };
            Ok(stream::try_unfold(state, |mut state| async move {
                loop {
                    if state.cancellation.is_cancelled() { return Err(ServiceError::Cancelled); }
                    if let Some(event) = state.pending.pop_front() { return Ok(Some((event, state))); }
                    if state.ended { return Ok(None); }
                    let bytes = tokio::select! {
                        _ = state.cancellation.cancelled() => return Err(ServiceError::Cancelled),
                        result = tokio::time::timeout(Duration::from_secs(60), state.source.next()) =>
                            result.map_err(|_| failure("Provider stream stalled."))?,
                    };
                    match bytes {
                        Some(bytes) => state.pending.extend(state.decoder.push(&bytes?)?),
                        None => {
                            if !state.decoder.finished { return Err(failure("Provider stream ended before completion.")); }
                            state.ended = true;
                        }
                    }
                    if state.decoder.finished { state.ended = true; }
                }
            }).boxed())
        })
    }
}

struct StreamState {
    source: BoxStream<'static, Result<Vec<u8>>>,
    decoder: Decoder,
    pending: VecDeque<ProviderEvent>,
    cancellation: CancellationToken,
    ended: bool,
}

#[derive(Default)]
struct Tool {
    id: String,
    name: String,
    input: String,
}

struct Decoder {
    protocol: Protocol,
    buffer: Vec<u8>,
    tools: BTreeMap<u64, Tool>,
    finished: bool,
}

impl Decoder {
    fn new(protocol: Protocol) -> Self {
        Self {
            protocol,
            buffer: Vec::new(),
            tools: BTreeMap::new(),
            finished: false,
        }
    }

    fn push(&mut self, bytes: &[u8]) -> Result<Vec<ProviderEvent>> {
        if self.buffer.len() + bytes.len() > MAX_TEXT {
            return Err(failure("Provider event exceeds stream limit."));
        }
        self.buffer.extend_from_slice(bytes);
        let mut events = Vec::new();
        loop {
            let boundary = self
                .buffer
                .windows(2)
                .position(|v| v == b"\n\n")
                .map(|p| (p, 2))
                .or_else(|| {
                    self.buffer
                        .windows(4)
                        .position(|v| v == b"\r\n\r\n")
                        .map(|p| (p, 4))
                });
            let Some((end, length)) = boundary else { break };
            let frame: Vec<u8> = self.buffer.drain(..end + length).collect();
            let frame = std::str::from_utf8(&frame)
                .map_err(|_| failure("Invalid UTF-8 provider event."))?;
            let data = frame
                .lines()
                .filter_map(|line| line.strip_prefix("data:"))
                .map(str::trim_start)
                .collect::<Vec<_>>()
                .join("\n");
            if data.is_empty() {
                continue;
            }
            if data == "[DONE]" {
                self.finish(&mut events)?;
                continue;
            }
            let value: Value =
                serde_json::from_str(&data).map_err(|_| failure("Malformed provider event."))?;
            if value.get("error").is_some() || value["type"] == "error" {
                return Err(failure("Provider reported a stream error."));
            }
            match self.protocol {
                Protocol::OpenAi => {
                    if let Some(choices) = value["choices"].as_array() {
                        for choice in choices {
                            if choice["index"].as_u64().unwrap_or(0) != 0 {
                                continue;
                            }
                            let delta = &choice["delta"];
                            text(&mut events, &delta["content"], false);
                            text(&mut events, &delta["reasoning_content"], true);
                            text(&mut events, &delta["reasoning"], true);
                            if let Some(tools) = delta["tool_calls"].as_array() {
                                for tool in tools {
                                    let index = tool["index"]
                                        .as_u64()
                                        .ok_or_else(|| failure("Missing tool call index."))?;
                                    let item = self.tools.entry(index).or_default();
                                    append(&mut item.id, &tool["id"]);
                                    append(&mut item.name, &tool["function"]["name"]);
                                    append(&mut item.input, &tool["function"]["arguments"]);
                                }
                            }
                            if matches!(
                                choice["finish_reason"].as_str(),
                                Some("length" | "content_filter")
                            ) {
                                return Err(failure(
                                    "Provider stopped before completing its response.",
                                ));
                            }
                        }
                    }
                }
                Protocol::Anthropic => {
                    let index = value["index"].as_u64().unwrap_or(0);
                    match value["type"].as_str() {
                        Some("content_block_start")
                            if value["content_block"]["type"] == "tool_use" =>
                        {
                            let block = &value["content_block"];
                            self.tools.insert(
                                index,
                                Tool {
                                    id: block["id"].as_str().unwrap_or_default().into(),
                                    name: block["name"].as_str().unwrap_or_default().into(),
                                    input: String::new(),
                                },
                            );
                        }
                        Some("content_block_delta") => {
                            text(&mut events, &value["delta"]["text"], false);
                            text(&mut events, &value["delta"]["thinking"], true);
                            if let Some(tool) = self.tools.get_mut(&index) {
                                append(&mut tool.input, &value["delta"]["partial_json"]);
                            }
                        }
                        Some("message_delta") if value["delta"]["stop_reason"] == "max_tokens" => {
                            return Err(failure("Provider response exceeded its output limit."));
                        }
                        Some("message_stop") => self.finish(&mut events)?,
                        _ => {}
                    }
                }
                Protocol::Responses => match value["type"].as_str() {
                    Some("response.output_text.delta") => text(&mut events, &value["delta"], false),
                    Some("response.reasoning_summary_text.delta") => {
                        text(&mut events, &value["delta"], true)
                    }
                    Some("response.output_item.added")
                        if value["item"]["type"] == "function_call" =>
                    {
                        let item = &value["item"];
                        self.tools.insert(
                            value["output_index"].as_u64().unwrap_or(0),
                            Tool {
                                id: item["call_id"].as_str().unwrap_or_default().into(),
                                name: item["name"].as_str().unwrap_or_default().into(),
                                input: item["arguments"].as_str().unwrap_or_default().into(),
                            },
                        );
                    }
                    Some("response.function_call_arguments.delta") => {
                        if let Some(tool) = self
                            .tools
                            .get_mut(&value["output_index"].as_u64().unwrap_or(0))
                        {
                            append(&mut tool.input, &value["delta"]);
                        }
                    }
                    Some("response.completed") => self.finish(&mut events)?,
                    Some("response.failed" | "response.incomplete") => {
                        return Err(failure("Provider did not complete its response."));
                    }
                    _ => {}
                },
                Protocol::Google => {
                    if let Some(candidates) = value["candidates"].as_array() {
                        for candidate in candidates {
                            if let Some(parts) = candidate["content"]["parts"].as_array() {
                                for part in parts {
                                    text(&mut events, &part["text"], part["thought"] == true);
                                    if part["functionCall"].is_object() {
                                        let call = &part["functionCall"];
                                        events.push(ProviderEvent::Tool {
                                            call_id: uuid::Uuid::new_v4().to_string(),
                                            name: call["name"].as_str().unwrap_or_default().into(),
                                            input: call["args"].clone(),
                                        });
                                    }
                                }
                            }
                            if let Some(reason) = candidate["finishReason"].as_str() {
                                if reason != "STOP" {
                                    return Err(failure(
                                        "Provider response was blocked or truncated.",
                                    ));
                                }
                                self.finish(&mut events)?;
                            }
                        }
                    }
                }
            }
            if self.tools.len() > 32
                || self
                    .tools
                    .values()
                    .map(|t| t.input.len() + t.name.len() + t.id.len())
                    .sum::<usize>()
                    > MAX_TEXT
            {
                return Err(failure("Provider tool calls exceed stream limits."));
            }
        }
        Ok(events)
    }

    fn finish(&mut self, events: &mut Vec<ProviderEvent>) -> Result<()> {
        for (_, tool) in std::mem::take(&mut self.tools) {
            if tool.id.is_empty() || tool.name.is_empty() {
                return Err(failure("Incomplete provider tool call."));
            }
            events.push(ProviderEvent::Tool {
                call_id: tool.id,
                name: tool.name,
                input: serde_json::from_str(if tool.input.is_empty() {
                    "{}"
                } else {
                    &tool.input
                })
                .map_err(|_| failure("Invalid tool arguments."))?,
            });
        }
        self.finished = true;
        Ok(())
    }
}

fn append(target: &mut String, value: &Value) {
    if let Some(text) = value.as_str() {
        target.push_str(text);
    }
}
fn text(events: &mut Vec<ProviderEvent>, value: &Value, reasoning: bool) {
    if let Some(text) = value.as_str().filter(|text| !text.is_empty()) {
        events.push(if reasoning {
            ProviderEvent::Reasoning(text.into())
        } else {
            ProviderEvent::Text(text.into())
        });
    }
}

fn openai_messages(messages: &[Message]) -> Vec<Value> {
    let mut result = Vec::new();
    for message in messages {
        let content = message
            .parts
            .iter()
            .filter_map(|p| {
                if let Part::Text { text } = p {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect::<String>();
        let tools: Vec<_> = message.parts.iter().filter_map(|p| if let Part::Tool { call_id, name, input, .. } = p { Some(json!({"id": call_id, "type": "function", "function": {"name": name, "arguments": input.to_string()}})) } else { None }).collect();
        let mut value = json!({"role": message.role, "content": content});
        if !tools.is_empty() {
            value["tool_calls"] = json!(tools);
        }
        result.push(value);
        for part in &message.parts {
            if let Part::Tool {
                call_id, output, ..
            } = part
            {
                result.push(
                    json!({"role": "tool", "tool_call_id": call_id, "content": output.to_string()}),
                );
            }
        }
    }
    result
}

fn responses_messages(messages: &[Message]) -> Vec<Value> {
    let mut result = Vec::new();
    for message in messages.iter().filter(|m| m.role != Role::System) {
        for part in &message.parts {
            match part {
                Part::Text { text } => result.push(json!({"role":message.role,"content":[{"type":if message.role == Role::Assistant { "output_text" } else { "input_text" },"text":text}]})),
                Part::Tool { call_id, name, input, output, .. } => {
                    result.push(json!({"type":"function_call","call_id":call_id,"name":name,"arguments":input.to_string()}));
                    result.push(json!({"type":"function_call_output","call_id":call_id,"output":output.to_string()}));
                }
                Part::Reasoning { .. } => {}
            }
        }
    }
    result
}

fn anthropic_messages(messages: &[Message]) -> Vec<Value> {
    let mut result = Vec::new();
    for message in messages.iter().filter(|m| m.role != Role::System) {
        let mut content = Vec::new();
        let mut outputs = Vec::new();
        for part in &message.parts {
            match part {
                Part::Text { text } => content.push(json!({"type": "text", "text": text})),
                Part::Tool {
                    call_id,
                    name,
                    input,
                    output,
                    ..
                } => {
                    content.push(
                        json!({"type": "tool_use", "id": call_id, "name": name, "input": input}),
                    );
                    outputs.push(json!({"type": "tool_result", "tool_use_id": call_id, "content": output.to_string()}));
                }
                Part::Reasoning { .. } => {}
            }
        }
        if !content.is_empty() {
            result.push(json!({"role": message.role, "content": content}));
        }
        if !outputs.is_empty() {
            result.push(json!({"role": "user", "content": outputs}));
        }
    }
    result
}

fn google_messages(messages: &[Message]) -> Vec<Value> {
    let mut result = Vec::new();
    for message in messages.iter().filter(|m| m.role != Role::System) {
        let mut parts = Vec::new();
        let mut outputs = Vec::new();
        for part in &message.parts {
            match part {
                Part::Text { text } => parts.push(json!({"text": text})),
                Part::Tool {
                    name,
                    input,
                    output,
                    ..
                } => {
                    parts.push(json!({"functionCall": {"name": name, "args": input}}));
                    outputs.push(
                        json!({"functionResponse": {"name": name, "response": {"result": output}}}),
                    );
                }
                Part::Reasoning { .. } => {}
            }
        }
        if !parts.is_empty() {
            result.push(json!({"role": if message.role == Role::Assistant { "model" } else { "user" }, "parts": parts}));
        }
        if !outputs.is_empty() {
            result.push(json!({"role": "user", "parts": outputs}));
        }
    }
    result
}

fn tool_definitions() -> Vec<Value> {
    [
        ("meeting", "Read a meeting and its notes.", vec!["session_id"]),
        ("transcript", "Read a meeting transcript.", vec!["session_id"]),
        ("history", "Read meeting chat history.", vec!["session_id"]),
        ("search", "Find meetings in this library.", vec!["query"]),
        ("edit_memo", "Propose a memo edit; requires approval.", vec!["session_id", "expected", "body"]),
        ("edit_summary", "Propose a summary edit; requires approval.", vec!["document_id", "expected", "body"]),
        ("correct_transcript", "Propose a word correction; requires approval.", vec!["transcript_id", "word_id", "expected", "text"]),
    ].into_iter().map(|(name, description, required)| {
        let properties: serde_json::Map<String, Value> = required.iter().map(|key| ((*key).into(), json!({"type": "string"}))).collect();
        json!({"name": name, "description": description, "parameters": {"type": "object", "properties": properties, "required": required, "additionalProperties": false}})
    }).collect()
}

pub fn default_base(provider: &str) -> Option<String> {
    Some(
        match provider {
            "anthropic" | "claude" => "https://api.anthropic.com/v1",
            "openai" => "https://api.openai.com/v1",
            "openrouter" => "https://openrouter.ai/api/v1",
            "google_generative_ai" => "https://generativelanguage.googleapis.com/v1beta",
            "ollama" => "http://127.0.0.1:11434/v1",
            "lmstudio" => "http://127.0.0.1:1234/v1",
            "unsloth" => "http://127.0.0.1:8888/v1",
            "venice" => "https://api.venice.ai/api/v1",
            "moonshot" => "https://api.moonshot.ai/v1",
            "kimi_code" => "https://api.kimi.com/coding/v1",
            "zai" => "https://api.z.ai/api/paas/v4",
            "deepseek" => "https://api.deepseek.com",
            "alibaba_cloud" => "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
            "siliconflow" => "https://api.siliconflow.com/v1",
            "cohere" => "https://api.cohere.ai/compatibility/v1",
            "groq" => "https://api.groq.com/openai/v1",
            "xai" | "grok" => "https://api.x.ai/v1",
            "together" => "https://api.together.xyz/v1",
            "fireworks" => "https://api.fireworks.ai/inference/v1",
            "cerebras" => "https://api.cerebras.ai/v1",
            "mistral" => "https://api.mistral.ai/v1",
            "meta" => "https://api.meta.ai/v1",
            "github_copilot" => "https://api.githubcopilot.com",
            "chatgpt" => "https://chatgpt.com/backend-api/codex",
            _ => return None,
        }
        .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragmented_unicode_and_tool_arguments_survive_stream_boundaries() {
        let mut decoder = Decoder::new(Protocol::OpenAi);
        let payload = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"회의\"}}]}\r\n\r\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call\",\"function\":{\"name\":\"meeting\",\"arguments\":\"{\\\"session_id\\\":\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"abc\\\"}\"}}]}}]}\n\n",
            "data: [DONE]\n\n"
        );
        let mut events = Vec::new();
        for byte in payload.bytes() {
            events.extend(decoder.push(&[byte]).unwrap());
        }
        assert!(decoder.finished);
        assert!(matches!(&events[0], ProviderEvent::Text(text) if text == "회의"));
        assert!(
            matches!(&events[1], ProviderEvent::Tool { input, .. } if input["session_id"] == "abc")
        );
    }

    #[test]
    fn responses_anthropic_and_google_preserve_tools_and_terminal_errors() {
        for (protocol, payload) in [
            (
                Protocol::Responses,
                concat!(
                    "data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"type\":\"function_call\",\"call_id\":\"call\",\"name\":\"meeting\",\"arguments\":\"\"}}\n\n",
                    "data: {\"type\":\"response.function_call_arguments.delta\",\"output_index\":0,\"delta\":\"{\\\"session_id\\\":\\\"abc\\\"}\"}\n\n",
                    "data: {\"type\":\"response.completed\"}\n\n"
                ),
            ),
            (
                Protocol::Anthropic,
                concat!(
                    "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"call\",\"name\":\"meeting\"}}\n\n",
                    "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"session_id\\\":\\\"abc\\\"}\"}}\n\n",
                    "data: {\"type\":\"message_stop\"}\n\n"
                ),
            ),
            (
                Protocol::Google,
                "data: {\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"name\":\"meeting\",\"args\":{\"session_id\":\"abc\"}}}]},\"finishReason\":\"STOP\"}]}\n\n",
            ),
        ] {
            let mut decoder = Decoder::new(protocol);
            let mut events = Vec::new();
            for bytes in payload.as_bytes().chunks(3) {
                events.extend(decoder.push(bytes).unwrap());
            }
            assert!(decoder.finished);
            assert!(
                matches!(&events[0], ProviderEvent::Tool { name, input, .. } if name == "meeting" && input["session_id"] == "abc")
            );
        }
        let mut decoder = Decoder::new(Protocol::Responses);
        assert!(
            decoder
                .push(b"data: {\"type\":\"response.incomplete\"}\n\n")
                .is_err()
        );
        let mut decoder = Decoder::new(Protocol::Google);
        assert!(
            decoder
                .push(b"data: {\"candidates\":[{\"finishReason\":\"MAX_TOKENS\"}]}\n\n")
                .is_err()
        );
    }

    #[test]
    fn tool_results_are_returned_with_provider_call_ids() {
        let messages = openai_messages(&[Message {
            id: "m".into(),
            role: Role::Assistant,
            parts: vec![Part::Tool {
                call_id: "call".into(),
                name: "meeting".into(),
                state: "output-available".into(),
                input: json!({"session_id": "abc"}),
                output: json!({"title": "회의"}),
            }],
        }]);
        assert_eq!(messages[0]["tool_calls"][0]["id"], "call");
        assert_eq!(messages[1]["tool_call_id"], "call");
        assert_eq!(messages[1]["role"], "tool");
    }
}
