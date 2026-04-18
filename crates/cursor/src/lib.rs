mod client;
mod error;
mod types;

pub use client::{CursorClient, CursorClientBuilder};
pub use error::Error;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_status_serialization() {
        let status = AgentStatus::Finished;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"FINISHED\"");

        let parsed: AgentStatus = serde_json::from_str("\"RUNNING\"").unwrap();
        assert_eq!(parsed, AgentStatus::Running);
    }

    #[test]
    fn agent_status_deserialization_accepts_unknown_variants() {
        let parsed: AgentStatus = serde_json::from_str("\"PAUSED\"").unwrap();
        assert_eq!(parsed, AgentStatus::Unknown);
    }

    #[test]
    fn conversation_message_type_deserialization_accepts_unknown_variants() {
        let parsed: ConversationMessageType = serde_json::from_str("\"tool_message\"").unwrap();
        assert_eq!(parsed, ConversationMessageType::Unknown);
    }

    #[test]
    fn launch_agent_request_serialization_omits_empty_fields() {
        let value = serde_json::to_value(LaunchAgentRequest {
            prompt: PromptInput {
                text: "Add a README".to_string(),
                images: None,
            },
            model: None,
            source: AgentSourceInput {
                repository: Some("https://github.com/acme/repo".to_string()),
                r#ref: Some("main".to_string()),
                pr_url: None,
            },
            target: Some(AgentTargetInput {
                auto_create_pr: Some(true),
                open_as_cursor_github_app: None,
                skip_reviewer_request: None,
                branch_name: Some("feature/add-readme".to_string()),
                auto_branch: None,
            }),
            webhook: None,
        })
        .unwrap();

        assert_eq!(value["prompt"]["text"], "Add a README");
        assert_eq!(
            value["source"]["repository"],
            "https://github.com/acme/repo"
        );
        assert_eq!(value["target"]["autoCreatePr"], true);
        assert!(value.get("model").is_none());
        assert!(value["target"].get("openAsCursorGithubApp").is_none());
        assert!(value["target"].get("url").is_none());
        assert!(value["target"].get("prUrl").is_none());
        assert!(value["source"].get("prUrl").is_none());
    }
}
