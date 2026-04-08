use std::pin::Pin;

use futures_util::Stream;
use serde::{Deserialize, Serialize};

use crate::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub total_cost_usd: Option<f64>,
    pub duration_ms: Option<u64>,
    pub duration_api_ms: Option<u64>,
    pub num_turns: Option<u64>,
    pub raw: serde_json::Value,
}

impl Usage {
    pub(crate) fn from_value(value: &serde_json::Value) -> Option<Self> {
        let total_cost_usd = value
            .get("total_cost_usd")
            .and_then(serde_json::Value::as_f64);
        let duration_ms = value.get("duration_ms").and_then(serde_json::Value::as_u64);
        let duration_api_ms = value
            .get("duration_api_ms")
            .and_then(serde_json::Value::as_u64);
        let num_turns = value.get("num_turns").and_then(serde_json::Value::as_u64);

        if total_cost_usd.is_none()
            && duration_ms.is_none()
            && duration_api_ms.is_none()
            && num_turns.is_none()
        {
            return None;
        }

        Some(Self {
            total_cost_usd,
            duration_ms,
            duration_api_ms,
            num_turns,
            raw: value.clone(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeEvent {
    pub event_type: Option<String>,
    pub session_id: Option<String>,
    pub raw: serde_json::Value,
}

impl ClaudeEvent {
    pub fn from_value(raw: serde_json::Value) -> Self {
        Self {
            event_type: raw
                .get("type")
                .and_then(serde_json::Value::as_str)
                .or_else(|| raw.get("event_type").and_then(serde_json::Value::as_str))
                .map(ToOwned::to_owned),
            session_id: session_id_from_value(&raw),
            raw,
        }
    }

    pub fn is_result(&self) -> bool {
        self.event_type.as_deref() == Some("result")
    }

    pub fn is_hook_event(&self) -> bool {
        self.event_type
            .as_deref()
            .is_some_and(|event_type| event_type.to_lowercase().contains("hook"))
    }

    pub fn error_message(&self) -> Option<String> {
        if !self
            .raw
            .get("is_error")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
            && !self
                .event_type
                .as_deref()
                .is_some_and(|event_type| event_type.to_lowercase().contains("error"))
        {
            return None;
        }

        string_from_key_path(&self.raw, &["error", "message"])
            .or_else(|| string_from_key_path(&self.raw, &["message"]))
            .or_else(|| string_from_key_path(&self.raw, &["result"]))
    }

    pub fn partial_assistant_text(&self) -> Option<String> {
        let Some(event_type) = self.event_type.as_deref() else {
            return None;
        };

        let event_type = event_type.to_lowercase();
        if !(event_type.contains("assistant")
            || event_type.contains("message")
            || event_type.contains("partial")
            || event_type.contains("delta"))
        {
            return None;
        }

        string_from_key_path(&self.raw, &["text"]).or_else(|| first_content_text(&self.raw))
    }
}

#[derive(Debug, Clone)]
pub struct Turn {
    pub events: Vec<ClaudeEvent>,
    pub final_response: Option<String>,
    pub session_id: Option<String>,
    pub result: serde_json::Value,
    pub usage: Option<Usage>,
}

pub type EventStream = Pin<Box<dyn Stream<Item = Result<ClaudeEvent, Error>> + Send>>;

pub struct RunStreamedResult {
    pub events: EventStream,
}

pub(crate) fn session_id_from_value(value: &serde_json::Value) -> Option<String> {
    string_from_key_path(value, &["session_id"])
        .or_else(|| string_from_key_path(value, &["sessionId"]))
}

pub(crate) fn final_response_from_value(value: &serde_json::Value) -> Option<String> {
    value
        .get("result")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| first_content_text(value))
        .or_else(|| string_from_key_path(value, &["message", "content", "text"]))
        .or_else(|| string_from_key_path(value, &["text"]))
}

fn string_from_key_path(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str().map(ToOwned::to_owned)
}

fn first_content_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Array(items) => items.iter().find_map(first_content_text),
        serde_json::Value::Object(map) => {
            if map.get("type").and_then(serde_json::Value::as_str) == Some("text") {
                if let Some(text) = map.get("text").and_then(serde_json::Value::as_str) {
                    return Some(text.to_string());
                }
            }

            if let Some(items) = map.get("content").and_then(serde_json::Value::as_array) {
                if let Some(text) = items.iter().find_map(first_content_text) {
                    return Some(text);
                }
            }

            map.values().find_map(first_content_text)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ClaudeEvent, Usage, final_response_from_value, session_id_from_value};

    #[test]
    fn extracts_session_id_and_result_text() {
        let payload = json!({
            "type": "result",
            "result": "done",
            "session_id": "session-123"
        });

        assert_eq!(
            session_id_from_value(&payload).as_deref(),
            Some("session-123")
        );
        assert_eq!(final_response_from_value(&payload).as_deref(), Some("done"));
    }

    #[test]
    fn extracts_partial_assistant_text_from_content() {
        let event = ClaudeEvent::from_value(json!({
            "type": "assistant",
            "message": {
                "content": [{ "type": "text", "text": "partial" }]
            }
        }));

        assert_eq!(event.partial_assistant_text().as_deref(), Some("partial"));
    }

    #[test]
    fn usage_is_present_for_result_payloads() {
        let payload = json!({
            "type": "result",
            "total_cost_usd": 0.003,
            "duration_ms": 1234,
            "duration_api_ms": 800,
            "num_turns": 6
        });

        let usage = Usage::from_value(&payload).expect("usage");
        assert_eq!(usage.duration_ms, Some(1234));
        assert_eq!(usage.num_turns, Some(6));
    }
}
