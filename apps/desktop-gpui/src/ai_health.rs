//! `useConnectionHealth` (`llm/health.tsx`): a one-message generation against
//! the selected model (`If user says hi, respond with hello` / `Hi`), retried
//! five times 200ms apart, whose outcome drives the combobox check and the
//! `Connection failed: …` alert.

use std::time::Duration;

use serde_json::{Value, json};

const SYSTEM_PROMPT: &str = "If user says hi, respond with hello, without any other text.";
const RETRIES: usize = 5;
const RETRY_DELAY: Duration = Duration::from_millis(200);
const TIMEOUT: Duration = Duration::from_secs(20);

/// `LlmHealthStatus`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Health {
    Pending,
    Success,
    Error(String),
}

/// One request shaped for the provider's chat API.
struct Probe {
    url: String,
    headers: Vec<(&'static str, String)>,
    body: Value,
}

/// `createProviderModel`: the request each provider family answers.
/// `None` for the subscription / hosted providers the shell cannot reach.
fn probe(provider_id: &str, base_url: &str, api_key: &str, model: &str) -> Option<Probe> {
    let base = base_url.trim_end_matches('/');
    if base.is_empty() {
        return None;
    }
    let openai_body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content": "Hi" }
        ]
    });
    Some(match provider_id {
        "anarlog" | "claude" | "chatgpt" | "grok" | "github_copilot" | "apple_foundation" => {
            return None;
        }
        "anthropic" => Probe {
            url: format!("{base}/messages"),
            headers: vec![
                ("x-api-key", api_key.to_string()),
                ("anthropic-version", "2023-06-01".to_string()),
                (
                    "anthropic-dangerous-direct-browser-access",
                    "true".to_string(),
                ),
            ],
            body: json!({
                "model": model,
                "max_tokens": 64,
                "system": SYSTEM_PROMPT,
                "messages": [{ "role": "user", "content": "Hi" }]
            }),
        },
        "google_generative_ai" => Probe {
            url: format!("{base}/models/{model}:generateContent"),
            headers: vec![("x-goog-api-key", api_key.to_string())],
            body: json!({
                "systemInstruction": { "parts": [{ "text": SYSTEM_PROMPT }] },
                "contents": [{ "role": "user", "parts": [{ "text": "Hi" }] }]
            }),
        },
        // `createOpenAI` / `createAzure` answer through the Responses API, the
        // system prompt a `developer` item for the reasoning models.
        "openai" => Probe {
            url: format!("{base}/responses"),
            headers: vec![("Authorization", format!("Bearer {api_key}"))],
            body: responses_body(model),
        },
        "azure_openai" => Probe {
            url: format!("{base}/v1/responses?api-version=v1"),
            headers: vec![("api-key", api_key.to_string())],
            body: responses_body(model),
        },
        "azure_ai" => Probe {
            url: format!("{base}/chat/completions"),
            headers: vec![
                ("api-key", api_key.to_string()),
                ("Authorization", format!("Bearer {api_key}")),
            ],
            body: openai_body,
        },
        "ollama" => Probe {
            url: format!("{base}/chat/completions"),
            headers: Vec::new(),
            body: openai_body,
        },
        _ => Probe {
            url: format!("{base}/chat/completions"),
            headers: if api_key.is_empty() {
                Vec::new()
            } else {
                vec![("Authorization", format!("Bearer {api_key}"))]
            },
            body: openai_body,
        },
    })
}

/// The Responses request `generateText` sends: `input` items only.
fn responses_body(model: &str) -> Value {
    let role = if crate::llm_stream::openai_is_reasoning_model(model) {
        "developer"
    } else {
        "system"
    };
    json!({
        "model": model,
        "input": [
            { "role": role, "content": SYSTEM_PROMPT },
            { "role": "user", "content": [{ "type": "input_text", "text": "Hi" }] }
        ]
    })
}

/// Whether the shell can probe this provider at all.
pub fn can_probe(provider_id: &str, base_url: &str) -> bool {
    probe(provider_id, base_url, "", "m").is_some()
}

async fn attempt(client: &reqwest::Client, probe: &Probe) -> Result<(), String> {
    let mut request = client.post(&probe.url).json(&probe.body);
    for (name, value) in &probe.headers {
        request = request.header(*name, value);
    }
    let response = request
        .send()
        .await
        .map_err(|error| first_useful_line(&error.to_string()))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(llm_health_error_message(status.as_u16(), &body));
    }
    Ok(())
}

/// `useQuery({ retry: 5, retryDelay: 200 })` around `generateText`.
pub async fn check(provider_id: &str, base_url: &str, api_key: &str, model: &str) -> Health {
    let Some(probe) = probe(provider_id, base_url, api_key, model) else {
        return Health::Success;
    };
    let client = match reqwest::Client::builder().timeout(TIMEOUT).build() {
        Ok(client) => client,
        Err(error) => return Health::Error(format!("Connection failed: {error}")),
    };
    let mut last = String::new();
    for attempt_index in 0..=RETRIES {
        match attempt(&client, &probe).await {
            Ok(()) => return Health::Success,
            Err(message) => last = message,
        }
        if attempt_index < RETRIES {
            tokio::time::sleep(RETRY_DELAY).await;
        }
    }
    Health::Error(format!("Connection failed: {last}"))
}

/// `llmHealthErrorMessage`: the API's own message when the body carries one
/// (`message`, `detail`, `error` or `error.message`), else the first useful
/// line of the body, else the status.
pub fn llm_health_error_message(status: u16, body: &str) -> String {
    let payload = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| api_error_from_value(&value));
    if let Some(message) = payload {
        return message;
    }
    let line = first_useful_line(body);
    if !line.is_empty() {
        return line;
    }
    if status > 0 {
        return format!("HTTP {status}");
    }
    "Unknown error".to_string()
}

fn api_error_from_value(value: &Value) -> Option<String> {
    let record = value.as_object()?;
    for key in ["message", "detail"] {
        if let Some(text) = record.get(key).and_then(Value::as_str)
            && !text.trim().is_empty()
        {
            return Some(first_useful_line(text));
        }
    }
    match record.get("error") {
        Some(Value::String(text)) if !text.trim().is_empty() => Some(first_useful_line(text)),
        Some(Value::Object(nested)) => nested
            .get("message")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .map(first_useful_line),
        _ => None,
    }
}

/// `firstUsefulLine`: the first non-blank line, cut to 200 characters.
pub fn first_useful_line(value: &str) -> String {
    let line = value
        .lines()
        .map(str::trim)
        .find(|part| !part.is_empty())
        .unwrap_or_else(|| value.trim());
    let count = line.chars().count();
    if count > 200 {
        format!("{}...", line.chars().take(197).collect::<String>())
    } else {
        line.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_prefer_the_api_payload() {
        assert_eq!(
            llm_health_error_message(401, r#"{"error":{"message":"invalid api key"}}"#),
            "invalid api key"
        );
        assert_eq!(
            llm_health_error_message(400, r#"{"error":"bad request"}"#),
            "bad request"
        );
        assert_eq!(
            llm_health_error_message(422, r#"{"detail":"model not found"}"#),
            "model not found"
        );
        assert_eq!(
            llm_health_error_message(500, "\n\n  Internal Server Error\nmore"),
            "Internal Server Error"
        );
        assert_eq!(llm_health_error_message(502, ""), "HTTP 502");
        let long = "x".repeat(250);
        assert_eq!(first_useful_line(&long).chars().count(), 200);
    }

    #[test]
    fn probes_follow_the_provider_family() {
        assert!(can_probe("custom", "http://localhost:1234/v1"));
        assert!(can_probe("anthropic", "https://api.anthropic.com/v1"));
        assert!(!can_probe("anarlog", "https://api.example"));
        assert!(!can_probe("custom", ""));
        let openai = probe("openai", "https://api.openai.com/v1/", "k", "gpt-5.6").unwrap();
        assert_eq!(openai.url, "https://api.openai.com/v1/responses");
        assert_eq!(
            openai.headers,
            vec![("Authorization", "Bearer k".to_string())]
        );
        assert_eq!(openai.body["input"][0]["role"], "developer");
        assert_eq!(
            openai.body["input"][1]["content"][0],
            json!({ "type": "input_text", "text": "Hi" })
        );
        let chat = probe("openai", "https://api.openai.com/v1/", "k", "gpt-4o").unwrap();
        assert_eq!(chat.body["input"][0]["role"], "system");
        let compatible = probe("groq", "https://api.groq.com/openai/v1", "k", "m").unwrap();
        assert_eq!(
            compatible.url,
            "https://api.groq.com/openai/v1/chat/completions"
        );
        let google = probe(
            "google_generative_ai",
            "https://g/v1beta",
            "k",
            "gemini-3.8-flash",
        )
        .unwrap();
        assert_eq!(
            google.url,
            "https://g/v1beta/models/gemini-3.8-flash:generateContent"
        );
        let azure = probe(
            "azure_openai",
            "https://a.openai.azure.com/openai",
            "k",
            "dep",
        )
        .unwrap();
        assert_eq!(
            azure.url,
            "https://a.openai.azure.com/openai/v1/responses?api-version=v1"
        );
        assert_eq!(azure.headers, vec![("api-key", "k".to_string())]);
        let ollama = probe("ollama", "http://localhost:11434/v1", "", "llama").unwrap();
        assert!(ollama.headers.is_empty());
    }
}
