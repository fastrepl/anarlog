use futures_util::StreamExt;
use rig::agent::{Agent, MultiTurnStreamItem, StreamingError};
use rig::client::CompletionClient;
use rig::message::Message;
use rig::providers::{anthropic, openai, openrouter};
use rig::streaming::{StreamedAssistantContent, StreamedUserContent, StreamingChat};

use crate::error::{CliError, CliResult};
use crate::llm::{LlmProvider, ResolvedLlmConfig};

use super::tools;

pub enum StreamEvent {
    TextChunk(String),
    ToolCallStart {
        tool_name: String,
        arguments: String,
    },
    ToolResult {
        success: bool,
    },
}

macro_rules! build_agent {
    ($client_type:path, $config:expr, $system_message:expr, $provider_name:expr) => {{
        let mut builder = <$client_type>::builder().api_key($config.api_key.as_str());
        if !$config.base_url.is_empty() {
            builder = builder.base_url(&$config.base_url);
        }
        let client = builder.build().map_err(|e| {
            CliError::operation_failed(concat!("build ", $provider_name, " client"), e.to_string())
        })?;
        let mut agent = client.agent($config.model);
        if let Some(msg) = $system_message.as_deref() {
            agent = agent.preamble(msg);
        }
        agent.tool(tools::Char).build()
    }};
}

#[derive(Clone)]
pub enum Backend {
    Anthropic(Agent<anthropic::completion::CompletionModel>),
    Openai(Agent<openai::CompletionModel>),
    Openrouter(Agent<openrouter::CompletionModel>),
}

impl Backend {
    pub fn new(config: ResolvedLlmConfig, system_message: Option<String>) -> CliResult<Self> {
        match config.provider {
            LlmProvider::Anthropic => Ok(Self::Anthropic(build_agent!(
                anthropic::Client,
                config,
                system_message,
                "anthropic"
            ))),
            LlmProvider::Openai => Ok(Self::Openai(build_agent!(
                openai::CompletionsClient,
                config,
                system_message,
                "openai"
            ))),
            LlmProvider::Openrouter => Ok(Self::Openrouter(build_agent!(
                openrouter::Client,
                config,
                system_message,
                "openrouter"
            ))),
        }
    }

    pub async fn stream_text<F>(
        &self,
        prompt: String,
        history: Vec<Message>,
        max_turns: usize,
        mut on_event: F,
    ) -> CliResult<Option<String>>
    where
        F: FnMut(StreamEvent) -> CliResult<()>,
    {
        macro_rules! do_stream {
            ($agent:expr) => {{
                let stream = $agent
                    .stream_chat(prompt, history)
                    .multi_turn(max_turns)
                    .await;
                process_stream(stream, &mut on_event).await
            }};
        }

        match self {
            Self::Anthropic(agent) => do_stream!(agent),
            Self::Openai(agent) => do_stream!(agent),
            Self::Openrouter(agent) => do_stream!(agent),
        }
    }
}

async fn process_stream<S, R, F>(mut stream: S, on_event: &mut F) -> CliResult<Option<String>>
where
    S: StreamExt<Item = Result<MultiTurnStreamItem<R>, StreamingError>> + Unpin,
    F: FnMut(StreamEvent) -> CliResult<()>,
{
    let mut accumulated = String::new();
    let mut final_response = None;
    while let Some(item) = stream.next().await {
        match item {
            Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(text))) => {
                accumulated.push_str(&text.text);
                on_event(StreamEvent::TextChunk(text.text))?;
            }
            Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::ToolCall {
                tool_call,
                ..
            })) => {
                on_event(StreamEvent::ToolCallStart {
                    tool_name: tool_call.function.name.clone(),
                    arguments: tool_call.function.arguments.to_string(),
                })?;
            }
            Ok(MultiTurnStreamItem::StreamUserItem(StreamedUserContent::ToolResult { .. })) => {
                on_event(StreamEvent::ToolResult { success: true })?;
            }
            Ok(MultiTurnStreamItem::FinalResponse(response)) => {
                final_response = Some(response);
            }
            Ok(_) => {}
            Err(error) => {
                return Err(CliError::operation_failed("chat stream", error.to_string()));
            }
        }
    }
    if let Some(response) = final_response
        && !response.response().is_empty()
    {
        let final_text = response.response();
        if accumulated.is_empty() {
            on_event(StreamEvent::TextChunk(final_text.to_string()))?;
        } else if final_text.starts_with(&accumulated) && final_text.len() > accumulated.len() {
            on_event(StreamEvent::TextChunk(
                final_text[accumulated.len()..].to_string(),
            ))?;
        }
        return Ok(Some(final_text.to_string()));
    }
    Ok((!accumulated.is_empty()).then_some(accumulated))
}
