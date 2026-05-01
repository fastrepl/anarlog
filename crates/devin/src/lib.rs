mod client;
mod error;
mod types;

pub use client::{DevinClient, DevinClientBuilder};
pub use error::Error;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path, query_param},
    };

    #[test]
    fn session_status_serialization() {
        let status = SessionStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"running\"");

        let parsed: SessionStatus = serde_json::from_str("\"suspended\"").unwrap();
        assert_eq!(parsed, SessionStatus::Suspended);
    }

    #[test]
    fn session_status_unknown_variant_is_forward_compatible() {
        let parsed: SessionStatus = serde_json::from_str("\"brand_new_state\"").unwrap();
        assert_eq!(parsed, SessionStatus::Unknown);
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
    fn session_origin_as_str_matches_wire_format() {
        for origin in [
            SessionOrigin::Webapp,
            SessionOrigin::Slack,
            SessionOrigin::Teams,
            SessionOrigin::Api,
            SessionOrigin::Linear,
            SessionOrigin::Jira,
            SessionOrigin::Scheduled,
            SessionOrigin::Cli,
            SessionOrigin::Other,
        ] {
            let wire = serde_json::to_string(&origin).unwrap();
            assert_eq!(wire.trim_matches('"'), origin.as_str());
        }
    }

    #[test]
    fn session_message_source_parses_known_values() {
        let devin: SessionMessageSource = serde_json::from_str("\"devin\"").unwrap();
        assert_eq!(devin, SessionMessageSource::Devin);

        let user: SessionMessageSource = serde_json::from_str("\"user\"").unwrap();
        assert_eq!(user, SessionMessageSource::User);

        let other: SessionMessageSource = serde_json::from_str("\"system\"").unwrap();
        assert_eq!(other, SessionMessageSource::Unknown);
    }

    #[test]
    fn session_deserializes_with_null_pr_state() {
        let payload = json!({
            "acus_consumed": 0.0,
            "created_at": 0,
            "org_id": "org_1",
            "pull_requests": [
                { "pr_state": null, "pr_url": "https://github.com/a/b/pull/1" }
            ],
            "session_id": "devin-1",
            "status": "running",
            "tags": [],
            "updated_at": 0,
            "url": "https://app.devin.ai/sessions/devin-1"
        });

        let session: Session = serde_json::from_value(payload).unwrap();
        assert_eq!(session.pull_requests.len(), 1);
        assert!(session.pull_requests[0].pr_state.is_none());
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

    fn empty_page() -> serde_json::Value {
        json!({
            "items": [],
            "end_cursor": null,
            "has_next_page": false,
            "total": 0,
        })
    }

    async fn client_for(server: &MockServer) -> DevinClient {
        DevinClient::builder()
            .api_key("sk_test_secret")
            .api_base(server.uri())
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn list_sessions_sends_bearer_auth_and_expected_query_params() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v3/organizations/org_1/sessions"))
            .and(header("authorization", "Bearer sk_test_secret"))
            .and(query_param("first", "25"))
            .and(query_param("tags", "bug"))
            .and(query_param("origins", "api"))
            .and(query_param("origins", "cli"))
            .and(query_param("user_ids", "u_1"))
            .and(query_param("user_ids", "u_2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(empty_page()))
            .expect(1)
            .mount(&server)
            .await;

        let client = client_for(&server).await;
        client
            .list_sessions(
                "org_1",
                ListSessionsRequest {
                    first: Some(25),
                    tags: Some(vec!["bug".to_string()]),
                    origins: Some(vec![SessionOrigin::Api, SessionOrigin::Cli]),
                    user_ids: Some(vec!["u_1".to_string(), "u_2".to_string()]),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn terminate_session_with_archive_sends_query_param() {
        let server = MockServer::start().await;

        Mock::given(method("DELETE"))
            .and(path("/v3/organizations/org_1/sessions/devin-1"))
            .and(query_param("archive", "true"))
            .and(header("authorization", "Bearer sk_test_secret"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "acus_consumed": 0.0,
                "created_at": 0,
                "org_id": "org_1",
                "pull_requests": [],
                "session_id": "devin-1",
                "status": "exit",
                "tags": [],
                "updated_at": 0,
                "url": "https://app.devin.ai/sessions/devin-1",
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = client_for(&server).await;
        let session = client
            .terminate_session("org_1", "devin-1", true)
            .await
            .unwrap();
        assert_eq!(session.status, SessionStatus::Exit);
    }

    #[tokio::test]
    async fn custom_http_client_still_receives_auth_header() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v3/organizations/org_1/sessions/devin-1"))
            .and(header("authorization", "Bearer sk_test_secret"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "acus_consumed": 0.0,
                "created_at": 0,
                "org_id": "org_1",
                "pull_requests": [],
                "session_id": "devin-1",
                "status": "running",
                "tags": [],
                "updated_at": 0,
                "url": "https://app.devin.ai/sessions/devin-1",
            })))
            .expect(1)
            .mount(&server)
            .await;

        let custom = reqwest::Client::builder().build().unwrap();
        let client = DevinClient::builder()
            .api_key("sk_test_secret")
            .api_base(server.uri())
            .http_client(custom)
            .build()
            .unwrap();

        client.get_session("org_1", "devin-1").await.unwrap();
    }

    #[tokio::test]
    async fn api_base_with_path_prefix_is_preserved() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/devin/v3/organizations/org_1/sessions/devin-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "acus_consumed": 0.0,
                "created_at": 0,
                "org_id": "org_1",
                "pull_requests": [],
                "session_id": "devin-1",
                "status": "running",
                "tags": [],
                "updated_at": 0,
                "url": "https://app.devin.ai/sessions/devin-1",
            })))
            .expect(1)
            .mount(&server)
            .await;

        let base = format!("{}/devin", server.uri());
        let client = DevinClient::builder()
            .api_key("sk_test_secret")
            .api_base(base)
            .build()
            .unwrap();

        client.get_session("org_1", "devin-1").await.unwrap();
    }
}
