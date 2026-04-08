mod client;
mod error;
mod types;

pub use client::{ApiAgentClient, ApiAgentClientBuilder};
pub use error::Error;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_status_serialization() {
        let status = AgentStatus::Succeeded;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"succeeded\"");

        let parsed: AgentStatus = serde_json::from_str("\"running\"").unwrap();
        assert_eq!(parsed, AgentStatus::Running);
    }

    #[test]
    fn message_role_serialization() {
        let role = MessageRole::Assistant;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"assistant\"");

        let parsed: MessageRole = serde_json::from_str("\"tool\"").unwrap();
        assert_eq!(parsed, MessageRole::Tool);
    }

    #[test]
    fn create_agent_request_serialization_omits_empty_fields() {
        let value = serde_json::to_value(CreateAgentRequest {
            prompt: "Summarize the latest failures".to_string(),
            model: Some("gpt-5.4".to_string()),
            title: None,
            messages: Some(vec![Message {
                role: MessageRole::User,
                content: "Look at CI".to_string(),
            }]),
            metadata: None,
        })
        .unwrap();

        assert_eq!(value["prompt"], "Summarize the latest failures");
        assert_eq!(value["model"], "gpt-5.4");
        assert_eq!(value["messages"][0]["role"], "user");
        assert!(value.get("title").is_none());
        assert!(value.get("metadata").is_none());
    }
}
