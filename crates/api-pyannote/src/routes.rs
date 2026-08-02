use anlg_api_auth::AuthContext;
use anlg_pyannote_cloud::ClientInfo;
use axum::{
    Extension, Json, Router,
    body::Body,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::{
    config::PyannoteConfig,
    error::{PyannoteError, Result},
    request::{DiarizeRequest, IdentifyRequest, VoiceprintRequest},
};

#[derive(Clone)]
struct AppState {
    client: anlg_pyannote_cloud::Client,
    job_handle_key: String,
}

const JOB_HANDLE_VERSION: &str = "v1";
const JOB_HANDLE_DOMAIN: &[u8] = b"anarlog-pyannote-job-handle-v1";

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
        job_handle_key: config.api_key,
    };

    Router::new()
        .route("/v1/diarize", post(diarize))
        .route("/v1/identify", post(identify))
        .route("/v1/voiceprint", post(voiceprint))
        .route("/v1/jobs/{job_id}", get(job))
        .route("/v1/media/input", post(media_input))
        .with_state(state)
}

async fn diarize(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(body): Json<DiarizeRequest>,
) -> Result<Response> {
    let body = sanitize_diarize_request(auth.claims.sub.as_str(), body)?;
    let payload = upstream_payload(body)?;

    forward_job_request(
        state
            .client
            .client()
            .post(format!("{}/v1/diarize", state.client.baseurl()))
            .json(&payload),
        auth.claims.sub.as_str(),
        &state.job_handle_key,
        true,
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

    forward_job_request(
        state
            .client
            .client()
            .post(format!("{}/v1/identify", state.client.baseurl()))
            .json(&payload),
        auth.claims.sub.as_str(),
        &state.job_handle_key,
        true,
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

    forward_job_request(
        state
            .client
            .client()
            .post(format!("{}/v1/voiceprint", state.client.baseurl()))
            .json(&payload),
        auth.claims.sub.as_str(),
        &state.job_handle_key,
        true,
    )
    .await
}

async fn job(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Path(job_handle): Path<String>,
) -> Result<Response> {
    let job_id = verify_job_handle(&state.job_handle_key, auth.claims.sub.as_str(), &job_handle)?;

    forward_job_request(
        state
            .client
            .client()
            .get(format!("{}/v1/jobs/{job_id}", state.client.baseurl())),
        auth.claims.sub.as_str(),
        &state.job_handle_key,
        false,
    )
    .await
}

async fn media_input(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(mut body): Json<anlg_pyannote_cloud::types::GetMediaUploadUrl>,
) -> Result<Response> {
    let url = normalize_media_url(auth.claims.sub.as_str(), &body.url)?;
    body.url = url
        .try_into()
        .map_err(|_| PyannoteError::bad_request("Invalid managed media URL"))?;

    forward_request(
        state
            .client
            .client()
            .post(format!("{}/v1/media/input", state.client.baseurl()))
            .json(&body),
    )
    .await
}

async fn forward_request(request: reqwest::RequestBuilder) -> Result<Response> {
    let (status, bytes) = upstream_response(request).await?;

    Ok((
        status,
        [("content-type", "application/json")],
        Body::from(bytes),
    )
        .into_response())
}

async fn forward_job_request(
    request: reqwest::RequestBuilder,
    user_id: &str,
    key: &str,
    job_id_required: bool,
) -> Result<Response> {
    let (status, bytes) = upstream_response(request).await?;
    let mut body: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| PyannoteError::bad_gateway("Invalid upstream response"))?;
    let job_id = body
        .get("jobId")
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string);

    if job_id_required && job_id.is_none() {
        return Err(PyannoteError::bad_gateway(
            "Upstream response did not include a job ID",
        ));
    }

    if let Some(job_id) = job_id {
        body["jobId"] = serde_json::Value::String(sign_job_handle(key, user_id, &job_id));
    }

    Ok((status, Json(body)).into_response())
}

async fn upstream_response(request: reqwest::RequestBuilder) -> Result<(StatusCode, Vec<u8>)> {
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
        return Ok((status, bytes.to_vec()));
    }

    let body = String::from_utf8_lossy(&bytes).to_string();
    let message = extract_upstream_error_message(&body).unwrap_or_else(|| default_message(status));
    Err(PyannoteError::upstream(status, message))
}

fn sanitize_diarize_request(
    user_id: &str,
    mut body: DiarizeRequest,
) -> Result<anlg_pyannote_cloud::types::DiarizeRequest> {
    body.url = normalize_media_url(user_id, &body.url)?;
    Ok(body.into())
}

fn sanitize_identify_request(
    user_id: &str,
    mut body: IdentifyRequest,
) -> Result<anlg_pyannote_cloud::types::IdentifyRequest> {
    body.url = normalize_media_url(user_id, &body.url)?;
    Ok(body.into())
}

fn sanitize_voiceprint_request(
    user_id: &str,
    mut body: VoiceprintRequest,
) -> Result<anlg_pyannote_cloud::types::VoiceprintRequest> {
    body.url = normalize_media_url(user_id, &body.url)?;
    Ok(body.into())
}

fn normalize_media_url(user_id: &str, url: &str) -> Result<String> {
    let prefix = format!("media://users/{user_id}/");
    let normalized = if url.starts_with(&prefix) {
        url.to_string()
    } else if let Some(suffix) = url.strip_prefix("media://owhisper/") {
        format!("{prefix}owhisper/{suffix}")
    } else {
        return Err(invalid_media_url());
    };
    let suffix = normalized.strip_prefix(&prefix);
    let valid = normalized.len() <= 255
        && normalized.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'/' | b'-' | b'_' | b'.')
        })
        && suffix.is_some_and(|suffix| {
            !suffix.is_empty()
                && suffix
                    .split('/')
                    .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
        });

    if valid {
        Ok(normalized)
    } else {
        Err(invalid_media_url())
    }
}

fn invalid_media_url() -> PyannoteError {
    PyannoteError::bad_request("Invalid media URL: expected caller-owned managed media")
}

fn sign_job_handle(key: &str, user_id: &str, job_id: &str) -> String {
    let encoded_job_id = URL_SAFE_NO_PAD.encode(job_id);
    let signature = job_handle_mac(key, user_id, job_id).finalize().into_bytes();
    format!(
        "{JOB_HANDLE_VERSION}.{encoded_job_id}.{}",
        URL_SAFE_NO_PAD.encode(signature)
    )
}

fn verify_job_handle(key: &str, user_id: &str, handle: &str) -> Result<String> {
    let mut parts = handle.split('.');
    let (Some(version), Some(encoded_job_id), Some(encoded_signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(invalid_job_handle());
    };

    if version != JOB_HANDLE_VERSION {
        return Err(invalid_job_handle());
    }

    let job_id = URL_SAFE_NO_PAD
        .decode(encoded_job_id)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .filter(|job_id| !job_id.is_empty())
        .ok_or_else(invalid_job_handle)?;
    let signature = URL_SAFE_NO_PAD
        .decode(encoded_signature)
        .map_err(|_| invalid_job_handle())?;

    job_handle_mac(key, user_id, &job_id)
        .verify_slice(&signature)
        .map_err(|_| invalid_job_handle())?;

    Ok(job_id)
}

fn job_handle_mac(key: &str, user_id: &str, job_id: &str) -> Hmac<Sha256> {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key.as_bytes())
        .expect("HMAC accepts keys of any length");
    mac.update(JOB_HANDLE_DOMAIN);
    mac.update(&(user_id.len() as u64).to_be_bytes());
    mac.update(user_id.as_bytes());
    mac.update(job_id.as_bytes());
    mac
}

fn invalid_job_handle() -> PyannoteError {
    PyannoteError::bad_request("Invalid job handle")
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
    use anlg_api_auth::{AuthContext, Claims};
    use axum::{Extension, Router, body::Body, body::to_bytes, http::Request, http::StatusCode};
    use serde_json::{Value, json};
    use tower::ServiceExt;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path},
    };

    use crate::config::PyannoteConfig;

    fn router_for_user(server: &MockServer, user_id: &str) -> Router {
        super::router(PyannoteConfig {
            api_key: "pyannote-key".to_string(),
            api_base: server.uri(),
        })
        .layer(Extension(AuthContext {
            token: "token".to_string(),
            claims: Claims {
                sub: user_id.to_string(),
                email: None,
                entitlements: vec![],
                subscription_status: None,
                trial_end: None,
                has_payment_method: None,
            },
        }))
    }

    fn router(server: &MockServer) -> Router {
        router_for_user(server, "user-123")
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
        let response_body = response_json(response).await;
        let job_handle = response_body["jobId"].as_str().unwrap();
        assert_ne!(job_handle, "job-123");
        assert_eq!(response_body["status"], json!("created"));
        assert_eq!(
            super::verify_job_handle("pyannote-key", "user-123", job_handle).unwrap(),
            "job-123"
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
    async fn job_detail_accepts_only_a_caller_bound_handle() {
        let server = MockServer::start().await;
        let job_handle = super::sign_job_handle("pyannote-key", "user-123", "job-123");

        Mock::given(method("GET"))
            .and(path("/v1/jobs/job-123"))
            .and(header("authorization", "Bearer pyannote-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "jobId": "job-123",
                "status": "succeeded",
                "output": {"voiceprint": "biometric-value"}
            })))
            .mount(&server)
            .await;

        let response = router(&server)
            .oneshot(
                Request::get(format!("/v1/jobs/{job_handle}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response_json(response).await,
            json!({
                "jobId": job_handle,
                "status": "succeeded",
                "output": {"voiceprint": "biometric-value"}
            })
        );
    }

    #[tokio::test]
    async fn job_detail_rejects_a_handle_bound_to_another_user() {
        let server = MockServer::start().await;
        let job_handle = super::sign_job_handle("pyannote-key", "user-999", "job-123");

        let response = router(&server)
            .oneshot(
                Request::get(format!("/v1/jobs/{job_handle}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response_json(response).await,
            json!({"error": {"code": "bad_request", "message": "Invalid job handle"}})
        );
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn media_input_forwards_only_caller_owned_media() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/media/input"))
            .and(header("authorization", "Bearer pyannote-key"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "url": "https://uploads.pyannote.test/presigned"
            })))
            .mount(&server)
            .await;

        let response = router(&server)
            .oneshot(
                Request::post("/v1/media/input")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"url":"media://users/user-123/audio.wav"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            response_json(response).await,
            json!({"url": "https://uploads.pyannote.test/presigned"})
        );

        let requests = server.received_requests().await.unwrap();
        assert_eq!(
            requests[0].body_json::<Value>().unwrap(),
            json!({"url": "media://users/user-123/audio.wav"})
        );
    }

    #[tokio::test]
    async fn media_input_scopes_an_owhisper_key_to_the_caller() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/media/input"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "url": "https://uploads.pyannote.test/presigned"
            })))
            .mount(&server)
            .await;

        let response = router(&server)
            .oneshot(
                Request::post("/v1/media/input")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"url":"media://owhisper/process-123-audio.mp3"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        let requests = server.received_requests().await.unwrap();
        assert_eq!(
            requests[0].body_json::<Value>().unwrap(),
            json!({"url": "media://users/user-123/owhisper/process-123-audio.mp3"})
        );
    }

    #[tokio::test]
    async fn diarize_scopes_an_owhisper_key_to_the_caller() {
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
                    .body(Body::from(
                        r#"{"url":"media://owhisper/process-123-audio.mp3"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let requests = server.received_requests().await.unwrap();
        assert_eq!(
            requests[0].body_json::<Value>().unwrap()["url"],
            json!("media://users/user-123/owhisper/process-123-audio.mp3")
        );
    }

    #[tokio::test]
    async fn media_input_rejects_another_users_media() {
        let server = MockServer::start().await;

        let response = router(&server)
            .oneshot(
                Request::post("/v1/media/input")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"url":"media://users/user-999/audio.wav"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn media_input_rejects_parent_path_segments() {
        let server = MockServer::start().await;

        let response = router(&server)
            .oneshot(
                Request::post("/v1/media/input")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"url":"media://users/user-123/../user-999/audio.wav"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(server.received_requests().await.unwrap().is_empty());
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
