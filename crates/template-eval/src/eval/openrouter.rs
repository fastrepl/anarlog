use std::collections::HashMap;

use hypr_openrouter::{
    ChatCompletionRequest, ChatMessage, Client as OpenRouterClient, ResponseFormat, Role,
};

use crate::eval::types::{CaseResponseFormat, EvalCase, EvalMessage};

pub(crate) fn run_openrouter(
    runtime: &tokio::runtime::Runtime,
    client: &OpenRouterClient,
    model: &str,
    case: &EvalCase,
    sample_index: usize,
) -> Result<String, String> {
    let messages: Vec<ChatMessage> = case
        .messages
        .iter()
        .map(case_message_to_openrouter)
        .collect();
    let request = ChatCompletionRequest {
        model: Some(model.to_string()),
        messages,
        max_tokens: Some(case.max_tokens),
        temperature: Some(0.0),
        response_format: case.response_format.map(case_response_format_to_openrouter),
        metadata: Some(HashMap::from([
            ("case_id".to_string(), case.name.clone()),
            ("sample_index".to_string(), sample_index.to_string()),
        ])),
        ..Default::default()
    };

    let response = runtime
        .block_on(client.chat_completion(&request))
        .map_err(|err| format!("OpenRouter request failed for {}: {err}", case.name))?;

    response
        .choices
        .first()
        .and_then(|choice| choice.message.content.as_ref())
        .and_then(|content| content.as_text())
        .map(str::to_string)
        .ok_or_else(|| {
            format!(
                "OpenRouter returned no text content for {} sample {}",
                case.name,
                sample_index + 1
            )
        })
}

fn case_message_to_openrouter(message: &EvalMessage) -> ChatMessage {
    ChatMessage::new(
        case_role_to_openrouter(&message.role),
        message.content.clone(),
    )
}

fn case_role_to_openrouter(role: &str) -> Role {
    match role {
        "system" => Role::System,
        "user" => Role::User,
        "assistant" => Role::Assistant,
        "developer" => Role::Developer,
        "tool" => Role::Tool,
        other => panic!("unsupported chat role: {other}"),
    }
}

fn case_response_format_to_openrouter(response_format: CaseResponseFormat) -> ResponseFormat {
    match response_format {
        CaseResponseFormat::JsonObject => ResponseFormat::json_object(),
    }
}
