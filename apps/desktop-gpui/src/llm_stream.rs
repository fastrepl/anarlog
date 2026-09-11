//! Streaming chat generation against the configured language model: the
//! `streamText` call of the AI tasks, over the provider shapes
//! `useLLMConnection`'s `createProviderModel` picks (OpenAI-compatible chat
//! completions, the OpenAI / Azure Responses API, Anthropic messages, Gemini
//! `streamGenerateContent`), with
//! `extractReasoningMiddleware`'s `<think>` / `<thinking>` handling and
//! `reasoningProviderOptions`.

use std::time::Duration;

use futures_util::StreamExt as _;
use serde_json::{Value, json};
use tokio::sync::mpsc;

/// `maxRetries: 4` on `streamText`.
const MAX_RETRIES: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chunk {
    TextDelta(String),
    ReasoningDelta(String),
    /// A complete tool call the model asked for; arrives before `Done`.
    ToolCall(ToolCall),
    /// A finished Responses output item the SDK records as provider
    /// metadata (`itemId`) on the message's text or reasoning part, so the
    /// next turn can refer back to it.
    Item(StoredItem),
    Done,
    Error(String),
}

/// A Responses API output item stored server-side (`store: true`), referred
/// to from the next request as `{ "type": "item_reference", "id" }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredItem {
    /// The assistant `message` item that carried the reply text.
    Message { id: String },
    /// A `reasoning` item; `encrypted_content` is only present when asked for.
    Reasoning {
        id: String,
        encrypted_content: Option<String>,
    },
}

impl StoredItem {
    pub fn id(&self) -> &str {
        match self {
            StoredItem::Message { id } | StoredItem::Reasoning { id, .. } => id,
        }
    }
}

/// A tool the model may call (`tools[].function` in the OpenAI shape).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// A tool call the model made, with its arguments parsed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
    /// The Responses API's `function_call` item id: a follow-up in the same
    /// conversation refers back to the stored item (`item_reference`, the
    /// SDK's `store: true` default) instead of resending the call.
    pub item_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    pub provider_id: String,
    pub base_url: String,
    pub api_key: String,
    pub model_id: String,
    /// `default` / `low` / `medium` / `high`.
    pub reasoning_effort: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub system: String,
    /// The conversation after the system prompt, oldest first; the last
    /// entry is the user message or tool result the model answers.
    pub messages: Vec<Turn>,
    /// `0` leaves the provider's default in place (the chat sets no cap).
    pub max_output_tokens: u32,
    /// The tools the model may call; empty leaves the request without any.
    pub tools: Vec<ToolSpec>,
    /// `Output.object`'s JSON Schema: the reply must be one object matching
    /// it, requested in each provider's structured-output shape.
    pub json_schema: Option<Value>,
}

impl Request {
    /// A single-turn request: the system prompt and one user message.
    pub fn new(
        system: impl Into<String>,
        prompt: impl Into<String>,
        max_output_tokens: u32,
    ) -> Self {
        Self {
            system: system.into(),
            messages: vec![Turn::User(prompt.into())],
            max_output_tokens,
            tools: Vec::new(),
            json_schema: None,
        }
    }
}

/// One message of a multi-turn chat (`ModelMessage` minus the system one).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Turn {
    User(String),
    /// A user message of several text parts (the chat's context block ahead
    /// of the typed message), each its own part in every provider's shape.
    UserParts(Vec<String>),
    /// A user message with `ImagePart`s after its text (`createPromptInput`
    /// with image context).
    UserWithImages {
        text: String,
        images: Vec<ImagePart>,
    },
    Assistant {
        text: String,
        tool_calls: Vec<ToolCall>,
    },
    /// A tool's output for one call, as the JSON text the model reads;
    /// `is_error` for a tool that failed (`error-text`), which Anthropic's
    /// `tool_result` flags.
    ToolResult {
        call_id: String,
        name: String,
        output: String,
        is_error: bool,
    },
    /// A Responses output item of the assistant turn that follows, referred
    /// to by id like `convertToOpenAIResponsesInput` does for parts carrying
    /// an `itemId` (a `Message` item stands in for that turn's text). Other
    /// providers have no stored items and skip it.
    StoredItem(StoredItem),
}

impl Turn {
    fn is_stored_item(&self) -> bool {
        matches!(self, Turn::StoredItem(_))
    }
}

/// An `ImagePart`: base64 data with its media type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePart {
    pub base64: String,
    pub mime_type: String,
}

impl Request {
    /// `Request::new` with images attached to the user message.
    pub fn with_images(
        system: impl Into<String>,
        prompt: impl Into<String>,
        images: Vec<ImagePart>,
        max_output_tokens: u32,
    ) -> Self {
        let text = prompt.into();
        Self {
            system: system.into(),
            messages: vec![if images.is_empty() {
                Turn::User(text)
            } else {
                Turn::UserWithImages { text, images }
            }],
            max_output_tokens,
            tools: Vec::new(),
            json_schema: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    OpenAiCompatible,
    /// `@ai-sdk/openai` / `@ai-sdk/azure`'s default model: the Responses API.
    OpenAiResponses,
    Anthropic,
    Google,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub url: String,
    pub headers: Vec<(&'static str, String)>,
    pub body: Value,
    family: Family,
}

/// `isLocalModelProviderId`: on-device servers get the long start timeout.
pub fn is_local_model_provider(provider_id: &str) -> bool {
    matches!(
        provider_id,
        "apple_foundation" | "lmstudio" | "ollama" | "unsloth"
    )
}

/// `reasoningProviderOptions`, already in each provider's wire format.
fn reasoning_options(conn: &Connection) -> Option<Value> {
    let effort = conn.reasoning_effort.as_str();
    if effort == "default" || !crate::ai_models::supports_reasoning_effort(&conn.provider_id) {
        return None;
    }
    Some(match conn.provider_id.as_str() {
        "openai" | "chatgpt" | "azure_openai" => json!({ "reasoning_effort": effort }),
        "anthropic" | "claude" => json!({
            "thinking": { "type": "adaptive" },
            "output_config": { "effort": effort }
        }),
        "openrouter" => json!({ "reasoning": { "effort": effort } }),
        "google_generative_ai" => {
            let version = regex::Regex::new(r"gemini-(\d+)(?:\.(\d+))?").ok()?;
            let captures = version.captures(&conn.model_id)?;
            let major: u32 = captures.get(1)?.as_str().parse().ok()?;
            let minor: u32 = captures
                .get(2)
                .map(|m| m.as_str().parse().unwrap_or(0))
                .unwrap_or(0);
            if major >= 3 {
                json!({ "thinkingConfig": { "thinkingLevel": effort } })
            } else if major == 2 && minor == 5 {
                let budget = match effort {
                    "low" => 1024,
                    "medium" => 8192,
                    _ => 24576,
                };
                json!({ "thinkingConfig": { "thinkingBudget": budget } })
            } else {
                return None;
            }
        }
        _ => json!({ "reasoning_effort": effort }),
    })
}

/// `getOpenAILanguageModelCapabilities(modelId).isReasoningModel`: the
/// models that take `reasoning.effort` and a `developer` system message.
pub fn openai_is_reasoning_model(model: &str) -> bool {
    model.starts_with("o1")
        || model.starts_with("o3")
        || model.starts_with("o4-mini")
        || (model.starts_with("gpt-5") && !model.starts_with("gpt-5-chat"))
}

/// `convertToOpenAIResponsesInput` for the turns the tasks send: the system
/// prompt as a `system` (or `developer`) item, user text as `input_text`
/// with `input_image` data URLs, assistant text as an `output_text`
/// message, tool calls as `function_call` items with their arguments as
/// JSON text, and tool results as `function_call_output`.
fn openai_responses_input(request: &Request, reasoning_model: bool) -> Vec<Value> {
    let mut input = vec![json!({
        "role": if reasoning_model { "developer" } else { "system" },
        "content": request.system
    })];
    let mut text_referenced = false;
    for turn in &request.messages {
        match turn {
            Turn::StoredItem(item) => {
                input.push(json!({ "type": "item_reference", "id": item.id() }));
                if matches!(item, StoredItem::Message { .. }) {
                    text_referenced = true;
                }
            }
            Turn::User(text) => input.push(json!({
                "role": "user",
                "content": [{ "type": "input_text", "text": text }]
            })),
            Turn::UserParts(texts) => input.push(json!({
                "role": "user",
                "content": texts
                    .iter()
                    .map(|text| json!({ "type": "input_text", "text": text }))
                    .collect::<Vec<_>>()
            })),
            Turn::UserWithImages { text, images } => {
                let mut content = vec![json!({ "type": "input_text", "text": text })];
                content.extend(images.iter().map(|image| {
                    json!({
                        "type": "input_image",
                        "image_url": format!("data:{};base64,{}", image.mime_type, image.base64)
                    })
                }));
                input.push(json!({ "role": "user", "content": content }));
            }
            Turn::Assistant { text, tool_calls } => {
                if !text.is_empty() && !std::mem::take(&mut text_referenced) {
                    input.push(json!({
                        "role": "assistant",
                        "content": [{ "type": "output_text", "text": text }]
                    }));
                }
                for call in tool_calls {
                    input.push(match &call.item_id {
                        Some(item_id) => json!({ "type": "item_reference", "id": item_id }),
                        None => json!({
                            "type": "function_call",
                            "call_id": call.id,
                            "name": call.name,
                            "arguments": call.arguments.to_string()
                        }),
                    });
                }
            }
            Turn::ToolResult {
                call_id, output, ..
            } => input.push(json!({
                "type": "function_call_output",
                "call_id": call_id,
                "output": output
            })),
        }
    }
    input
}

/// `OpenAIResponsesLanguageModel.getArgs`: `model`, `input`,
/// `max_output_tokens`, `text.format` for a schema, `reasoning.effort` on a
/// reasoning model, then `tools` / `tool_choice` and `stream`.
fn openai_responses_body(conn: &Connection, request: &Request, stream: bool) -> Value {
    let reasoning_model = openai_is_reasoning_model(&conn.model_id);
    let mut body = json!({
        "model": conn.model_id,
        "input": openai_responses_input(request, reasoning_model)
    });
    if request.max_output_tokens > 0 {
        merge(
            &mut body,
            Some(json!({ "max_output_tokens": request.max_output_tokens })),
        );
    }
    if let Some(schema) = &request.json_schema {
        merge(
            &mut body,
            Some(json!({
                "text": {
                    "format": {
                        "type": "json_schema",
                        "strict": true,
                        "name": "response",
                        "schema": schema
                    }
                }
            })),
        );
    }
    // The SDK drops `reasoningEffort` for non-reasoning models with a warning.
    if reasoning_model
        && conn.reasoning_effort != "default"
        && crate::ai_models::supports_reasoning_effort(&conn.provider_id)
    {
        merge(
            &mut body,
            Some(json!({ "reasoning": { "effort": conn.reasoning_effort } })),
        );
    }
    if !request.tools.is_empty() {
        let tools: Vec<Value> = request
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters
                })
            })
            .collect();
        merge(
            &mut body,
            Some(json!({ "tools": tools, "tool_choice": "auto" })),
        );
    }
    if stream {
        merge(&mut body, Some(json!({ "stream": true })));
    }
    body
}

fn merge(target: &mut Value, extra: Option<Value>) {
    if let (Some(target), Some(Value::Object(extra))) = (target.as_object_mut(), extra) {
        for (key, value) in extra {
            target.insert(key, value);
        }
    }
}

/// `getModelCapabilities().supportsStructuredOutput` of `@ai-sdk/anthropic`:
/// the models that take `output_config.format`; older ones get the `json`
/// tool instead.
fn anthropic_supports_structured_output(model: &str) -> bool {
    [
        "claude-sonnet-4-6",
        "claude-opus-4-6",
        "claude-sonnet-4-5",
        "claude-opus-4-5",
        "claude-haiku-4-5",
        "claude-opus-4-1",
    ]
    .iter()
    .any(|family| model.contains(family))
}

/// `convertJSONSchemaToOpenAPISchema` of `@ai-sdk/google`, for the schema
/// features the AI tasks use: Gemini's `responseSchema` drops `$schema`,
/// `additionalProperties`, and array bounds.
fn openapi_schema(schema: &Value) -> Value {
    let Some(object) = schema.as_object() else {
        return schema.clone();
    };
    let mut out = serde_json::Map::new();
    for key in ["description", "required", "format"] {
        if let Some(value) = object.get(key) {
            out.insert(key.to_string(), value.clone());
        }
    }
    if let Some(constant) = object.get("const") {
        out.insert("enum".to_string(), json!([constant]));
    }
    if let Some(kind) = object.get("type") {
        match kind.as_array() {
            Some(kinds) if kinds.iter().any(|k| k == "null") => {
                if let Some(first) = kinds.iter().find(|k| *k != "null") {
                    out.insert("type".to_string(), first.clone());
                }
                out.insert("nullable".to_string(), Value::Bool(true));
            }
            _ => {
                out.insert("type".to_string(), kind.clone());
            }
        }
    }
    if let Some(values) = object.get("enum") {
        out.insert("enum".to_string(), values.clone());
    }
    if let Some(properties) = object.get("properties").and_then(Value::as_object) {
        out.insert(
            "properties".to_string(),
            Value::Object(
                properties
                    .iter()
                    .map(|(key, value)| (key.clone(), openapi_schema(value)))
                    .collect(),
            ),
        );
    }
    if let Some(items) = object.get("items") {
        out.insert(
            "items".to_string(),
            match items.as_array() {
                Some(items) => Value::Array(items.iter().map(openapi_schema).collect()),
                None => openapi_schema(items),
            },
        );
    }
    for key in ["allOf", "anyOf", "oneOf"] {
        if let Some(variants) = object.get(key).and_then(Value::as_array) {
            out.insert(
                key.to_string(),
                Value::Array(variants.iter().map(openapi_schema).collect()),
            );
        }
    }
    if let Some(min_length) = object.get("minLength") {
        out.insert("minLength".to_string(), min_length.clone());
    }
    Value::Object(out)
}

/// The HTTP request for one streaming generation attempt.
pub fn build_request(conn: &Connection, request: &Request) -> Result<HttpRequest, String> {
    build(conn, request, true)
}

/// The HTTP request for one `generateText` attempt: the reply arrives whole.
pub fn build_generate_request(conn: &Connection, request: &Request) -> Result<HttpRequest, String> {
    build(conn, request, false)
}

fn build(conn: &Connection, request: &Request, stream: bool) -> Result<HttpRequest, String> {
    let base = conn.base_url.trim_end_matches('/');
    if base.is_empty() {
        return Err("The language model provider has no base URL.".to_string());
    }
    let api_key = conn.api_key.as_str();
    let model = conn.model_id.as_str();
    let mut openai_messages = vec![json!({ "role": "system", "content": request.system })];
    openai_messages.extend(
        request
            .messages
            .iter()
            .filter(|turn| !turn.is_stored_item())
            .map(openai_message),
    );
    // Keys in the order the SDK's `args` spread produces them.
    let mut openai_body = json!({ "model": model });
    if request.max_output_tokens > 0 {
        merge(
            &mut openai_body,
            Some(json!({ "max_tokens": request.max_output_tokens })),
        );
    }
    if request.json_schema.is_some() {
        // `@ai-sdk/openai-compatible` without `supportsStructuredOutputs`
        // asks for any JSON object.
        merge(
            &mut openai_body,
            Some(json!({ "response_format": { "type": "json_object" } })),
        );
    }
    merge(
        &mut openai_body,
        Some(json!({ "messages": openai_messages })),
    );
    if !request.tools.is_empty() {
        let tools: Vec<Value> = request
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.parameters
                    }
                })
            })
            .collect();
        merge(
            &mut openai_body,
            Some(json!({ "tools": tools, "tool_choice": "auto" })),
        );
    }
    if stream {
        merge(&mut openai_body, Some(json!({ "stream": true })));
    }
    Ok(match conn.provider_id.as_str() {
        "anarlog" | "claude" | "chatgpt" | "grok" | "github_copilot" | "apple_foundation" => {
            return Err(format!(
                "The {} provider needs the account flows, which the native shell does not ship yet.",
                conn.provider_id
            ));
        }
        "anthropic" => {
            let mut body = json!({
                "model": model,
                // Anthropic requires the cap; the SDK's default for chat.
                "max_tokens": if request.max_output_tokens > 0 { request.max_output_tokens } else { 4096 },
            });
            // `thinking` / `output_config` precede the prompt in `baseArgs`.
            merge(&mut body, reasoning_options(conn));
            if let Some(schema) = &request.json_schema
                && anthropic_supports_structured_output(model)
            {
                let mut output_config = body
                    .as_object_mut()
                    .and_then(|body| body.remove("output_config"))
                    .unwrap_or_else(|| json!({}));
                merge(
                    &mut output_config,
                    Some(json!({ "format": { "type": "json_schema", "schema": schema } })),
                );
                merge(&mut body, Some(json!({ "output_config": output_config })));
            }
            merge(
                &mut body,
                Some(json!({
                    // The system prompt is a text block too.
                    "system": [{ "type": "text", "text": request.system }],
                    "messages": request.messages.iter().filter(|turn| !turn.is_stored_item()).map(anthropic_message).collect::<Vec<_>>()
                })),
            );
            if !request.tools.is_empty() {
                let tools: Vec<Value> = request
                    .tools
                    .iter()
                    .map(|tool| {
                        json!({
                            "name": tool.name,
                            "description": tool.description,
                            "input_schema": tool.parameters
                        })
                    })
                    .collect();
                merge(&mut body, Some(json!({ "tools": tools })));
            }
            if let Some(schema) = &request.json_schema
                && !anthropic_supports_structured_output(model)
            {
                // `jsonResponseTool`: the object arrives as a tool call.
                merge(
                    &mut body,
                    Some(json!({
                        "tools": [{
                            "name": "json",
                            "description": "Respond with a JSON object.",
                            "input_schema": schema
                        }],
                        "tool_choice": { "type": "any", "disable_parallel_tool_use": true }
                    })),
                );
            }
            if stream {
                merge(&mut body, Some(json!({ "stream": true })));
            }
            HttpRequest {
                url: format!("{base}/messages"),
                headers: vec![
                    ("x-api-key", api_key.to_string()),
                    ("anthropic-version", "2023-06-01".to_string()),
                    (
                        "anthropic-dangerous-direct-browser-access",
                        "true".to_string(),
                    ),
                ],
                body,
                family: Family::Anthropic,
            }
        }
        "google_generative_ai" => {
            let mut generation_config = json!({});
            if request.max_output_tokens > 0 {
                generation_config = json!({ "maxOutputTokens": request.max_output_tokens });
            }
            if let Some(schema) = &request.json_schema {
                merge(
                    &mut generation_config,
                    Some(json!({
                        "responseMimeType": "application/json",
                        "responseSchema": openapi_schema(schema)
                    })),
                );
            }
            merge(&mut generation_config, reasoning_options(conn));
            let contents: Vec<Value> = request
                .messages
                .iter()
                .filter(|turn| !turn.is_stored_item())
                .map(google_content)
                .collect();
            let mut body = json!({
                "generationConfig": generation_config,
                "contents": contents,
                "systemInstruction": { "parts": [{ "text": request.system }] }
            });
            if !request.tools.is_empty() {
                let declarations: Vec<Value> = request
                    .tools
                    .iter()
                    .map(|tool| {
                        json!({
                            "name": tool.name,
                            "description": tool.description,
                            "parameters": tool.parameters
                        })
                    })
                    .collect();
                merge(
                    &mut body,
                    Some(json!({ "tools": [{ "functionDeclarations": declarations }] })),
                );
            }
            HttpRequest {
                url: if stream {
                    format!("{base}/models/{model}:streamGenerateContent?alt=sse")
                } else {
                    format!("{base}/models/{model}:generateContent")
                },
                headers: vec![("x-goog-api-key", api_key.to_string())],
                body,
                family: Family::Google,
            }
        }
        // `createOpenAI(...)(modelId)` is the Responses model.
        "openai" => HttpRequest {
            url: format!("{base}/responses"),
            headers: vec![("Authorization", format!("Bearer {api_key}"))],
            body: openai_responses_body(conn, request, stream),
            family: Family::OpenAiResponses,
        },
        // `createAzure(...)(deployment)`: `{baseURL}/v1/responses?api-version=v1`
        // with the `api-key` header.
        "azure_openai" => HttpRequest {
            url: format!("{base}/v1/responses?api-version=v1"),
            headers: vec![("api-key", api_key.to_string())],
            body: openai_responses_body(conn, request, stream),
            family: Family::OpenAiResponses,
        },
        "azure_ai" => {
            merge(&mut openai_body, reasoning_options(conn));
            HttpRequest {
                url: format!("{base}/chat/completions"),
                headers: vec![
                    ("api-key", api_key.to_string()),
                    ("Authorization", format!("Bearer {api_key}")),
                ],
                body: openai_body,
                family: Family::OpenAiCompatible,
            }
        }
        provider => {
            merge(&mut openai_body, reasoning_options(conn));
            HttpRequest {
                url: format!("{base}/chat/completions"),
                headers: if api_key.is_empty() || provider == "ollama" {
                    Vec::new()
                } else {
                    vec![("Authorization", format!("Bearer {api_key}"))]
                },
                body: openai_body,
                family: Family::OpenAiCompatible,
            }
        }
    })
}

/// `convertToOpenAIChatMessages`: assistant tool calls carry their
/// arguments as JSON text, tool results answer by `tool_call_id`.
fn openai_message(turn: &Turn) -> Value {
    match turn {
        // Filtered out before mapping: only the Responses API has stored items.
        Turn::StoredItem(_) => Value::Null,
        Turn::User(text) => json!({ "role": "user", "content": text }),
        Turn::UserParts(texts) => json!({
            "role": "user",
            "content": texts
                .iter()
                .map(|text| json!({ "type": "text", "text": text }))
                .collect::<Vec<_>>()
        }),
        // `image_url` parts carry a data URL.
        Turn::UserWithImages { text, images } => {
            let mut content = vec![json!({ "type": "text", "text": text })];
            content.extend(images.iter().map(|image| {
                json!({
                    "type": "image_url",
                    "image_url": { "url": format!("data:{};base64,{}", image.mime_type, image.base64) }
                })
            }));
            json!({ "role": "user", "content": content })
        }
        Turn::Assistant { text, tool_calls } => {
            let mut message = json!({ "role": "assistant", "content": text });
            if !tool_calls.is_empty() {
                let calls: Vec<Value> = tool_calls
                    .iter()
                    .map(|call| {
                        json!({
                            "id": call.id,
                            "type": "function",
                            "function": {
                                "name": call.name,
                                "arguments": call.arguments.to_string()
                            }
                        })
                    })
                    .collect();
                merge(&mut message, Some(json!({ "tool_calls": calls })));
            }
            message
        }
        Turn::ToolResult {
            call_id, output, ..
        } => json!({ "role": "tool", "tool_call_id": call_id, "content": output }),
    }
}

fn anthropic_message(turn: &Turn) -> Value {
    match turn {
        // Filtered out before mapping: only the Responses API has stored items.
        Turn::StoredItem(_) => Value::Null,
        // `convertToAnthropicMessagesPrompt`: user content is always blocks.
        Turn::User(text) => json!({
            "role": "user",
            "content": [{ "type": "text", "text": text }]
        }),
        Turn::UserParts(texts) => json!({
            "role": "user",
            "content": texts
                .iter()
                .map(|text| json!({ "type": "text", "text": text }))
                .collect::<Vec<_>>()
        }),
        Turn::UserWithImages { text, images } => {
            let mut content = vec![json!({ "type": "text", "text": text })];
            content.extend(images.iter().map(|image| {
                json!({
                    "type": "image",
                    "source": { "type": "base64", "media_type": image.mime_type, "data": image.base64 }
                })
            }));
            json!({ "role": "user", "content": content })
        }
        Turn::Assistant { text, tool_calls } => {
            let mut content: Vec<Value> = Vec::new();
            if !text.is_empty() {
                content.push(json!({ "type": "text", "text": text }));
            }
            for call in tool_calls {
                content.push(json!({
                    "type": "tool_use",
                    "id": call.id,
                    "name": call.name,
                    "input": call.arguments
                }));
            }
            json!({ "role": "assistant", "content": content })
        }
        Turn::ToolResult {
            call_id,
            output,
            is_error,
            ..
        } => {
            let mut result =
                json!({ "type": "tool_result", "tool_use_id": call_id, "content": output });
            if *is_error {
                merge(&mut result, Some(json!({ "is_error": true })));
            }
            json!({ "role": "user", "content": [result] })
        }
    }
}

fn google_content(turn: &Turn) -> Value {
    match turn {
        // Filtered out before mapping: only the Responses API has stored items.
        Turn::StoredItem(_) => Value::Null,
        Turn::User(text) => json!({ "role": "user", "parts": [{ "text": text }] }),
        Turn::UserParts(texts) => json!({
            "role": "user",
            "parts": texts.iter().map(|text| json!({ "text": text })).collect::<Vec<_>>()
        }),
        Turn::UserWithImages { text, images } => {
            let mut parts = vec![json!({ "text": text })];
            parts.extend(images.iter().map(|image| {
                json!({ "inlineData": { "mimeType": image.mime_type, "data": image.base64 } })
            }));
            json!({ "role": "user", "parts": parts })
        }
        Turn::Assistant { text, tool_calls } => {
            let mut parts: Vec<Value> = Vec::new();
            if !text.is_empty() {
                parts.push(json!({ "text": text }));
            }
            for call in tool_calls {
                parts
                    .push(json!({ "functionCall": { "name": call.name, "args": call.arguments } }));
            }
            json!({ "role": "model", "parts": parts })
        }
        Turn::ToolResult { name, output, .. } => {
            let response = serde_json::from_str::<Value>(output)
                .ok()
                .filter(|value| value.is_object())
                .unwrap_or_else(|| json!({ "result": output }));
            json!({
                "role": "user",
                "parts": [{ "functionResponse": { "name": name, "response": response } }]
            })
        }
    }
}

/// What one SSE event contributes before tool calls are assembled.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Text(String),
    Reasoning(String),
    /// A fragment of a streamed tool call: OpenAI's `tool_calls[index]`
    /// deltas or Anthropic's `tool_use` block and its `input_json_delta`s.
    ToolCallDelta {
        index: usize,
        id: Option<String>,
        name: Option<String>,
        arguments: String,
    },
    /// A tool call that arrives whole (Google's `functionCall`).
    ToolCall(ToolCall),
    /// A finished Responses `message` / `reasoning` item.
    Item(StoredItem),
    Done,
    Error(String),
}

/// One SSE `data:` payload → the events it carries.
fn parse_event(family: Family, data: &str) -> Vec<Event> {
    if data.trim() == "[DONE]" {
        return vec![Event::Done];
    }
    let Ok(value) = serde_json::from_str::<Value>(data) else {
        return Vec::new();
    };
    let mut events = Vec::new();
    match family {
        Family::OpenAiCompatible => {
            if let Some(error) = value.get("error") {
                events.push(Event::Error(api_error_message(error)));
                return events;
            }
            let Some(choice) = value
                .get("choices")
                .and_then(|c| c.as_array())
                .and_then(|c| c.first())
            else {
                return events;
            };
            if let Some(delta) = choice.get("delta") {
                for key in ["reasoning_content", "reasoning"] {
                    if let Some(text) = delta.get(key).and_then(|t| t.as_str())
                        && !text.is_empty()
                    {
                        events.push(Event::Reasoning(text.to_string()));
                    }
                }
                if let Some(text) = delta.get("content").and_then(|t| t.as_str())
                    && !text.is_empty()
                {
                    events.push(Event::Text(text.to_string()));
                }
                for (position, call) in delta
                    .get("tool_calls")
                    .and_then(|c| c.as_array())
                    .into_iter()
                    .flatten()
                    .enumerate()
                {
                    let function = call.get("function");
                    events.push(Event::ToolCallDelta {
                        index: call
                            .get("index")
                            .and_then(|i| i.as_u64())
                            .map_or(position, |i| i as usize),
                        id: call
                            .get("id")
                            .and_then(|i| i.as_str())
                            .filter(|i| !i.is_empty())
                            .map(str::to_string),
                        name: function
                            .and_then(|f| f.get("name"))
                            .and_then(|n| n.as_str())
                            .filter(|n| !n.is_empty())
                            .map(str::to_string),
                        arguments: function
                            .and_then(|f| f.get("arguments"))
                            .and_then(|a| a.as_str())
                            .unwrap_or_default()
                            .to_string(),
                    });
                }
            }
            if choice
                .get("finish_reason")
                .and_then(|f| f.as_str())
                .is_some_and(|f| !f.is_empty())
            {
                events.push(Event::Done);
            }
        }
        Family::OpenAiResponses => match value.get("type").and_then(|t| t.as_str()) {
            Some("response.output_text.delta") => {
                if let Some(text) = value.get("delta").and_then(|t| t.as_str())
                    && !text.is_empty()
                {
                    events.push(Event::Text(text.to_string()));
                }
            }
            Some("response.reasoning_summary_text.delta") => {
                if let Some(text) = value.get("delta").and_then(|t| t.as_str())
                    && !text.is_empty()
                {
                    events.push(Event::Reasoning(text.to_string()));
                }
            }
            // The SDK's `tool-call` comes from the finished item (its
            // `arguments` complete), not from the argument deltas.
            Some("response.output_item.done") => {
                if let Some(item) = value.get("item") {
                    let id = item
                        .get("id")
                        .and_then(|i| i.as_str())
                        .filter(|id| !id.is_empty())
                        .map(str::to_string);
                    match (item.get("type").and_then(|t| t.as_str()), id) {
                        (Some("function_call"), _) => {
                            events.push(Event::ToolCall(responses_function_call(
                                item,
                                value
                                    .get("output_index")
                                    .and_then(|i| i.as_u64())
                                    .unwrap_or(0) as usize,
                            )));
                        }
                        (Some("message"), Some(id)) => {
                            events.push(Event::Item(StoredItem::Message { id }));
                        }
                        (Some("reasoning"), Some(id)) => {
                            events.push(Event::Item(StoredItem::Reasoning {
                                id,
                                encrypted_content: item
                                    .get("encrypted_content")
                                    .and_then(|c| c.as_str())
                                    .map(str::to_string),
                            }));
                        }
                        _ => {}
                    }
                }
            }
            Some("response.completed" | "response.incomplete") => events.push(Event::Done),
            Some("response.failed") => {
                let error = value
                    .get("response")
                    .and_then(|r| r.get("error"))
                    .cloned()
                    .unwrap_or(Value::Null);
                events.push(Event::Error(api_error_message(&error)));
            }
            Some("error") => events.push(Event::Error(api_error_message(&value))),
            _ => {}
        },
        Family::Anthropic => match value.get("type").and_then(|t| t.as_str()) {
            Some("content_block_start") => {
                if let Some(block) = value.get("content_block")
                    && block.get("type").and_then(|t| t.as_str()) == Some("tool_use")
                {
                    events.push(Event::ToolCallDelta {
                        index: value.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize,
                        id: block.get("id").and_then(|i| i.as_str()).map(str::to_string),
                        name: block
                            .get("name")
                            .and_then(|n| n.as_str())
                            .map(str::to_string),
                        arguments: String::new(),
                    });
                }
            }
            Some("content_block_delta") => {
                if let Some(delta) = value.get("delta") {
                    match delta.get("type").and_then(|t| t.as_str()) {
                        Some("text_delta") => {
                            if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                events.push(Event::Text(text.to_string()));
                            }
                        }
                        Some("thinking_delta") => {
                            if let Some(text) = delta.get("thinking").and_then(|t| t.as_str()) {
                                events.push(Event::Reasoning(text.to_string()));
                            }
                        }
                        Some("input_json_delta") => {
                            events.push(Event::ToolCallDelta {
                                index: value.get("index").and_then(|i| i.as_u64()).unwrap_or(0)
                                    as usize,
                                id: None,
                                name: None,
                                arguments: delta
                                    .get("partial_json")
                                    .and_then(|j| j.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                            });
                        }
                        _ => {}
                    }
                }
            }
            Some("message_stop") => events.push(Event::Done),
            Some("error") => {
                events.push(Event::Error(api_error_message(
                    value.get("error").unwrap_or(&Value::Null),
                )));
            }
            _ => {}
        },
        Family::Google => {
            if let Some(error) = value.get("error") {
                events.push(Event::Error(api_error_message(error)));
                return events;
            }
            let parts = value
                .get("candidates")
                .and_then(|c| c.as_array())
                .and_then(|c| c.first())
                .and_then(|c| c.get("content"))
                .and_then(|c| c.get("parts"))
                .and_then(|p| p.as_array());
            for (position, part) in parts.into_iter().flatten().enumerate() {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    if part.get("thought").and_then(|t| t.as_bool()) == Some(true) {
                        events.push(Event::Reasoning(text.to_string()));
                    } else {
                        events.push(Event::Text(text.to_string()));
                    }
                }
                if let Some(call) = part.get("functionCall") {
                    events.push(Event::ToolCall(ToolCall {
                        id: call
                            .get("id")
                            .and_then(|i| i.as_str())
                            .map(str::to_string)
                            .unwrap_or_else(|| format!("call_{position}")),
                        name: call
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        arguments: call.get("args").cloned().unwrap_or_else(|| json!({})),
                        item_id: None,
                    }));
                }
            }
            if value
                .get("candidates")
                .and_then(|c| c.as_array())
                .and_then(|c| c.first())
                .and_then(|c| c.get("finishReason"))
                .and_then(|f| f.as_str())
                .is_some()
            {
                events.push(Event::Done);
            }
        }
    }
    events
}

/// A Responses `function_call` item: `call_id`, `name`, the `arguments`
/// JSON text parsed (an unparsable string becomes `{}`), and the item id.
fn responses_function_call(item: &Value, position: usize) -> ToolCall {
    let arguments = item
        .get("arguments")
        .and_then(Value::as_str)
        .unwrap_or_default();
    ToolCall {
        id: item
            .get("call_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .map_or_else(|| format!("call_{position}"), str::to_string),
        name: item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        arguments: serde_json::from_str(arguments).unwrap_or_else(|_| json!({})),
        item_id: item
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .map(str::to_string),
    }
}

/// Assembles streamed tool-call fragments by index; `finish` yields the
/// calls in index order with their arguments parsed (an unparsable
/// argument string becomes `{}`, like the SDK's lenient parse).
#[derive(Default)]
struct ToolCallAssembler {
    calls: Vec<(usize, ToolCall, String)>,
}

impl ToolCallAssembler {
    fn push(&mut self, index: usize, id: Option<String>, name: Option<String>, arguments: &str) {
        let entry = match self.calls.iter_mut().find(|(i, _, _)| *i == index) {
            Some(entry) => entry,
            None => {
                self.calls.push((index, ToolCall::default(), String::new()));
                self.calls.last_mut().expect("just pushed")
            }
        };
        if let Some(id) = id {
            entry.1.id = id;
        }
        if let Some(name) = name {
            entry.1.name = name;
        }
        entry.2.push_str(arguments);
    }

    fn finish(&mut self) -> Vec<ToolCall> {
        let mut calls = std::mem::take(&mut self.calls);
        calls.sort_by_key(|(index, _, _)| *index);
        calls
            .into_iter()
            .enumerate()
            .filter(|(_, (_, call, _))| !call.name.is_empty())
            .map(|(position, (_, mut call, arguments))| {
                call.arguments = if arguments.trim().is_empty() {
                    json!({})
                } else {
                    serde_json::from_str(&arguments).unwrap_or_else(|_| json!({}))
                };
                if call.id.is_empty() {
                    call.id = format!("call_{position}");
                }
                call
            })
            .collect()
    }
}

fn api_error_message(error: &Value) -> String {
    error
        .get("message")
        .and_then(|m| m.as_str())
        .map(str::to_string)
        .or_else(|| error.as_str().map(str::to_string))
        .unwrap_or_else(|| error.to_string())
}

/// `extractReasoningMiddleware({ tagName })` for `think` and `thinking`:
/// text inside the tags becomes reasoning; the tags themselves vanish.
#[derive(Debug, Default)]
pub struct ReasoningExtractor {
    pending: String,
    inside: Option<&'static str>,
}

const TAGS: [&str; 2] = ["thinking", "think"];

impl ReasoningExtractor {
    pub fn push(&mut self, text: &str) -> Vec<Chunk> {
        self.pending.push_str(text);
        let mut out = Vec::new();
        loop {
            match self.inside {
                None => {
                    // Emit everything up to a possible opening tag.
                    let Some(lt) = self.pending.find('<') else {
                        if !self.pending.is_empty() {
                            out.push(Chunk::TextDelta(std::mem::take(&mut self.pending)));
                        }
                        break;
                    };
                    if lt > 0 {
                        out.push(Chunk::TextDelta(self.pending[..lt].to_string()));
                        self.pending = self.pending[lt..].to_string();
                    }
                    let mut matched = None;
                    let mut could_match = false;
                    for tag in TAGS {
                        let open = format!("<{tag}>");
                        if self.pending.starts_with(&open) {
                            matched = Some((tag, open.len()));
                            break;
                        }
                        if open.starts_with(&self.pending) {
                            could_match = true;
                        }
                    }
                    match matched {
                        Some((tag, len)) => {
                            self.pending = self.pending[len..].to_string();
                            self.inside = Some(tag);
                        }
                        None if could_match => break,
                        None => {
                            // A `<` that is not one of our tags: emit it.
                            out.push(Chunk::TextDelta(self.pending[..1].to_string()));
                            self.pending = self.pending[1..].to_string();
                        }
                    }
                }
                Some(tag) => {
                    let close = format!("</{tag}>");
                    if let Some(end) = self.pending.find(&close) {
                        if end > 0 {
                            out.push(Chunk::ReasoningDelta(self.pending[..end].to_string()));
                        }
                        self.pending = self.pending[end + close.len()..].to_string();
                        self.inside = None;
                        // The middleware drops the newline right after the tag.
                        if let Some(rest) = self.pending.strip_prefix('\n') {
                            self.pending = rest.to_string();
                        }
                    } else {
                        // Keep a possible partial closing tag buffered.
                        let keep = (0..close.len())
                            .rev()
                            .find(|len| self.pending.ends_with(&close[..*len]))
                            .unwrap_or(0);
                        let emit_to = self.pending.len() - keep;
                        if emit_to > 0 {
                            out.push(Chunk::ReasoningDelta(self.pending[..emit_to].to_string()));
                            self.pending = self.pending[emit_to..].to_string();
                        }
                        break;
                    }
                }
            }
        }
        out
    }

    pub fn finish(&mut self) -> Vec<Chunk> {
        let rest = std::mem::take(&mut self.pending);
        if rest.is_empty() {
            return Vec::new();
        }
        vec![match self.inside {
            Some(_) => Chunk::ReasoningDelta(rest),
            None => Chunk::TextDelta(rest),
        }]
    }
}

fn retryable_status(status: u16) -> bool {
    matches!(status, 408 | 409 | 429) || status >= 500
}

/// Start a generation: chunks arrive on the receiver until `Done` or
/// `Error`; dropping the receiver cancels the request.
pub fn stream(
    runtime: &tokio::runtime::Handle,
    conn: Connection,
    request: Request,
) -> mpsc::UnboundedReceiver<Chunk> {
    let (sender, receiver) = mpsc::unbounded_channel();
    runtime.spawn(async move {
        let http = match build_request(&conn, &request) {
            Ok(http) => http,
            Err(error) => {
                let _ = sender.send(Chunk::Error(error));
                return;
            }
        };
        let client = match reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .build()
        {
            Ok(client) => client,
            Err(error) => {
                let _ = sender.send(Chunk::Error(error.to_string()));
                return;
            }
        };
        let mut attempt = 0;
        loop {
            match run_once(&client, &http, &sender).await {
                Ok(()) => return,
                Err(Retry::Give(message)) => {
                    let _ = sender.send(Chunk::Error(message));
                    return;
                }
                Err(Retry::Again(message)) => {
                    attempt += 1;
                    if attempt > MAX_RETRIES || sender.is_closed() {
                        let _ = sender.send(Chunk::Error(message));
                        return;
                    }
                    // The AI SDK's exponential backoff: 2s, 4s, 8s, 16s.
                    tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
                }
            }
        }
    });
    receiver
}

enum Retry {
    Again(String),
    Give(String),
}

/// A whole reply: `generateText`'s text and tool calls.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Generated {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
}

/// `generateText`: one complete reply, retried like the SDK (`maxRetries`
/// attempts after the first, backing off 2s, 4s, ...).
pub async fn generate(
    conn: &Connection,
    request: &Request,
    max_retries: usize,
) -> Result<Generated, String> {
    let http = build_generate_request(conn, request)?;
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| error.to_string())?;
    let mut attempt = 0;
    loop {
        match generate_once(&client, &http).await {
            Ok(generated) => return Ok(generated),
            Err(Retry::Give(message)) => return Err(message),
            Err(Retry::Again(message)) => {
                attempt += 1;
                if attempt > max_retries {
                    return Err(message);
                }
                tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
            }
        }
    }
}

/// `Output.object` over a whole reply: the object the model produced,
/// whether as text or as Anthropic's `json` tool call.
pub async fn generate_object(
    conn: &Connection,
    request: &Request,
    max_retries: usize,
) -> Result<Value, String> {
    let generated = generate(conn, request, max_retries).await?;
    if let Some(call) = generated.tool_calls.iter().find(|call| call.name == "json") {
        return Ok(call.arguments.clone());
    }
    serde_json::from_str(generated.text.trim())
        .map_err(|_| "No object generated: could not parse the response.".to_string())
}

async fn generate_once(client: &reqwest::Client, http: &HttpRequest) -> Result<Generated, Retry> {
    let mut builder = client.post(&http.url).json(&http.body);
    for (name, value) in &http.headers {
        builder = builder.header(*name, value);
    }
    let response = builder.send().await.map_err(|error| {
        if error.is_connect() || error.is_timeout() || error.is_request() {
            Retry::Again(error.to_string())
        } else {
            Retry::Give(error.to_string())
        }
    })?;
    let status = response.status().as_u16();
    let body = response
        .text()
        .await
        .map_err(|error| Retry::Again(error.to_string()))?;
    if status >= 400 {
        let message = crate::ai_health::llm_health_error_message(status, &body);
        return Err(if retryable_status(status) {
            Retry::Again(message)
        } else {
            Retry::Give(message)
        });
    }
    let value: Value = serde_json::from_str(&body)
        .map_err(|_| Retry::Give("The language model returned an unreadable reply.".to_string()))?;
    parse_generated(http.family, &value).map_err(Retry::Give)
}

/// A complete reply's text and tool calls, per family, with the thinking
/// tags stripped like `extractReasoningMiddleware`'s `wrapGenerate`.
fn parse_generated(family: Family, value: &Value) -> Result<Generated, String> {
    if let Some(error) = value.get("error") {
        return Err(api_error_message(error));
    }
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    match family {
        Family::OpenAiCompatible => {
            let message = value
                .get("choices")
                .and_then(Value::as_array)
                .and_then(|choices| choices.first())
                .and_then(|choice| choice.get("message"));
            if let Some(content) = message
                .and_then(|message| message.get("content"))
                .and_then(Value::as_str)
            {
                text.push_str(content);
            }
            for (position, call) in message
                .and_then(|message| message.get("tool_calls"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .enumerate()
            {
                let function = call.get("function");
                let arguments = function
                    .and_then(|f| f.get("arguments"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                tool_calls.push(ToolCall {
                    id: call
                        .get("id")
                        .and_then(Value::as_str)
                        .filter(|id| !id.is_empty())
                        .map_or_else(|| format!("call_{position}"), str::to_string),
                    name: function
                        .and_then(|f| f.get("name"))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    arguments: serde_json::from_str(arguments).unwrap_or_else(|_| json!({})),
                    item_id: None,
                });
            }
        }
        Family::OpenAiResponses => {
            for (position, item) in value
                .get("output")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .enumerate()
            {
                match item.get("type").and_then(Value::as_str) {
                    Some("message") => {
                        for part in item
                            .get("content")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                        {
                            if part.get("type").and_then(Value::as_str) == Some("output_text")
                                && let Some(text_part) = part.get("text").and_then(Value::as_str)
                            {
                                text.push_str(text_part);
                            }
                        }
                    }
                    Some("function_call") => {
                        tool_calls.push(responses_function_call(item, position));
                    }
                    _ => {}
                }
            }
        }
        Family::Anthropic => {
            for block in value
                .get("content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                match block.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(part) = block.get("text").and_then(Value::as_str) {
                            text.push_str(part);
                        }
                    }
                    Some("tool_use") => tool_calls.push(ToolCall {
                        id: block
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        name: block
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        arguments: block.get("input").cloned().unwrap_or_else(|| json!({})),
                        item_id: None,
                    }),
                    _ => {}
                }
            }
        }
        Family::Google => {
            let parts = value
                .get("candidates")
                .and_then(Value::as_array)
                .and_then(|candidates| candidates.first())
                .and_then(|candidate| candidate.get("content"))
                .and_then(|content| content.get("parts"))
                .and_then(Value::as_array);
            for (position, part) in parts.into_iter().flatten().enumerate() {
                if part.get("thought").and_then(Value::as_bool) == Some(true) {
                    continue;
                }
                if let Some(part_text) = part.get("text").and_then(Value::as_str) {
                    text.push_str(part_text);
                }
                if let Some(call) = part.get("functionCall") {
                    tool_calls.push(ToolCall {
                        id: call
                            .get("id")
                            .and_then(Value::as_str)
                            .map_or_else(|| format!("call_{position}"), str::to_string),
                        name: call
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        arguments: call.get("args").cloned().unwrap_or_else(|| json!({})),
                        item_id: None,
                    });
                }
            }
        }
    }
    let mut extractor = ReasoningExtractor::default();
    let mut chunks = extractor.push(&text);
    chunks.extend(extractor.finish());
    Ok(Generated {
        text: chunks
            .into_iter()
            .filter_map(|chunk| match chunk {
                Chunk::TextDelta(text) => Some(text),
                _ => None,
            })
            .collect(),
        tool_calls,
    })
}

async fn run_once(
    client: &reqwest::Client,
    http: &HttpRequest,
    sender: &mpsc::UnboundedSender<Chunk>,
) -> Result<(), Retry> {
    let mut builder = client.post(&http.url).json(&http.body);
    for (name, value) in &http.headers {
        builder = builder.header(*name, value);
    }
    let response = builder.send().await.map_err(|error| {
        if error.is_connect() || error.is_timeout() || error.is_request() {
            Retry::Again(error.to_string())
        } else {
            Retry::Give(error.to_string())
        }
    })?;
    let status = response.status().as_u16();
    if status >= 400 {
        let body = response.text().await.unwrap_or_default();
        let message = crate::ai_health::llm_health_error_message(status, &body);
        return Err(if retryable_status(status) {
            Retry::Again(message)
        } else {
            Retry::Give(message)
        });
    }

    let mut body = response.bytes_stream();
    let mut buffer = String::new();
    let mut extractor = ReasoningExtractor::default();
    let mut assembler = ToolCallAssembler::default();
    let mut emitted_any = false;
    let mut data_lines: Vec<String> = Vec::new();
    let deliver = |sender: &mpsc::UnboundedSender<Chunk>, chunks: Vec<Chunk>| -> bool {
        for chunk in chunks {
            if sender.send(chunk).is_err() {
                return false;
            }
        }
        true
    };
    // One event's contribution; `false` once the receiver is gone.
    let mut handle = |event: Event,
                      extractor: &mut ReasoningExtractor,
                      assembler: &mut ToolCallAssembler|
     -> bool {
        match event {
            Event::Text(text) => {
                emitted_any = true;
                deliver(sender, extractor.push(&text))
            }
            Event::Reasoning(text) => {
                emitted_any = true;
                deliver(sender, vec![Chunk::ReasoningDelta(text)])
            }
            Event::ToolCallDelta {
                index,
                id,
                name,
                arguments,
            } => {
                emitted_any = true;
                assembler.push(index, id, name, &arguments);
                true
            }
            Event::ToolCall(call) => {
                emitted_any = true;
                deliver(sender, vec![Chunk::ToolCall(call)])
            }
            Event::Item(item) => deliver(sender, vec![Chunk::Item(item)]),
            Event::Error(error) => {
                emitted_any = true;
                deliver(sender, vec![Chunk::Error(error)])
            }
            Event::Done => true,
        }
    };
    while let Some(next) = body.next().await {
        let bytes = match next {
            Ok(bytes) => bytes,
            Err(error) => {
                return Err(if emitted_any {
                    Retry::Give(error.to_string())
                } else {
                    Retry::Again(error.to_string())
                });
            }
        };
        buffer.push_str(&String::from_utf8_lossy(&bytes));
        while let Some(newline) = buffer.find('\n') {
            let line = buffer[..newline].trim_end_matches('\r').to_string();
            buffer = buffer[newline + 1..].to_string();
            if line.is_empty() {
                // Event boundary.
                if !data_lines.is_empty() {
                    let data = data_lines.join("\n");
                    data_lines.clear();
                    for event in parse_event(http.family, &data) {
                        if !handle(event, &mut extractor, &mut assembler) {
                            return Ok(());
                        }
                    }
                }
                continue;
            }
            if let Some(data) = line.strip_prefix("data:") {
                data_lines.push(data.strip_prefix(' ').unwrap_or(data).to_string());
            }
        }
        if sender.is_closed() {
            return Ok(());
        }
    }
    if !data_lines.is_empty() {
        let data = data_lines.join("\n");
        for event in parse_event(http.family, &data) {
            if !handle(event, &mut extractor, &mut assembler) {
                return Ok(());
            }
        }
    }
    deliver(sender, extractor.finish());
    deliver(
        sender,
        assembler
            .finish()
            .into_iter()
            .map(Chunk::ToolCall)
            .collect(),
    );
    let _ = sender.send(Chunk::Done);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn(provider: &str, model: &str, effort: &str) -> Connection {
        Connection {
            provider_id: provider.into(),
            base_url: "https://api.example/v1/".into(),
            api_key: "k".into(),
            model_id: model.into(),
            reasoning_effort: effort.into(),
        }
    }

    fn request() -> Request {
        Request::new("sys", "hi", 8192)
    }

    #[test]
    fn history_turns_precede_the_prompt_per_family() {
        let mut request = Request::new("sys", "third", 0);
        request.messages = vec![
            Turn::User("first".into()),
            Turn::Assistant {
                text: "second".into(),
                tool_calls: Vec::new(),
            },
            Turn::User("third".into()),
        ];
        let chat = build_request(&conn("openrouter", "gpt-5.6", "default"), &request).unwrap();
        assert_eq!(
            chat.body["messages"],
            json!([
                { "role": "system", "content": "sys" },
                { "role": "user", "content": "first" },
                { "role": "assistant", "content": "second" },
                { "role": "user", "content": "third" }
            ])
        );
        // No cap requested: the provider default stands.
        assert!(chat.body.get("max_tokens").is_none());
        // The Responses API's `input` items, the system prompt a `developer`
        // message for a reasoning model.
        let openai = build_request(&conn("openai", "gpt-5.6", "default"), &request).unwrap();
        assert_eq!(
            openai.body["input"],
            json!([
                { "role": "developer", "content": "sys" },
                { "role": "user", "content": [{ "type": "input_text", "text": "first" }] },
                { "role": "assistant", "content": [{ "type": "output_text", "text": "second" }] },
                { "role": "user", "content": [{ "type": "input_text", "text": "third" }] }
            ])
        );
        assert!(openai.body.get("max_output_tokens").is_none());
        let plain = build_request(&conn("openai", "gpt-4o", "default"), &request).unwrap();
        assert_eq!(plain.body["input"][0]["role"], "system");
        // The chat's context block rides as its own text part in every shape.
        let mut with_context = Request::new("sys", "", 0);
        with_context.messages = vec![Turn::UserParts(vec!["<context/>\n\n".into(), "q".into()])];
        let chat = build_request(&conn("openrouter", "m", "default"), &with_context).unwrap();
        assert_eq!(
            chat.body["messages"][1]["content"],
            json!([{ "type": "text", "text": "<context/>\n\n" }, { "type": "text", "text": "q" }])
        );
        let responses = build_request(&conn("openai", "gpt-4o", "default"), &with_context).unwrap();
        assert_eq!(
            responses.body["input"][1]["content"],
            json!([
                { "type": "input_text", "text": "<context/>\n\n" },
                { "type": "input_text", "text": "q" }
            ])
        );
        let anthropic =
            build_request(&conn("anthropic", "claude", "default"), &with_context).unwrap();
        assert_eq!(
            anthropic.body["messages"][0]["content"],
            json!([{ "type": "text", "text": "<context/>\n\n" }, { "type": "text", "text": "q" }])
        );
        let google = build_request(
            &conn("google_generative_ai", "gemini", "default"),
            &with_context,
        )
        .unwrap();
        assert_eq!(
            google.body["contents"][0]["parts"],
            json!([{ "text": "<context/>\n\n" }, { "text": "q" }])
        );
        let anthropic = build_request(&conn("anthropic", "claude", "default"), &request).unwrap();
        assert_eq!(anthropic.body["messages"].as_array().unwrap().len(), 3);
        assert_eq!(anthropic.body["max_tokens"], 4096);
        let google =
            build_request(&conn("google_generative_ai", "gemini", "default"), &request).unwrap();
        assert_eq!(google.body["contents"][1]["role"], "model");
        assert_eq!(google.body["contents"][2]["parts"][0]["text"], "third");
    }

    #[test]
    fn tool_calls_and_results_follow_each_family() {
        let call = ToolCall {
            id: "call_1".into(),
            name: "list_meetings".into(),
            arguments: json!({ "limit": 3 }),
            item_id: None,
        };
        let mut request = Request::new("sys", "list", 0);
        request.messages.push(Turn::Assistant {
            text: String::new(),
            tool_calls: vec![call.clone()],
        });
        request.messages.push(Turn::ToolResult {
            call_id: "call_1".into(),
            name: "list_meetings".into(),
            output: r#"{"meetings":[]}"#.into(),
            is_error: false,
        });
        request.tools = vec![ToolSpec {
            name: "list_meetings".into(),
            description: "List meetings".into(),
            parameters: json!({ "type": "object", "properties": {} }),
        }];
        let chat = build_request(&conn("openrouter", "gpt-5.6", "default"), &request).unwrap();
        assert_eq!(chat.body["tool_choice"], "auto");
        assert_eq!(chat.body["tools"][0]["function"]["name"], "list_meetings");
        assert_eq!(
            chat.body["messages"][2],
            json!({
                "role": "assistant",
                "content": "",
                "tool_calls": [{ "id": "call_1", "type": "function", "function": { "name": "list_meetings", "arguments": "{\"limit\":3}" } }]
            })
        );
        assert_eq!(
            chat.body["messages"][3],
            json!({ "role": "tool", "tool_call_id": "call_1", "content": "{\"meetings\":[]}" })
        );
        // Responses: flat `function` tools, `function_call` items with the
        // arguments as JSON text, `function_call_output` results.
        let openai = build_request(&conn("openai", "gpt-5.6", "default"), &request).unwrap();
        assert_eq!(openai.body["tool_choice"], "auto");
        assert_eq!(
            openai.body["tools"][0],
            json!({
                "type": "function",
                "name": "list_meetings",
                "description": "List meetings",
                "parameters": { "type": "object", "properties": {} }
            })
        );
        assert_eq!(
            openai.body["input"][2],
            json!({ "type": "function_call", "call_id": "call_1", "name": "list_meetings", "arguments": "{\"limit\":3}" })
        );
        assert_eq!(
            openai.body["input"][3],
            json!({ "type": "function_call_output", "call_id": "call_1", "output": "{\"meetings\":[]}" })
        );
        // A call from this conversation's own stream refers to its stored item.
        let mut same_conversation = request.clone();
        same_conversation.messages[1] = Turn::Assistant {
            text: String::new(),
            tool_calls: vec![ToolCall {
                item_id: Some("fc_1".into()),
                ..call.clone()
            }],
        };
        let openai =
            build_request(&conn("openai", "gpt-5.6", "default"), &same_conversation).unwrap();
        assert_eq!(
            openai.body["input"][2],
            json!({ "type": "item_reference", "id": "fc_1" })
        );
        let anthropic = build_request(&conn("anthropic", "claude", "default"), &request).unwrap();
        assert_eq!(anthropic.body["tools"][0]["input_schema"]["type"], "object");
        assert_eq!(
            anthropic.body["messages"][1]["content"][0]["type"],
            "tool_use"
        );
        assert_eq!(
            anthropic.body["messages"][2]["content"][0]["tool_use_id"],
            "call_1"
        );
        assert!(
            anthropic.body["messages"][2]["content"][0]
                .get("is_error")
                .is_none()
        );
        // A failed tool answers with `is_error` for Anthropic only.
        let mut failed = request.clone();
        failed.messages[2] = Turn::ToolResult {
            call_id: "call_1".into(),
            name: "list_meetings".into(),
            output: "boom".into(),
            is_error: true,
        };
        let anthropic_failed =
            build_request(&conn("anthropic", "claude", "default"), &failed).unwrap();
        assert_eq!(
            anthropic_failed.body["messages"][2]["content"][0],
            json!({ "type": "tool_result", "tool_use_id": "call_1", "content": "boom", "is_error": true })
        );
        let chat_failed = build_request(&conn("openrouter", "m", "default"), &failed).unwrap();
        assert_eq!(
            chat_failed.body["messages"][3],
            json!({ "role": "tool", "tool_call_id": "call_1", "content": "boom" })
        );
        let google =
            build_request(&conn("google_generative_ai", "gemini", "default"), &request).unwrap();
        assert_eq!(
            google.body["tools"][0]["functionDeclarations"][0]["name"],
            "list_meetings"
        );
        assert_eq!(
            google.body["contents"][1]["parts"][0]["functionCall"]["args"]["limit"],
            3
        );
        assert_eq!(
            google.body["contents"][2]["parts"][0]["functionResponse"]["response"]["meetings"],
            json!([])
        );
        // The request without tools carries neither key.
        let plain = build_request(
            &conn("openai", "gpt-5.6", "default"),
            &Request::new("s", "p", 0),
        )
        .unwrap();
        assert!(plain.body.get("tools").is_none());
    }

    #[test]
    fn streamed_tool_calls_assemble_per_family() {
        assert_eq!(
            parse_event(
                Family::OpenAiCompatible,
                r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"list_meetings","arguments":""}}]}}]}"#
            ),
            vec![Event::ToolCallDelta {
                index: 0,
                id: Some("call_1".into()),
                name: Some("list_meetings".into()),
                arguments: String::new(),
            }]
        );
        let mut assembler = ToolCallAssembler::default();
        assembler.push(0, Some("call_1".into()), Some("list_meetings".into()), "");
        assembler.push(0, None, None, "{\"lim");
        assembler.push(0, None, None, "it\": 3}");
        assembler.push(1, None, Some("get_meeting".into()), "not json");
        assert_eq!(
            assembler.finish(),
            vec![
                ToolCall {
                    id: "call_1".into(),
                    name: "list_meetings".into(),
                    arguments: json!({ "limit": 3 }),
                    item_id: None,
                },
                ToolCall {
                    id: "call_1".into(),
                    name: "get_meeting".into(),
                    arguments: json!({}),
                    item_id: None,
                },
            ]
        );
        assert_eq!(
            parse_event(
                Family::Anthropic,
                r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_1","name":"search_meetings","input":{}}}"#
            ),
            vec![Event::ToolCallDelta {
                index: 1,
                id: Some("toolu_1".into()),
                name: Some("search_meetings".into()),
                arguments: String::new(),
            }]
        );
        assert_eq!(
            parse_event(
                Family::Anthropic,
                r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"query\":"}}"#
            ),
            vec![Event::ToolCallDelta {
                index: 1,
                id: None,
                name: None,
                arguments: "{\"query\":".into(),
            }]
        );
        assert_eq!(
            parse_event(
                Family::Google,
                r#"{"candidates":[{"content":{"parts":[{"functionCall":{"name":"search_contacts","args":{"query":"ada"}}}]}}]}"#
            ),
            vec![Event::ToolCall(ToolCall {
                id: "call_0".into(),
                name: "search_contacts".into(),
                arguments: json!({ "query": "ada" }),
                item_id: None,
            })]
        );
    }

    #[test]
    fn image_parts_take_each_provider_shape() {
        let images = vec![ImagePart {
            base64: "QUJD".into(),
            mime_type: "image/png".into(),
        }];
        let request = Request::with_images("sys", "look", images, 0);
        assert!(
            matches!(&request.messages[0], Turn::UserWithImages { text, images } if text == "look" && images.len() == 1)
        );
        assert!(matches!(
            Request::with_images("sys", "plain", Vec::new(), 0).messages[0],
            Turn::User(_)
        ));

        let chat = build_request(&conn("openrouter", "gpt-5.6", "default"), &request).unwrap();
        assert_eq!(
            chat.body["messages"][1]["content"],
            json!([
                { "type": "text", "text": "look" },
                { "type": "image_url", "image_url": { "url": "data:image/png;base64,QUJD" } }
            ])
        );
        let openai = build_request(&conn("openai", "gpt-5.6", "default"), &request).unwrap();
        assert_eq!(
            openai.body["input"][1]["content"],
            json!([
                { "type": "input_text", "text": "look" },
                { "type": "input_image", "image_url": "data:image/png;base64,QUJD" }
            ])
        );
        let anthropic =
            build_request(&conn("anthropic", "claude-sonnet-4", "default"), &request).unwrap();
        assert_eq!(
            anthropic.body["messages"][0]["content"],
            json!([
                { "type": "text", "text": "look" },
                { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": "QUJD" } }
            ])
        );
        let google = build_request(
            &conn("google_generative_ai", "gemini-2.5-flash", "default"),
            &request,
        )
        .unwrap();
        assert_eq!(
            google.body["contents"][0]["parts"],
            json!([
                { "text": "look" },
                { "inlineData": { "mimeType": "image/png", "data": "QUJD" } }
            ])
        );
    }

    #[test]
    fn requests_follow_the_provider_family() {
        let openai = build_request(&conn("openai", "gpt-5.6", "high"), &request()).unwrap();
        assert_eq!(openai.url, "https://api.example/v1/responses");
        assert_eq!(
            openai.headers,
            vec![("Authorization", "Bearer k".to_string())]
        );
        assert_eq!(openai.body["stream"], json!(true));
        assert_eq!(openai.body["max_output_tokens"], json!(8192));
        assert_eq!(openai.body["reasoning"], json!({ "effort": "high" }));
        assert_eq!(openai.body["input"][0]["role"], json!("developer"));
        // `reasoningEffort` is dropped for non-reasoning models.
        let chat_model = build_request(&conn("openai", "gpt-4o", "high"), &request()).unwrap();
        assert!(chat_model.body.get("reasoning").is_none());
        assert_eq!(chat_model.body["input"][0]["role"], json!("system"));
        let azure = build_request(&conn("azure_openai", "dep", "default"), &request()).unwrap();
        assert_eq!(
            azure.url,
            "https://api.example/v1/v1/responses?api-version=v1"
        );
        assert_eq!(azure.headers, vec![("api-key", "k".to_string())]);
        let compatible = build_request(&conn("groq", "m", "high"), &request()).unwrap();
        assert_eq!(compatible.url, "https://api.example/v1/chat/completions");
        assert_eq!(compatible.body["reasoning_effort"], json!("high"));
        assert_eq!(compatible.body["messages"][0]["role"], json!("system"));

        let custom = build_request(&conn("custom", "m", "default"), &request()).unwrap();
        assert!(custom.body.get("reasoning_effort").is_none());
        assert_eq!(
            custom.headers,
            vec![("Authorization", "Bearer k".to_string())]
        );

        let anthropic = build_request(&conn("anthropic", "claude", "low"), &request()).unwrap();
        assert_eq!(anthropic.url, "https://api.example/v1/messages");
        assert_eq!(
            anthropic.body["system"],
            json!([{ "type": "text", "text": "sys" }])
        );
        assert_eq!(
            anthropic.body["messages"][0]["content"],
            json!([{ "type": "text", "text": "hi" }])
        );
        assert_eq!(anthropic.body["thinking"]["type"], json!("adaptive"));
        assert_eq!(anthropic.body["output_config"]["effort"], json!("low"));

        let google = build_request(
            &conn("google_generative_ai", "gemini-2.5-pro", "medium"),
            &request(),
        )
        .unwrap();
        assert_eq!(
            google.url,
            "https://api.example/v1/models/gemini-2.5-pro:streamGenerateContent?alt=sse"
        );
        assert_eq!(
            google.body["generationConfig"]["thinkingConfig"]["thinkingBudget"],
            json!(8192)
        );
        let gemini3 = build_request(
            &conn("google_generative_ai", "gemini-3.8-flash", "high"),
            &request(),
        )
        .unwrap();
        assert_eq!(
            gemini3.body["generationConfig"]["thinkingConfig"]["thinkingLevel"],
            json!("high")
        );
        let old = build_request(
            &conn("google_generative_ai", "gemini-1.5-pro", "high"),
            &request(),
        )
        .unwrap();
        assert!(old.body["generationConfig"].get("thinkingConfig").is_none());

        let openrouter = build_request(&conn("openrouter", "m", "low"), &request()).unwrap();
        assert_eq!(openrouter.body["reasoning"]["effort"], json!("low"));

        let ollama = build_request(&conn("ollama", "llama", "default"), &request()).unwrap();
        assert!(ollama.headers.is_empty());

        assert!(build_request(&conn("anarlog", "m", "default"), &request()).is_err());
        assert!(build_request(&conn("apple_foundation", "m", "default"), &request()).is_err());
    }

    #[test]
    fn events_parse_per_family() {
        assert_eq!(
            parse_event(
                Family::OpenAiCompatible,
                r#"{"choices":[{"delta":{"content":"Hel","reasoning_content":"why"}}]}"#
            ),
            vec![Event::Reasoning("why".into()), Event::Text("Hel".into())]
        );
        assert_eq!(
            parse_event(
                Family::OpenAiCompatible,
                r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#
            ),
            vec![Event::Done]
        );
        assert_eq!(
            parse_event(Family::OpenAiCompatible, "[DONE]"),
            vec![Event::Done]
        );
        assert_eq!(
            parse_event(Family::OpenAiCompatible, r#"{"error":{"message":"boom"}}"#),
            vec![Event::Error("boom".into())]
        );
        assert_eq!(
            parse_event(
                Family::Anthropic,
                r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"Hi"}}"#
            ),
            vec![Event::Text("Hi".into())]
        );
        assert_eq!(
            parse_event(
                Family::Anthropic,
                r#"{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"hm"}}"#
            ),
            vec![Event::Reasoning("hm".into())]
        );
        assert_eq!(
            parse_event(Family::Anthropic, r#"{"type":"message_stop"}"#),
            vec![Event::Done]
        );
        assert_eq!(
            parse_event(
                Family::Google,
                r#"{"candidates":[{"content":{"parts":[{"text":"t","thought":true},{"text":"x"}]},"finishReason":"STOP"}]}"#
            ),
            vec![
                Event::Reasoning("t".into()),
                Event::Text("x".into()),
                Event::Done
            ]
        );
    }

    #[test]
    fn reasoning_tags_are_extracted_across_chunks() {
        let mut extractor = ReasoningExtractor::default();
        let mut out = Vec::new();
        for piece in ["<thi", "nk>plan", " more</thi", "nk>\n# Title", " a < b"] {
            out.extend(extractor.push(piece));
        }
        out.extend(extractor.finish());
        let text: String = out
            .iter()
            .filter_map(|c| match c {
                Chunk::TextDelta(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        let reasoning: String = out
            .iter()
            .filter_map(|c| match c {
                Chunk::ReasoningDelta(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(text, "# Title a < b");
        assert_eq!(reasoning, "plan more");
    }

    #[test]
    fn local_providers_are_recognised() {
        assert!(is_local_model_provider("ollama"));
        assert!(is_local_model_provider("lmstudio"));
        assert!(!is_local_model_provider("openai"));
    }

    fn object_request() -> Request {
        let mut request = Request::new("SYS", r#"{"target":{"name":"A"},"meetings":[]}"#, 4096);
        request.json_schema = Some(crate::contact_summary::schema());
        request
    }

    // The bodies below are what `generateText({ output: Output.object })`
    // sent through each `@ai-sdk` provider, captured against a fake fetch.
    #[test]
    fn generate_requests_take_each_provider_structured_output_shape() {
        let request = object_request();
        let compatible =
            build_generate_request(&conn("lmstudio", "m", "default"), &request).unwrap();
        assert_eq!(compatible.url, "https://api.example/v1/chat/completions");
        assert_eq!(compatible.body.get("stream"), None);
        assert_eq!(compatible.body["max_tokens"], 4096);
        assert_eq!(
            compatible.body["response_format"],
            json!({ "type": "json_object" })
        );

        let openai =
            build_generate_request(&conn("openai", "gpt-4o", "default"), &request).unwrap();
        assert_eq!(openai.body.get("stream"), None);
        assert_eq!(openai.body["max_output_tokens"], 4096);
        assert_eq!(
            openai.body["text"],
            json!({
                "format": {
                    "type": "json_schema",
                    "strict": true,
                    "name": "response",
                    "schema": crate::contact_summary::schema()
                }
            })
        );
        assert_eq!(openai.body.get("response_format"), None);

        let anthropic =
            build_generate_request(&conn("anthropic", "claude-sonnet-4-5", "default"), &request)
                .unwrap();
        assert_eq!(anthropic.body.get("stream"), None);
        assert_eq!(
            anthropic.body["output_config"],
            json!({ "format": { "type": "json_schema", "schema": crate::contact_summary::schema() } })
        );
        assert_eq!(anthropic.body.get("tools"), None);

        // Reasoning effort shares `output_config` with the format.
        let anthropic_effort =
            build_generate_request(&conn("anthropic", "claude-sonnet-4-5", "high"), &request)
                .unwrap();
        assert_eq!(anthropic_effort.body["output_config"]["effort"], "high");
        assert_eq!(
            anthropic_effort.body["output_config"]["format"]["type"],
            "json_schema"
        );

        let anthropic_tool = build_generate_request(
            &conn("anthropic", "claude-3-haiku-20240307", "default"),
            &request,
        )
        .unwrap();
        assert_eq!(anthropic_tool.body.get("output_config"), None);
        assert_eq!(
            anthropic_tool.body["tools"],
            json!([{
                "name": "json",
                "description": "Respond with a JSON object.",
                "input_schema": crate::contact_summary::schema()
            }])
        );
        assert_eq!(
            anthropic_tool.body["tool_choice"],
            json!({ "type": "any", "disable_parallel_tool_use": true })
        );

        let google = build_generate_request(
            &conn("google_generative_ai", "gemini-2.5-flash", "default"),
            &request,
        )
        .unwrap();
        assert_eq!(
            google.url,
            "https://api.example/v1/models/gemini-2.5-flash:generateContent"
        );
        assert_eq!(
            google.body["generationConfig"],
            json!({
                "maxOutputTokens": 4096,
                "responseMimeType": "application/json",
                "responseSchema": {
                    "required": ["facts"],
                    "type": "object",
                    "properties": { "facts": { "type": "array", "items": { "type": "string" } } }
                }
            })
        );
    }

    #[test]
    fn responses_events_and_replies_parse() {
        assert_eq!(
            parse_event(
                Family::OpenAiResponses,
                r#"{"type":"response.output_text.delta","item_id":"msg_1","output_index":0,"delta":"Hel"}"#
            ),
            vec![Event::Text("Hel".into())]
        );
        assert_eq!(
            parse_event(
                Family::OpenAiResponses,
                r#"{"type":"response.reasoning_summary_text.delta","item_id":"rs_1","summary_index":0,"delta":"think"}"#
            ),
            vec![Event::Reasoning("think".into())]
        );
        // The call arrives whole with the finished item; the `added` item and
        // the argument deltas carry nothing the SDK's `tool-call` needs.
        assert!(
            parse_event(
                Family::OpenAiResponses,
                r#"{"type":"response.output_item.added","output_index":1,"item":{"type":"function_call","id":"fc_1","call_id":"call_1","name":"list_meetings","arguments":""}}"#
            )
            .is_empty()
        );
        assert!(
            parse_event(
                Family::OpenAiResponses,
                r#"{"type":"response.function_call_arguments.delta","item_id":"fc_1","output_index":1,"delta":"{\"limit\":3}"}"#
            )
            .is_empty()
        );
        assert_eq!(
            parse_event(
                Family::OpenAiResponses,
                r#"{"type":"response.output_item.done","output_index":1,"item":{"type":"function_call","id":"fc_1","call_id":"call_1","name":"list_meetings","arguments":"{\"limit\":3}","status":"completed"}}"#
            ),
            vec![Event::ToolCall(ToolCall {
                id: "call_1".into(),
                name: "list_meetings".into(),
                arguments: json!({ "limit": 3 }),
                item_id: Some("fc_1".into()),
            })]
        );
        assert_eq!(
            parse_event(
                Family::OpenAiResponses,
                r#"{"type":"response.completed","response":{"id":"resp_1","usage":{}}}"#
            ),
            vec![Event::Done]
        );
        assert_eq!(
            parse_event(
                Family::OpenAiResponses,
                r#"{"type":"response.failed","response":{"error":{"message":"boom"}}}"#
            ),
            vec![Event::Error("boom".into())]
        );
        // A message item's text and a function_call item's arguments.
        let generated = parse_generated(
            Family::OpenAiResponses,
            &json!({
                "output": [
                    { "type": "reasoning", "summary": [] },
                    { "type": "message", "content": [{ "type": "output_text", "text": "hello" }] },
                    { "type": "function_call", "id": "fc_9", "call_id": "call_9", "name": "list_meetings", "arguments": "{\"limit\":1}" }
                ]
            }),
        )
        .unwrap();
        assert_eq!(generated.text, "hello");
        assert_eq!(
            generated.tool_calls,
            vec![ToolCall {
                id: "call_9".into(),
                name: "list_meetings".into(),
                arguments: json!({ "limit": 1 }),
                item_id: Some("fc_9".into()),
            }]
        );
    }

    #[test]
    fn streaming_requests_are_unchanged_without_a_schema() {
        let http = build_request(&conn("openai", "gpt-4o", "default"), &request()).unwrap();
        assert_eq!(http.body["stream"], true);
        assert_eq!(http.body.get("response_format"), None);
        let google = build_request(
            &conn("google_generative_ai", "gemini-2.5-flash", "default"),
            &request(),
        )
        .unwrap();
        assert!(google.url.ends_with(":streamGenerateContent?alt=sse"));
        assert_eq!(
            google.body["generationConfig"].get("responseMimeType"),
            None
        );
    }

    #[test]
    fn whole_replies_parse_per_family() {
        let openai = parse_generated(
            Family::OpenAiCompatible,
            &json!({
                "choices": [{ "message": {
                    "role": "assistant",
                    "content": "<think>hmm</think>\n{\"facts\":[\"a\"]}",
                    "tool_calls": [{ "id": "c1", "type": "function", "function": { "name": "f", "arguments": "{\"x\":1}" } }]
                } }]
            }),
        )
        .unwrap();
        assert_eq!(openai.text, "{\"facts\":[\"a\"]}");
        assert_eq!(openai.tool_calls[0].name, "f");
        assert_eq!(openai.tool_calls[0].arguments, json!({ "x": 1 }));

        let anthropic = parse_generated(
            Family::Anthropic,
            &json!({
                "content": [
                    { "type": "text", "text": "hi " },
                    { "type": "text", "text": "there" },
                    { "type": "tool_use", "id": "t1", "name": "json", "input": { "facts": ["a", "b", "c"] } }
                ]
            }),
        )
        .unwrap();
        assert_eq!(anthropic.text, "hi there");
        assert_eq!(
            anthropic.tool_calls[0].arguments["facts"],
            json!(["a", "b", "c"])
        );

        let google = parse_generated(
            Family::Google,
            &json!({
                "candidates": [{ "content": { "parts": [
                    { "text": "plan", "thought": true },
                    { "text": "{\"facts\":[]}" }
                ] } }]
            }),
        )
        .unwrap();
        assert_eq!(google.text, "{\"facts\":[]}");
        assert!(google.tool_calls.is_empty());

        let error = parse_generated(
            Family::OpenAiCompatible,
            &json!({ "error": { "message": "quota exceeded" } }),
        );
        assert_eq!(error, Err("quota exceeded".to_string()));
    }

    #[test]
    fn openapi_schema_drops_unsupported_keywords() {
        let converted = openapi_schema(&json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": {
                "kind": { "type": ["string", "null"], "enum": ["a", "b"], "description": "d" },
                "n": { "const": 1 }
            },
            "required": ["kind"],
            "additionalProperties": false
        }));
        assert_eq!(
            converted,
            json!({
                "required": ["kind"],
                "type": "object",
                "properties": {
                    "kind": { "description": "d", "type": "string", "nullable": true, "enum": ["a", "b"] },
                    "n": { "enum": [1] }
                }
            })
        );
    }
}
