mod client;
mod error;
mod types;

pub use client::{DevinClient, DevinClientBuilder};
pub use error::Error;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_status_serialization() {
        let status = SessionStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"running\"");

        let parsed: SessionStatus = serde_json::from_str("\"suspended\"").unwrap();
        assert_eq!(parsed, SessionStatus::Suspended);
    }

    #[test]
    fn session_origin_serialization() {
        let origin = SessionOrigin::Webapp;
        let json = serde_json::to_string(&origin).unwrap();
        assert_eq!(json, "\"webapp\"");

        let parsed: SessionOrigin = serde_json::from_str("\"cli\"").unwrap();
        assert_eq!(parsed, SessionOrigin::Cli);
    }

    #[test]
    fn list_sessions_request_serialization_omits_empty_fields() {
        let value = serde_json::to_value(ListSessionsRequest {
            first: Some(50),
            tags: Some(vec!["bug".to_string(), "urgent".to_string()]),
            origins: Some(vec![SessionOrigin::Api, SessionOrigin::Cli]),
            ..Default::default()
        })
        .unwrap();

        assert_eq!(value["first"], 50);
        assert_eq!(value["tags"][0], "bug");
        assert_eq!(value["origins"][1], "cli");
        assert!(value.get("after").is_none());
    }
}
