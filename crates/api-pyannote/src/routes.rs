use axum::{
    Extension, Json, Router,
    body::Body,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use hypr_api_auth::AuthContext;
use hypr_pyannote_cloud::ClientInfo;
use serde::{Deserialize, Serialize};

use crate::{
    config::PyannoteConfig,
    error::{PyannoteError, Result},
    request::{DiarizeRequest, IdentifyRequest, VoiceprintRequest},
};

#[derive(Clone)]
struct AppState {
    client: hypr_pyannote_cloud::Client,
}

#[derive(Debug, Deserialize)]
struct UpstreamApiError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct UpstreamValidationError {
    message: String,
}

pub fn router(config: PyannoteConfig) -> Router {
    let state = AppState {
        client: config.client().expect("failed to build pyannote client"),
    };

    Router::new()
        .route("/v1/diarize", post(diarize))
        .route("/v1/identify", post(identify))
        .route("/v1/voiceprint", post(voiceprint))
        .with_state(state)
}

async fn diarize(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(body): Json<DiarizeRequest>,
) -> Result<Response> {
    let body = sanitize_diarize_request(auth.claims.sub.as_str(), body)?;
    let payload = upstream_payload(body)?;

    forward_request(
        state
            .client
            .client()
            .post(format!("{}/v1/diarize", state.client.baseurl()))
            .json(&payload),
    )
    .await
}

async fn identify(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(body): Json<IdentifyRequest>,
) -> Result<Response> {
    let body = sanitize_identify_request(auth.claims.sub.as_str(), body)?;
    let payload = upstream_payload(body)?;

    forward_request(
        state
            .client
            .client()
            .post(format!("{}/v1/identify", state.client.baseurl()))
            .json(&payload),
    )
    .await
}

async fn voiceprint(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(body): Json<VoiceprintRequest>,
) -> Result<Response> {
    let body = sanitize_voiceprint_request(auth.claims.sub.as_str(), body)?;
    let payload = upstream_payload(body)?;

    forward_request(
        state
            .client
            .client()
            .post(format!("{}/v1/voiceprint", state.client.baseurl()))
            .json(&payload),
    )
    .await
}

async fn forward_request(request: reqwest::RequestBuilder) -> Result<Response> {
    let response = request
        .send()
        .await
        .map_err(|err| PyannoteError::bad_gateway(err.to_string()))?;
    let status = status_code(response.status());
    let bytes = response
        .bytes()
        .await
        .map_err(|err| PyannoteError::bad_gateway(err.to_string()))?;

    if status.is_success() {
        return Ok((
            status,
            [("content-type", "application/json")],
            Body::from(bytes),
        )
            .into_response());
    }

    let body = String::from_utf8_lossy(&bytes).to_string();
    let message = extract_upstream_error_message(&body).unwrap_or_else(|| default_message(status));
    Err(PyannoteError::upstream(status, message))
}

fn sanitize_diarize_request(
    user_id: &str,
    mut body: DiarizeRequest,
) -> Result<hypr_pyannote_cloud::types::DiarizeRequest> {
    body.url = validate_media_url(user_id, &body.url)?;
    Ok(body.into())
}

fn sanitize_identify_request(
    user_id: &str,
    mut body: IdentifyRequest,
) -> Result<hypr_pyannote_cloud::types::IdentifyRequest> {
    body.url = validate_media_url(user_id, &body.url)?;
    Ok(body.into())
}

fn sanitize_voiceprint_request(
    user_id: &str,
    mut body: VoiceprintRequest,
) -> Result<hypr_pyannote_cloud::types::VoiceprintRequest> {
    body.url = validate_media_url(user_id, &body.url)?;
    Ok(body.into())
}

fn validate_media_url(user_id: &str, url: &str) -> Result<String> {
    let prefix = format!("media://users/{user_id}/");
    if url.starts_with(&prefix) {
        Ok(url.to_string())
    } else {
        Err(PyannoteError::bad_request(
            "Invalid media URL: expected caller-owned managed media",
        ))
    }
}

fn upstream_payload(body: impl Serialize) -> Result<serde_json::Value> {
    let mut payload =
        serde_json::to_value(body).map_err(|err| PyannoteError::bad_gateway(err.to_string()))?;

    if let Some(object) = payload.as_object_mut() {
        object.remove("webhookStatusOnly");
    }

    Ok(payload)
}

fn extract_upstream_error_message(body: &str) -> Option<String> {
    if body.trim().is_empty() {
        return None;
    }

    serde_json::from_str::<UpstreamApiError>(body)
        .map(|error| error.message)
        .ok()
        .or_else(|| {
            serde_json::from_str::<UpstreamValidationError>(body)
                .map(|error| error.message)
                .ok()
        })
        .or_else(|| error_message_from_json_body(body))
}

fn status_code(status: reqwest::StatusCode) -> StatusCode {
    StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY)
}

fn error_message_from_json_body(body: &str) -> Option<String> {
    if body.trim().is_empty() {
        return None;
    }

    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("message")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string)
                .or_else(|| {
                    value
                        .get("error")
                        .and_then(|error| error.get("message"))
                        .and_then(serde_json::Value::as_str)
                        .map(ToString::to_string)
                })
        })
}

fn default_message(status: StatusCode) -> String {
    match status {
        StatusCode::BAD_REQUEST => "Invalid request".to_string(),
        StatusCode::PAYMENT_REQUIRED => "Subscription is required".to_string(),
        StatusCode::TOO_MANY_REQUESTS => "Too many requests".to_string(),
        StatusCode::NOT_FOUND => "Resource not found".to_string(),
        _ => status
            .canonical_reason()
            .unwrap_or("Upstream request failed")
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use axum::{Extension, Router, body::Body, body::to_bytes, http::Request, http::StatusCode};
    use hypr_api_auth::{AuthContext, Claims};
    use serde_json::{Value, json};
    use tower::ServiceExt;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path},
    };

    use crate::config::PyannoteConfig;

    fn router(server: &MockServer) -> Router {
        super::router(PyannoteConfig {
            api_key: "pyannote-key".to_string(),
            api_base: server.uri(),
        })
        .layer(Extension(AuthContext {
            token: "token".to_string(),
            claims: Claims {
                sub: "user-123".to_string(),
                email: None,
                entitlements: vec![],
                subscription_status: None,
                trial_end: None,
            },
        }))
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn diarize_forwards_owned_media_url_and_auth_header() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/diarize"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "jobId": "job-123",
                "status": "created"
            })))
            .mount(&server)
            .await;

        let response = router(&server)
            .oneshot(
                Request::post("/v1/diarize")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"url":"media://users/user-123/audio.wav"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response_json(response).await,
            json!({"jobId": "job-123", "status": "created"})
        );

        let requests = server.received_requests().await.unwrap();
        let request = &requests[0];
        assert_eq!(request.method.as_str(), "POST");
        assert_eq!(request.url.path(), "/v1/diarize");
        assert_eq!(
            request
                .headers
                .get("authorization")
                .unwrap()
                .to_str()
                .unwrap(),
            "Bearer pyannote-key"
        );

        let body = request.body_json::<Value>().unwrap();
        assert_eq!(body["url"], json!("media://users/user-123/audio.wav"));
        assert!(body.get("webhook").is_none());
        assert!(body.get("webhookStatusOnly").is_none());
    }

    #[tokio::test]
    async fn identify_forwards_owned_media_url_without_webhook_fields() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/identify"))
            .and(header("authorization", "Bearer pyannote-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "jobId": "job-123",
                "status": "created"
            })))
            .mount(&server)
            .await;

        let response = router(&server)
            .oneshot(
                Request::post("/v1/identify")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"url":"media://users/user-123/audio.wav","voiceprints":[{"label":"speaker-a","voiceprint":"abc"}]}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let requests = server.received_requests().await.unwrap();
        let body = requests[0].body_json::<Value>().unwrap();
        assert_eq!(body["url"], json!("media://users/user-123/audio.wav"));
        assert!(body.get("webhook").is_none());
        assert!(body.get("webhookStatusOnly").is_none());
    }

    #[tokio::test]
    async fn voiceprint_forwards_owned_media_url_without_webhook_fields() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/voiceprint"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"jobId": "job-123", "status": "created"})),
            )
            .mount(&server)
            .await;

        let response = router(&server)
            .oneshot(
                Request::post("/v1/voiceprint")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"url":"media://users/user-123/audio.wav"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let requests = server.received_requests().await.unwrap();
        let body = requests[0].body_json::<Value>().unwrap();
        assert_eq!(body["url"], json!("media://users/user-123/audio.wav"));
        assert!(body.get("webhook").is_none());
        assert!(body.get("webhookStatusOnly").is_none());
    }

    #[tokio::test]
    async fn test_route_is_not_exposed() {
        let server = MockServer::start().await;

        let response = router(&server)
            .oneshot(Request::get("/v1/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn jobs_route_is_not_exposed() {
        let server = MockServer::start().await;

        let response = router(&server)
            .oneshot(Request::get("/v1/jobs").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn job_detail_route_is_not_exposed() {
        let server = MockServer::start().await;

        let response = router(&server)
            .oneshot(
                Request::get("/v1/jobs/job-123")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn media_input_route_is_not_exposed() {
        let server = MockServer::start().await;

        let response = router(&server)
            .oneshot(
                Request::post("/v1/media/input")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"url":"media://users/user-123/audio.wav"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn media_output_route_is_not_exposed() {
        let server = MockServer::start().await;

        let response = router(&server)
            .oneshot(
                Request::post("/v1/media/output")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"url":"media://users/user-123/audio.wav"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn diarize_rejects_external_url() {
        let server = MockServer::start().await;

        let response = router(&server)
            .oneshot(
                Request::post("/v1/diarize")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"url":"https://example.com/audio.wav"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response_json(response).await,
            json!({"error": {"code": "bad_request", "message": "Invalid media URL: expected caller-owned managed media"}})
        );
    }

    #[tokio::test]
    async fn identify_rejects_media_owned_by_another_user() {
        let server = MockServer::start().await;

        let response = router(&server)
            .oneshot(
                Request::post("/v1/identify")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"url":"media://users/user-999/audio.wav","voiceprints":[{"label":"speaker-a","voiceprint":"abc"}]}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response_json(response).await,
            json!({"error": {"code": "bad_request", "message": "Invalid media URL: expected caller-owned managed media"}})
        );
    }

    #[tokio::test]
    async fn voiceprint_requires_url() {
        let server = MockServer::start().await;

        let response = router(&server)
            .oneshot(
                Request::post("/v1/voiceprint")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn diarize_rejects_unknown_webhook_fields() {
        let server = MockServer::start().await;

        let response = router(&server)
            .oneshot(
                Request::post("/v1/diarize")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"url":"media://users/user-123/audio.wav","webhook":"https://example.com/webhook"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn upstream_bad_request_maps_to_char_error_shape() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/voiceprint"))
            .respond_with(
                ResponseTemplate::new(400).set_body_json(json!({"message": "Invalid key"})),
            )
            .mount(&server)
            .await;

        let response = router(&server)
            .oneshot(
                Request::post("/v1/voiceprint")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"url":"media://users/user-123/audio.wav"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response_json(response).await,
            json!({"error": {"code": "bad_request", "message": "Invalid key"}})
        );
    }

    #[tokio::test]
    async fn upstream_rate_limit_maps_to_char_error_shape() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/voiceprint"))
            .respond_with(ResponseTemplate::new(429).set_body_json(json!({"message": "Slow down"})))
            .mount(&server)
            .await;

        let response = router(&server)
            .oneshot(
                Request::post("/v1/voiceprint")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"url":"media://users/user-123/audio.wav"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response_json(response).await,
            json!({"error": {"code": "rate_limited", "message": "Slow down"}})
        );
    }

    #[tokio::test]
    async fn upstream_validation_error_preserves_message() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/voiceprint"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "message": "Invalid request",
                "errors": [{"field": "url", "message": "Invalid URL"}]
            })))
            .mount(&server)
            .await;

        let response = router(&server)
            .oneshot(
                Request::post("/v1/voiceprint")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"url":"media://users/user-123/audio.wav"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response_json(response).await,
            json!({"error": {"code": "bad_request", "message": "Invalid request"}})
        );
    }

    #[tokio::test]
    async fn malformed_upstream_body_falls_back_to_default_message() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/voiceprint"))
            .respond_with(ResponseTemplate::new(429).set_body_string("<<<not-json>>>"))
            .mount(&server)
            .await;

        let response = router(&server)
            .oneshot(
                Request::post("/v1/voiceprint")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"url":"media://users/user-123/audio.wav"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response_json(response).await,
            json!({"error": {"code": "rate_limited", "message": "Too many requests"}})
        );
    }

    #[tokio::test]
    async fn empty_upstream_body_falls_back_to_default_message() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/voiceprint"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;

        let response = router(&server)
            .oneshot(
                Request::post("/v1/voiceprint")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"url":"media://users/user-123/audio.wav"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response_json(response).await,
            json!({"error": {"code": "rate_limited", "message": "Too many requests"}})
        );
    }

    #[tokio::test]
    async fn upstream_server_error_still_redacts_message() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/voiceprint"))
            .respond_with(
                ResponseTemplate::new(500).set_body_json(json!({"message": "Upstream exploded"})),
            )
            .mount(&server)
            .await;

        let response = router(&server)
            .oneshot(
                Request::post("/v1/voiceprint")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"url":"media://users/user-123/audio.wav"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response_json(response).await,
            json!({"error": {"code": "upstream_error", "message": "Internal server error"}})
        );
    }
}
