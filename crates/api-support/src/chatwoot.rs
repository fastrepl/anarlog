use std::str::FromStr;

use serde::{Deserialize, Serialize};
use strum::{EnumString, FromRepr};

const CONTENT_SOURCE_AI: &str = "ai";
const ACTION_CABLE_CHANNEL: &str = "RoomChannel";
const DEFAULT_SENDER_NAME: &str = "Agent";

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString)]
enum ActionCableEventType {
    #[strum(serialize = "message.created")]
    MessageCreated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, FromRepr)]
#[repr(i64)]
#[serde(try_from = "i64")]
enum MessageType {
    Incoming = 0,
    Outgoing = 1,
    Activity = 2,
}

impl TryFrom<i64> for MessageType {
    type Error = String;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Self::from_repr(value).ok_or_else(|| format!("unknown message_type: {value}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessage {
    pub content: String,
    pub sender_name: String,
}

#[derive(Deserialize)]
struct ActionCableEnvelope {
    message: Option<ActionCableEvent>,
}

#[derive(Deserialize)]
struct ActionCableEvent {
    event: Option<String>,
    data: Option<MessageData>,
}

#[derive(Deserialize)]
struct MessageData {
    content: Option<String>,
    message_type: Option<MessageType>,
    content_attributes: Option<ContentAttributes>,
    sender: Option<Sender>,
}

#[derive(Deserialize)]
struct ContentAttributes {
    source: Option<String>,
}

#[derive(Deserialize)]
struct Sender {
    name: Option<String>,
}

impl FromStr for AgentMessage {
    type Err = ();

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let envelope: ActionCableEnvelope = serde_json::from_str(raw).map_err(|_| ())?;
        let event = envelope.message.ok_or(())?;

        if event.event.as_deref().and_then(|s| s.parse().ok())
            != Some(ActionCableEventType::MessageCreated)
        {
            return Err(());
        }

        let data = event.data.ok_or(())?;

        if data.message_type != Some(MessageType::Outgoing) {
            return Err(());
        }

        if data
            .content_attributes
            .as_ref()
            .and_then(|a| a.source.as_deref())
            == Some(CONTENT_SOURCE_AI)
        {
            return Err(());
        }

        let content = data.content.filter(|c| !c.is_empty()).ok_or(())?;
        let sender_name = data
            .sender
            .and_then(|s| s.name)
            .unwrap_or_else(|| DEFAULT_SENDER_NAME.to_string());

        Ok(AgentMessage {
            content,
            sender_name,
        })
    }
}

pub fn ws_url(base_url: &str) -> String {
    let ws_base = base_url
        .replace("https://", "wss://")
        .replace("http://", "ws://");
    format!("{}/cable", ws_base)
}

pub fn action_cable_identifier(pubsub_token: &str) -> String {
    serde_json::json!({"channel": ACTION_CABLE_CHANNEL, "pubsub_token": pubsub_token}).to_string()
}

pub fn ai_content_attributes() -> serde_json::Map<String, serde_json::Value> {
    let mut attrs = serde_json::Map::new();
    attrs.insert(
        "source".into(),
        serde_json::Value::String(CONTENT_SOURCE_AI.into()),
    );
    attrs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_url() {
        assert_eq!(
            ws_url("https://chatwoot.example.com"),
            "wss://chatwoot.example.com/cable"
        );
        assert_eq!(ws_url("http://localhost:3000"), "ws://localhost:3000/cable");
    }

    #[test]
    fn test_action_cable_identifier() {
        let id = action_cable_identifier("test-token");
        let parsed: serde_json::Value = serde_json::from_str(&id).unwrap();
        assert_eq!(parsed["channel"], ACTION_CABLE_CHANNEL);
        assert_eq!(parsed["pubsub_token"], "test-token");
    }

    fn parse(raw: &str) -> Option<AgentMessage> {
        raw.parse().ok()
    }

    #[test]
    fn test_parse_human_agent_message() {
        let raw = serde_json::json!({
            "identifier": "...",
            "message": {
                "event": "message.created",
                "data": {
                    "content": "Hello from support",
                    "message_type": 1,
                    "content_attributes": {},
                    "sender": { "name": "Alice" }
                }
            }
        });

        assert_eq!(
            parse(&raw.to_string()),
            Some(AgentMessage {
                content: "Hello from support".into(),
                sender_name: "Alice".into(),
            })
        );
    }

    #[test]
    fn test_parse_skips_ai_messages() {
        let raw = serde_json::json!({
            "identifier": "...",
            "message": {
                "event": "message.created",
                "data": {
                    "content": "AI response",
                    "message_type": 1,
                    "content_attributes": { "source": "ai" },
                    "sender": { "name": "Bot" }
                }
            }
        });

        assert_eq!(parse(&raw.to_string()), None);
    }

    #[test]
    fn test_parse_skips_incoming_messages() {
        let raw = serde_json::json!({
            "identifier": "...",
            "message": {
                "event": "message.created",
                "data": {
                    "content": "User message",
                    "message_type": 0,
                    "content_attributes": {},
                    "sender": { "name": "User" }
                }
            }
        });

        assert_eq!(parse(&raw.to_string()), None);
    }

    #[test]
    fn test_parse_skips_non_message_events() {
        let raw = serde_json::json!({
            "identifier": "...",
            "message": {
                "event": "conversation.typing_on",
                "data": {}
            }
        });

        assert_eq!(parse(&raw.to_string()), None);
    }

    #[test]
    fn test_parse_skips_empty_content() {
        let raw = serde_json::json!({
            "identifier": "...",
            "message": {
                "event": "message.created",
                "data": {
                    "content": "",
                    "message_type": 1,
                    "content_attributes": {},
                    "sender": { "name": "Agent" }
                }
            }
        });

        assert_eq!(parse(&raw.to_string()), None);
    }

    #[test]
    fn test_parse_skips_actioncable_pings() {
        assert_eq!(parse(r#"{"type":"ping","message":1234567890}"#), None);
    }

    #[test]
    fn test_parse_skips_welcome_message() {
        assert_eq!(parse(r#"{"type":"welcome"}"#), None);
    }

    #[test]
    fn test_parse_defaults_sender_name() {
        let raw = serde_json::json!({
            "identifier": "...",
            "message": {
                "event": "message.created",
                "data": {
                    "content": "Hello",
                    "message_type": 1,
                    "content_attributes": {}
                }
            }
        });

        let result = parse(&raw.to_string()).unwrap();
        assert_eq!(result.sender_name, "Agent");
    }
}
