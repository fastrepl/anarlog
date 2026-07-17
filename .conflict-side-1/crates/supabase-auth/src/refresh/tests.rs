use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use tokio::sync::Barrier;
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

use super::{AuthClient, Error};

fn session_payload() -> Value {
    json!({
        "access_token": "access-2",
        "refresh_token": "refresh-2",
        "token_type": "bearer",
        "expires_in": 3600,
        "expires_at": 1_800_000_000u64,
        "user": {
            "id": "user-123",
            "email": "user@example.com",
            "user_metadata": {
                "full_name": "Test User"
            }
        }
    })
}

fn test_client(server: &MockServer) -> AuthClient {
    AuthClient::new(server.uri(), "anon-key")
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[tokio::test]
async fn refresh_session_returns_rotated_session() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .and(body_json(json!({ "refresh_token": "refresh-1" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(session_payload()))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let session = client.refresh_session("refresh-1").await.unwrap();

    assert_eq!(session.access_token, "access-2");
    assert_eq!(session.refresh_token.as_deref(), Some("refresh-2"));
    assert_eq!(session.user.as_ref().unwrap().id, "user-123");
}

#[tokio::test]
async fn refresh_session_surfaces_auth_errors() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "msg": "Invalid Refresh Token: Already Used"
        })))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let err = client.refresh_session("refresh-1").await.unwrap_err();

    match err {
        Error::Api {
            status,
            code,
            message,
        } => {
            assert_eq!(status, 401);
            assert_eq!(code, None);
            assert!(message.contains("Invalid Refresh Token"));
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[tokio::test]
async fn refresh_session_rejects_invalid_payloads() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "missing-user"
        })))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let err = client.refresh_session("refresh-1").await.unwrap_err();
    assert!(matches!(err, Error::InvalidSession(_)));
}

#[tokio::test]
async fn refresh_session_rejects_empty_access_token() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "",
            "refresh_token": "refresh-2",
            "token_type": "bearer",
            "expires_in": 3600
        })))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let err = client.refresh_session("refresh-1").await.unwrap_err();

    assert!(matches!(err, Error::InvalidSession(_)));
}

#[tokio::test]
async fn refresh_session_rejects_zero_expires_in() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "access-2",
            "refresh_token": "refresh-2",
            "token_type": "bearer",
            "expires_in": 0
        })))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let err = client.refresh_session("refresh-1").await.unwrap_err();

    assert!(matches!(err, Error::InvalidSession(_)));
}

#[tokio::test]
async fn refresh_session_extracts_error_description() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": "invalid_grant",
            "error_description": "Token has been revoked"
        })))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let err = client.refresh_session("refresh-1").await.unwrap_err();

    match err {
        Error::Api {
            status,
            code,
            message,
        } => {
            assert_eq!(status, 400);
            assert_eq!(code, None);
            assert_eq!(message, "Token has been revoked");
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[tokio::test]
async fn refresh_session_falls_back_to_raw_body_for_non_json_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .respond_with(ResponseTemplate::new(500).set_body_string("upstream exploded"))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let err = client.refresh_session("refresh-1").await.unwrap_err();

    match err {
        Error::Api {
            status,
            code,
            message,
        } => {
            assert_eq!(status, 500);
            assert_eq!(code, None);
            assert_eq!(message, "upstream exploded");
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[tokio::test]
async fn refresh_session_rejects_empty_refresh_token() {
    let client = AuthClient::new("http://unused", "anon-key");
    let err = client.refresh_session("").await.unwrap_err();
    assert!(matches!(err, Error::SessionMissing));
    assert!(err.is_fatal());
}

#[tokio::test]
async fn refresh_session_handles_trailing_slash_base_url() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .and(body_json(json!({ "refresh_token": "refresh-1" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "access-2",
            "refresh_token": "refresh-2",
            "token_type": "bearer",
            "expires_in": 3600,
            "user": { "id": "user-123", "email": null }
        })))
        .mount(&server)
        .await;

    let client = AuthClient::new(format!("{}/", server.uri()), "anon-key");
    let session = client.refresh_session("refresh-1").await.unwrap();

    assert_eq!(session.access_token, "access-2");
}

#[tokio::test]
async fn retries_503_then_succeeds() {
    let server = MockServer::start().await;
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_ref = attempts.clone();

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .respond_with(move |_request: &Request| {
            let attempt = attempts_ref.fetch_add(1, Ordering::SeqCst);
            if attempt < 2 {
                return ResponseTemplate::new(503).set_body_json(json!({
                    "msg": "temporarily unavailable"
                }));
            }

            ResponseTemplate::new(200).set_body_json(session_payload())
        })
        .mount(&server)
        .await;

    let client = test_client(&server);
    let session = client.refresh_session("refresh-1").await.unwrap();

    assert_eq!(session.refresh_token.as_deref(), Some("refresh-2"));
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
}

#[tokio::test]
async fn concurrent_refresh_callers_share_one_request() {
    let server = MockServer::start().await;
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_ref = attempts.clone();

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .respond_with(move |_request: &Request| {
            attempts_ref.fetch_add(1, Ordering::SeqCst);
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(100))
                .set_body_json(session_payload())
        })
        .mount(&server)
        .await;

    let client = test_client(&server);
    let barrier = Arc::new(Barrier::new(3));
    let refresh_one = {
        let barrier = barrier.clone();
        let client = client.clone();
        async move {
            barrier.wait().await;
            client.refresh_session("refresh-1").await
        }
    };
    let refresh_two = {
        let barrier = barrier.clone();
        async move {
            barrier.wait().await;
            client.refresh_session("refresh-1").await
        }
    };

    let (first, second, _) = tokio::join!(refresh_one, refresh_two, async {
        barrier.wait().await;
    });

    let first = first.unwrap();
    let second = second.unwrap();
    assert_eq!(first.refresh_token.as_deref(), Some("refresh-2"));
    assert_eq!(second.refresh_token.as_deref(), Some("refresh-2"));
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn classifies_500_as_api_not_retried() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "msg": "upstream exploded"
        })))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let err = client.refresh_session("refresh-1").await.unwrap_err();

    match &err {
        Error::Api {
            status,
            code,
            message,
        } => {
            assert_eq!(*status, 500);
            assert_eq!(code, &None);
            assert_eq!(message, "upstream exploded");
        }
        other => panic!("unexpected error: {other}"),
    }

    assert!(!err.is_retryable());
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn extracts_code_field_when_api_version_current() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .respond_with(
            ResponseTemplate::new(401)
                .insert_header("x-supabase-api-version", "2024-01-01")
                .set_body_json(json!({
                    "code": "refresh_token_already_used",
                    "msg": "Invalid Refresh Token: Already Used"
                })),
        )
        .mount(&server)
        .await;

    let client = test_client(&server);
    let err = client.refresh_session("refresh-1").await.unwrap_err();

    match &err {
        Error::Api {
            status,
            code,
            message,
        } => {
            assert_eq!(*status, 401);
            assert_eq!(code.as_deref(), Some("refresh_token_already_used"));
            assert!(message.contains("Invalid Refresh Token"));
        }
        other => panic!("unexpected error: {other}"),
    }

    assert!(err.is_fatal());
}

#[tokio::test]
async fn ignores_code_field_when_api_version_missing() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "code": "refresh_token_already_used",
            "msg": "Invalid Refresh Token: Already Used"
        })))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let err = client.refresh_session("refresh-1").await.unwrap_err();

    match &err {
        Error::Api {
            status,
            code,
            message,
        } => {
            assert_eq!(*status, 401);
            assert_eq!(code, &None);
            assert!(message.contains("Invalid Refresh Token"));
        }
        other => panic!("unexpected error: {other}"),
    }

    assert!(!err.is_fatal());
}

#[tokio::test]
async fn ignores_code_field_for_legacy_api_version() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .respond_with(
            ResponseTemplate::new(401)
                .insert_header("x-supabase-api-version", "2023-06-01")
                .set_body_json(json!({
                    "code": "refresh_token_already_used",
                    "error_code": "refresh_token_not_found",
                    "msg": "Invalid Refresh Token"
                })),
        )
        .mount(&server)
        .await;

    let client = test_client(&server);
    let err = client.refresh_session("refresh-1").await.unwrap_err();

    match &err {
        Error::Api { code, .. } => {
            assert_eq!(code.as_deref(), Some("refresh_token_not_found"));
        }
        other => panic!("unexpected error: {other}"),
    }

    assert!(err.is_fatal());
}

#[tokio::test]
async fn extracts_legacy_error_code_field() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error_code": "refresh_token_not_found",
            "msg": "Refresh token not found"
        })))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let err = client.refresh_session("refresh-1").await.unwrap_err();

    match &err {
        Error::Api {
            status,
            code,
            message,
        } => {
            assert_eq!(*status, 400);
            assert_eq!(code.as_deref(), Some("refresh_token_not_found"));
            assert_eq!(message, "Refresh token not found");
        }
        other => panic!("unexpected error: {other}"),
    }

    assert!(err.is_fatal());
}

#[tokio::test]
async fn backfills_expires_at_from_expires_in() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "access-2",
            "refresh_token": "refresh-2",
            "token_type": "bearer",
            "expires_in": 3600,
            "user": { "id": "user-123", "email": null }
        })))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let before = now_epoch_secs();
    let session = client.refresh_session("refresh-1").await.unwrap();
    let after = now_epoch_secs();
    let expires_at = session.expires_at.unwrap();

    assert!(expires_at >= before + 3600);
    assert!(expires_at <= after + 3600);
}

#[tokio::test]
async fn refresh_session_accepts_token_only_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "access-2",
            "refresh_token": "refresh-2",
            "token_type": "bearer",
            "expires_in": 3600
        })))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let session = client.refresh_session("refresh-1").await.unwrap();

    assert_eq!(session.access_token, "access-2");
    assert_eq!(session.refresh_token.as_deref(), Some("refresh-2"));
    assert!(session.user.is_none());
}

#[tokio::test]
async fn rejects_response_missing_refresh_token() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "access-2",
            "token_type": "bearer",
            "expires_in": 3600,
            "user": { "id": "user-123", "email": null }
        })))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let err = client.refresh_session("refresh-1").await.unwrap_err();

    assert!(matches!(err, Error::InvalidSession(_)));
}

#[tokio::test]
async fn sends_reference_headers() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .and(header("apikey", "anon-key"))
        .and(header("authorization", "Bearer anon-key"))
        .and(header("x-supabase-api-version", "2024-01-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(session_payload()))
        .mount(&server)
        .await;

    let client = test_client(&server);
    client.refresh_session("refresh-1").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    let request = &requests[0];
    assert_eq!(
        request.body_json::<Value>().unwrap(),
        json!({ "refresh_token": "refresh-1" })
    );
    assert!(
        request
            .headers
            .get("x-client-info")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("hypr-supabase-auth/"))
    );
}

#[tokio::test]
async fn stops_retrying_when_budget_exhausted() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .respond_with(ResponseTemplate::new(503).set_body_json(json!({
            "msg": "temporarily unavailable"
        })))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let started = std::time::Instant::now();
    let err = client
        .refresh_session_with_total_delay("refresh-1", Duration::from_secs(2))
        .await
        .unwrap_err();

    match err {
        Error::Api { status, .. } => {
            assert_eq!(status, 503);
            assert!(
                Error::Api {
                    status,
                    code: None,
                    message: "temporarily unavailable".to_string(),
                }
                .is_retryable()
            );
        }
        other => panic!("unexpected error: {other}"),
    }

    assert!(started.elapsed() <= Duration::from_secs(3));
    assert_eq!(server.received_requests().await.unwrap().len(), 4);
}

#[tokio::test]
async fn msg_is_preferred_over_message() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "msg": "from msg",
            "message": "from message"
        })))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let err = client.refresh_session("refresh-1").await.unwrap_err();

    match err {
        Error::Api { message, .. } => assert_eq!(message, "from msg"),
        other => panic!("unexpected error: {other}"),
    }
}

#[tokio::test]
async fn session_not_found_becomes_session_missing() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .respond_with(
            ResponseTemplate::new(401)
                .insert_header("x-supabase-api-version", "2024-01-01")
                .set_body_json(json!({
                    "code": "session_not_found",
                    "msg": "Session from session_id claim in JWT does not exist"
                })),
        )
        .mount(&server)
        .await;

    let client = test_client(&server);
    let err = client.refresh_session("refresh-1").await.unwrap_err();

    assert!(matches!(err, Error::SessionMissing));
    assert!(err.is_fatal());
    assert!(!err.is_retryable());
}

#[tokio::test]
async fn session_not_found_legacy_error_code_also_becomes_session_missing() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "error_code": "session_not_found",
            "msg": "Session not found"
        })))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let err = client.refresh_session("refresh-1").await.unwrap_err();

    assert!(matches!(err, Error::SessionMissing));
}

#[tokio::test]
async fn session_expired_is_fatal() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .respond_with(
            ResponseTemplate::new(401)
                .insert_header("x-supabase-api-version", "2024-01-01")
                .set_body_json(json!({
                    "code": "session_expired",
                    "msg": "Session has expired"
                })),
        )
        .mount(&server)
        .await;

    let client = test_client(&server);
    let err = client.refresh_session("refresh-1").await.unwrap_err();

    match &err {
        Error::Api { code, .. } => assert_eq!(code.as_deref(), Some("session_expired")),
        other => panic!("unexpected error: {other}"),
    }
    assert!(err.is_fatal());
    assert!(!err.is_retryable());
}
