use reqwest::header::{HeaderMap, HeaderValue};
use serde::Serialize;

use crate::session::Session;

pub use error::{Error, Result};

mod error;

const AUTH_V1: &str = "/auth/v1";

#[derive(Clone)]
pub struct AuthClient {
    client: reqwest::Client,
    supabase_url: String,
    anon_key: String,
}

#[derive(Serialize)]
struct RefreshSessionPayload<'a> {
    refresh_token: &'a str,
}

impl AuthClient {
    pub fn new(supabase_url: impl Into<String>, anon_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            supabase_url: supabase_url.into().trim_end_matches('/').to_string(),
            anon_key: anon_key.into(),
        }
    }

    pub async fn refresh_session(&self, refresh_token: &str) -> Result<Session> {
        if refresh_token.is_empty() {
            return Err(Error::MissingRefreshToken);
        }

        let mut headers = HeaderMap::new();
        headers.insert("apikey", HeaderValue::from_str(&self.anon_key)?);

        let response = self
            .client
            .post(format!(
                "{}{AUTH_V1}/token?grant_type=refresh_token",
                self.supabase_url
            ))
            .headers(headers)
            .json(&RefreshSessionPayload { refresh_token })
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;

        if status.is_success() {
            return serde_json::from_str(&body).map_err(Error::InvalidSession);
        }

        Err(Error::Auth {
            status: status.as_u16(),
            message: extract_error_message(&body),
        })
    }
}

fn extract_error_message(body: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return body.to_string();
    };

    for key in ["message", "msg", "error_description", "error"] {
        if let Some(message) = value.get(key).and_then(serde_json::Value::as_str) {
            return message.to_string();
        }
    }

    body.to_string()
}

#[cfg(test)]
mod tests {
    use axum::{Json, Router, extract::Query, routing::post};
    use serde::Deserialize;
    use serde_json::json;
    use tokio::net::TcpListener;

    use super::*;

    #[derive(Deserialize)]
    struct GrantTypeQuery {
        grant_type: String,
    }

    #[tokio::test]
    async fn refresh_session_returns_rotated_session() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let app =
                Router::new().route(
                    "/auth/v1/token",
                    post(
                        |Query(query): Query<GrantTypeQuery>,
                         Json(body): Json<serde_json::Value>| async move {
                            assert_eq!(query.grant_type, "refresh_token");
                            assert_eq!(body["refresh_token"], "refresh-1");

                            Json(json!({
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
                            }))
                        },
                    ),
                );

            axum::serve(listener, app).await.unwrap();
        });

        let client = AuthClient::new(format!("http://{addr}"), "anon-key");
        let session = client.refresh_session("refresh-1").await.unwrap();

        assert_eq!(session.access_token, "access-2");
        assert_eq!(session.refresh_token.as_deref(), Some("refresh-2"));
        assert_eq!(session.user.id, "user-123");

        server.abort();
    }

    #[tokio::test]
    async fn refresh_session_surfaces_auth_errors() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let app = Router::new().route(
                "/auth/v1/token",
                post(|| async {
                    (
                        axum::http::StatusCode::UNAUTHORIZED,
                        Json(json!({
                            "msg": "Invalid Refresh Token: Already Used"
                        })),
                    )
                }),
            );

            axum::serve(listener, app).await.unwrap();
        });

        let client = AuthClient::new(format!("http://{addr}"), "anon-key");
        let err = client.refresh_session("refresh-1").await.unwrap_err();

        match err {
            Error::Auth { status, message } => {
                assert_eq!(status, 401);
                assert!(message.contains("Invalid Refresh Token"));
            }
            other => panic!("unexpected error: {other}"),
        }

        server.abort();
    }

    #[tokio::test]
    async fn refresh_session_rejects_invalid_payloads() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let app = Router::new().route(
                "/auth/v1/token",
                post(|| async { Json(json!({ "access_token": "missing-user" })) }),
            );

            axum::serve(listener, app).await.unwrap();
        });

        let client = AuthClient::new(format!("http://{addr}"), "anon-key");
        let err = client.refresh_session("refresh-1").await.unwrap_err();
        assert!(matches!(err, Error::InvalidSession(_)));

        server.abort();
    }

    #[tokio::test]
    async fn refresh_session_extracts_error_description() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let app = Router::new().route(
                "/auth/v1/token",
                post(|| async {
                    (
                        axum::http::StatusCode::BAD_REQUEST,
                        Json(json!({
                            "error": "invalid_grant",
                            "error_description": "Token has been revoked"
                        })),
                    )
                }),
            );

            axum::serve(listener, app).await.unwrap();
        });

        let client = AuthClient::new(format!("http://{addr}"), "anon-key");
        let err = client.refresh_session("refresh-1").await.unwrap_err();

        match err {
            Error::Auth { status, message } => {
                assert_eq!(status, 400);
                assert_eq!(message, "Token has been revoked");
            }
            other => panic!("unexpected error: {other}"),
        }

        server.abort();
    }

    #[tokio::test]
    async fn refresh_session_falls_back_to_raw_body_for_non_json_error() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let app = Router::new().route(
                "/auth/v1/token",
                post(|| async {
                    (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        "upstream exploded",
                    )
                }),
            );

            axum::serve(listener, app).await.unwrap();
        });

        let client = AuthClient::new(format!("http://{addr}"), "anon-key");
        let err = client.refresh_session("refresh-1").await.unwrap_err();

        match err {
            Error::Auth { status, message } => {
                assert_eq!(status, 500);
                assert_eq!(message, "upstream exploded");
            }
            other => panic!("unexpected error: {other}"),
        }

        server.abort();
    }

    #[tokio::test]
    async fn refresh_session_rejects_empty_refresh_token() {
        let client = AuthClient::new("http://unused", "anon-key");
        let err = client.refresh_session("").await.unwrap_err();
        assert!(matches!(err, Error::MissingRefreshToken));
    }

    #[tokio::test]
    async fn refresh_session_handles_trailing_slash_base_url() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let app = Router::new().route(
                "/auth/v1/token",
                post(|Json(body): Json<serde_json::Value>| async move {
                    assert_eq!(body["refresh_token"], "refresh-1");
                    Json(json!({
                        "access_token": "access-2",
                        "refresh_token": "refresh-2",
                        "token_type": "bearer",
                        "user": { "id": "user-123", "email": null }
                    }))
                }),
            );

            axum::serve(listener, app).await.unwrap();
        });

        let client = AuthClient::new(format!("http://{addr}/"), "anon-key");
        let session = client.refresh_session("refresh-1").await.unwrap();
        assert_eq!(session.access_token, "access-2");

        server.abort();
    }
}
