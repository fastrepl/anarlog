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
    ai::{
        Message, Part, ProviderAdapter, ProviderEvent, ProviderResolver, Request, Role, Structured,
    },
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
                Protocol::Google => json!([{"functionDeclarations": tools.iter().map(google_schema).collect::<Vec<_>>()}]),
            };
        }
        if let Some(structured) = &request.structured {
            structured_output(protocol, &self.connection.provider, structured, &mut body);
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

/// Gemini accepts an OpenAPI subset: no `additionalProperties` or `default`, and `enum`
/// instead of `const`, as the AI SDK's Google provider converts them.
/// Shipping's AI SDK structured output: OpenRouter's JSON schema, plain JSON mode on other
/// chat-completions servers, the Responses text format, Anthropic's forced `json` tool and
/// Gemini's response schema.
fn structured_output(
    protocol: Protocol,
    provider: &str,
    structured: &Structured,
    body: &mut Value,
) {
    let schema = &structured.schema;
    let tokens = structured.max_tokens;
    match protocol {
        Protocol::OpenAi => {
            body["max_tokens"] = json!(tokens);
            body["response_format"] = if matches!(provider, "anarlog" | "openrouter") {
                json!({"type": "json_schema", "json_schema": {"schema": schema, "strict": true, "name": "response"}})
            } else {
                json!({"type": "json_object"})
            };
        }
        Protocol::Responses => {
            body["max_output_tokens"] = json!(tokens);
            body["text"] = json!({"format": {"type": "json_schema", "strict": true, "name": "response", "schema": schema}});
        }
        Protocol::Anthropic => {
            body["max_tokens"] = json!(tokens);
            body["tools"] = json!([{"name": "json", "description": "Respond with a JSON object.", "input_schema": schema}]);
            body["tool_choice"] = json!({"type": "any", "disable_parallel_tool_use": true});
        }
        Protocol::Google => {
            body["generationConfig"] = json!({
                "maxOutputTokens": tokens,
                "responseMimeType": "application/json",
                "responseSchema": google_schema(schema),
            });
        }
    }
}

fn google_schema(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .filter(|(key, _)| !matches!(key.as_str(), "additionalProperties" | "default"))
                .map(|(key, value)| match key.as_str() {
                    "const" => ("enum".to_owned(), json!([value])),
                    _ => (key.clone(), google_schema(value)),
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(google_schema).collect()),
        value => value.clone(),
    }
}

/// Shipping's chat tools (`chat/tools/*.ts`) with their descriptions and input schemas.
fn tool_definitions() -> Vec<Value> {
    let string = |description: &str| json!({"type": "string", "description": description});
    let integer = |min: u32, max: u32, description: &str| json!({"type": "integer", "minimum": min, "maximum": max, "description": description});
    let session = string(
        "Session ID of the note to edit. Omit to edit the current note (the one marked as current in context). Pass another note's Session ID only when the user asks to edit that note.",
    );
    let bound = |description: &str| json!({"type": "number", "description": description});
    let folder_path = json!({"type": "string", "description": "Exact destination folder path, or an empty string to remove folder assignment"});
    let tool = |name: &str, description: &str, properties: Value, required: &[&str]| {
        json!({"name": name, "description": description, "parameters": {
            "type": "object", "properties": properties, "required": required, "additionalProperties": false,
        }})
    };
    vec![
        tool(
            "list_folders",
            "List existing meeting folders, including empty folders and parent folders. Use their exact paths for folder operations. Query matches a case-insensitive path substring; follow next_offset for more results.",
            json!({"query": {"type": "string"}, "limit": {"type": "integer", "minimum": 1, "maximum": 200}, "offset": {"type": "integer", "minimum": 0}}),
            &[],
        ),
        tool(
            "create_folder",
            "Create a named meeting folder, including parent folders for a nested path. List folders first and reuse an existing destination when it matches the user's request. This does not move any meetings.",
            json!({"folder_path": {"type": "string"}}),
            &["folder_path"],
        ),
        tool(
            "move_meetings_to_folder",
            "Move up to 200 existing meetings into an existing folder, or use an empty folder_path to unfile them. This changes folder assignment only; it does not merge meetings or move contents between meetings. Resolve meeting IDs with list_meetings or get_recurring_meeting_history and use the exact destination from list_folders or create_folder. For all/every requests, follow meeting pagination and submit each batch. Results report moved, unchanged, and failed meetings separately; never claim failures succeeded.",
            json!({"meeting_ids": {"type": "array", "items": {"type": "string", "minLength": 1}, "minItems": 1, "maxItems": 200}, "folder_path": folder_path}),
            &["meeting_ids", "folder_path"],
        ),
        tool(
            "list_meetings",
            "List recent Anarlog meetings with pagination metadata. Use query to narrow by title or meeting id and folder_path to scope to a folder and its subfolders, then pass next_offset as offset to continue.",
            json!({
                "query": string("Case-insensitive title or meeting id substring"),
                "series_id": string("Exact recurring series id"),
                "folder_path": string("Folder path from list_folders, such as Projects/Launch; includes meetings in its subfolders"),
                "limit": integer(1, 200, "Maximum results; defaults to 20 and is capped at 200"),
                "offset": {"type": "integer", "minimum": 0, "description": "Number of results to skip; defaults to 0"},
            }),
            &[],
        ),
        tool(
            "get_meeting",
            "Get one Anarlog meeting with its canonical note, summaries, participants, and action items. Use get_meeting_transcript separately for transcript words.",
            json!({"meeting_id": string("Anarlog meeting id")}),
            &["meeting_id"],
        ),
        tool(
            "get_meeting_transcript",
            "Get a bounded page of transcript words and readable text for an Anarlog meeting. Pass pagination.next_offset as offset to continue.",
            json!({
                "meeting_id": string("Anarlog meeting id"),
                "offset": {"type": "integer", "minimum": 0, "description": "Word offset; defaults to 0"},
                "limit": integer(1, 500, "Maximum words; defaults to 200 and is capped at 500"),
            }),
            &["meeting_id"],
        ),
        tool(
            "get_recurring_meeting_history",
            "List meetings in the same recurring series as the supplied meeting, newest first, with pagination metadata.",
            json!({
                "meeting_id": string("A meeting id used to resolve its recurring series"),
                "limit": integer(1, 200, "Maximum meetings; defaults to 20 and is capped at 200"),
                "offset": {"type": "integer", "minimum": 0, "description": "Number of meetings to skip; defaults to 0"},
            }),
            &["meeting_id"],
        ),
        tool(
            "search_meeting_content",
            "Search local meeting notes and transcripts for exact words or phrases. Use search_meetings first for open-ended questions about past meetings, people, decisions, or topics. This is lexical content search, not vector search.",
            json!({
                "query": string("Text to search for in meeting content"),
                "meeting_ids": {"type": "array", "items": {"type": "string"}, "description": "Optional meeting ids to restrict the content search"},
                "limit": integer(1, 10, "Maximum matching notes to return"),
            }),
            &["query"],
        ),
        tool(
            "read_folder_material",
            "Read a folder material such as a syllabus, PDF, or text file attached to the current folder. Use the material IDs listed in folder context.",
            json!({
                "materialId": string("Folder material id from the folder context list"),
                "maxChars": integer(1_000, 30_000, "Maximum material characters to return to the model"),
            }),
            &["materialId"],
        ),
        tool(
            "find_related_meetings",
            "Find meetings related to the current or given meeting by shared participants, the same calendar event, or nearby dates.",
            json!({
                "meeting_id": string("Meeting id to find related meetings for. Defaults to the current note (the one marked as current in context)."),
                "limit": integer(1, 10, "Maximum related meetings to return"),
            }),
            &[],
        ),
        tool(
            "search_meetings",
            "Search for meetings using note and transcript content plus optional date filters.\nUse this first for open-ended questions about past meetings, people, decisions, or topics when the answer may be in meeting notes and no meeting note context is attached.\nUse filters.created_at.kind=\"relative\" with recent_days for natural-language date ranges.\nUse an empty query string when the user only wants meetings by date/time filter.\nReturns relevant meetings with matching content excerpts.",
            json!({
                "query": string("Optional text query for finding relevant meetings. Omit this when filtering only by date/time."),
                "filters": {"type": "object", "description": "Optional meeting filters", "additionalProperties": false, "properties": {
                    "created_at": {"description": "Date filter for meetings. Uses event started_at for event-backed meetings, otherwise meeting created_at.", "anyOf": [
                        {"type": "object", "description": "Absolute timestamp bounds using Unix milliseconds. Use this only when you already know the exact timestamps.", "additionalProperties": false, "required": ["kind"], "properties": {
                            "kind": {"type": "string", "const": "absolute"},
                            "gte": bound("Include meetings on or after this Unix timestamp in milliseconds"),
                            "lte": bound("Include meetings on or before this Unix timestamp in milliseconds"),
                            "gt": bound("Include meetings after this Unix timestamp in milliseconds"),
                            "lt": bound("Include meetings before this Unix timestamp in milliseconds"),
                            "eq": bound("Include only meetings at this exact Unix timestamp in milliseconds"),
                        }},
                        {"type": "object", "description": "Relative date filter. Prefer this over absolute timestamps for natural-language time ranges.", "additionalProperties": false, "required": ["kind", "recent_days"], "properties": {
                            "kind": {"type": "string", "const": "relative"},
                            "recent_days": integer(1, 365, "Use for requests like 'last N days'. Includes today and counts backward in local time."),
                        }},
                    ]},
                }},
                "limit": integer(0, 10, "Maximum number of meetings to return"),
            }),
            &[],
        ),
        tool(
            "search_contacts",
            "Search contacts and return names, emails, phone numbers, roles, and organizations.",
            json!({"query": string("Search query for contacts"), "limit": integer(1, 20, "Maximum number of contacts to return")}),
            &["query"],
        ),
        tool(
            "search_calendar_events",
            "Search calendar events and return schedule, location, and linked session info.",
            json!({"query": string("Search query for calendar events"), "limit": integer(1, 20, "Maximum number of events to return")}),
            &["query"],
        ),
        tool(
            "web_search",
            "Search the public web for current or external information.\nUse this for questions about public websites, URLs, companies, products, people, news, or facts that may not be in local notes.\nReturn source URLs in the final answer when web results are used.\nDo not use this when the user is asking only about local notes, meetings, contacts, or calendar events.",
            json!({
                "query": {"type": "string", "minLength": 1, "description": "Search query for public web information."},
                "includeDomains": {"type": "array", "items": {"type": "string", "minLength": 1}, "maxItems": 5, "description": "Optional domains to search within, for example ['char.com']. Use only when the user names a specific site or domain."},
                "excludeDomains": {"type": "array", "items": {"type": "string", "minLength": 1}, "maxItems": 5, "description": "Optional domains to exclude from the search results."},
                "limit": integer(1, 10, "Maximum number of web results to return."),
            }),
            &["query"],
        ),
        tool(
            "edit_memo",
            "Propose a complete replacement for a session memo and open a diff review where the user can apply or cancel it. Use this to create or revise meeting preparation, agendas, talking points, and notes before or during a meeting. Preserve relevant existing memo content in the replacement markdown.",
            json!({"sessionId": session, "content": string("The complete replacement memo in markdown format")}),
            &["content"],
        ),
        tool(
            "edit_summary",
            "Propose a complete replacement for an existing session summary and open a diff review where the user can apply or cancel it. Use this for broad rewrites such as refocusing, shortening, or restructuring a summary. The content must be the full replacement markdown, not instructions or a partial patch.",
            json!({
                "sessionId": session,
                "enhancedNoteId": string("The specific summary ID (enhanced note ID) to edit. Defaults to the active summary in the session tab when possible."),
                "content": string("The complete replacement summary in markdown format"),
            }),
            &["content"],
        ),
        tool(
            "apply_session_correction",
            "Apply a correction to a session summary, visible session title, and/or transcript. Use this when the user corrects note content, for example 'it's not X but Y'. Prefer summary_and_transcript for factual meeting corrections unless the user explicitly asks for one target only. Read the note first if you need exact summary text.",
            json!({
                "sessionId": session,
                "target": {"type": "string", "enum": ["summary", "transcript", "summary_and_transcript"], "default": "summary_and_transcript", "description": "Which session content to correct. Use summary only or transcript only when the user explicitly scopes the correction."},
                "enhancedNoteId": string("Optional summary ID to restrict summary edits. Defaults to summaries in the target session."),
                "oldText": {"type": "string", "minLength": 1, "description": "Exact text currently present in the note or transcript."},
                "newText": string("Replacement text."),
                "dictionaryTerms": {"type": "array", "items": {"type": "string", "minLength": 1}, "default": [], "description": "Uncommon names, company/product names, acronyms, or jargon from the correction to save for future transcription. Skip common names."},
            }),
            &["oldText", "newText"],
        ),
        tool(
            "move_meeting_contents",
            "Move a finished recording, transcript, generated summaries, notes, and action items from one meeting onto another existing meeting. Use this when the user says a recording or notes landed on the wrong meeting. Resolve both meeting IDs with list_meetings or search_meetings first and never guess IDs. The target meeting must not already have a recording or transcript.",
            json!({
                "sourceMeetingId": string("Meeting that currently has the recording or notes. Defaults to the current note (the one marked as current in context)."),
                "targetMeetingId": string("Existing meeting that should receive the recording and notes."),
            }),
            &["targetMeetingId"],
        ),
    ]
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
                name: "get_meeting".into(),
                state: "output-available".into(),
                input: json!({"meeting_id": "abc"}),
                output: json!({"title": "회의"}),
                error: None,
            }],
        }]);
        assert_eq!(messages[0]["tool_calls"][0]["id"], "call");
        assert_eq!(messages[1]["tool_call_id"], "call");
        assert_eq!(messages[1]["role"], "tool");
    }
}
