use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
    routing::get,
};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde_json::{Value, json};
use tower::ServiceExt;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

use super::*;

const TEST_KEY_ID: &str = "sync-composition-test";
const TEST_PRIVATE_KEY: &str = "-----BEGIN PRIVATE KEY-----\n\
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgWTFfCGljY6aw3Hrt\n\
kHmPRiazukxPLb6ilpRAewjW8nihRANCAATDskChT+Altkm9X7MI69T3IUmrQU0L\n\
950IxEzvw/x5BMEINRMrXLBJhqzO9Bm+d6JbqA21YQmd1Kt4RzLJR1W+\n\
-----END PRIVATE KEY-----";

fn token_with_entitlements(entitlements: &[&str]) -> String {
    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some(TEST_KEY_ID.to_string());
    let expires_at = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3_600;

    encode(
        &header,
        &json!({
            "sub": "editor-user",
            "aud": "authenticated",
            "exp": expires_at,
            "entitlements": entitlements
        }),
        &EncodingKey::from_ec_pem(TEST_PRIVATE_KEY.as_bytes()).unwrap(),
    )
    .unwrap()
}

fn non_pro_token() -> String {
    token_with_entitlements(&[])
}

fn test_sync_rate_limit(burst: u32) -> rate_limit::RateLimitState {
    let quota = || {
        governor::Quota::with_period(Duration::from_secs(1))
            .unwrap()
            .allow_burst(NonZeroU32::new(burst).unwrap())
    };
    rate_limit::RateLimitState::builder()
        .pro(quota())
        .free(quota())
        .enforce_in_debug()
        .build()
}

fn test_sync_state(server: &MockServer) -> anlg_api_sync::AppState {
    let config = anlg_api_sync::SyncConfig::new(
        "https://test.sqlite.cloud",
        "issuer-key",
        "database-id",
        server.uri(),
        "anon-key",
        "service-role-key",
    )
    .unwrap()
    .with_token_ttl_seconds(60)
    .unwrap();
    anlg_api_sync::AppState::new(config)
}

fn test_replica_state(server: &MockServer) -> anlg_api_sync::ReplicaState {
    anlg_api_sync::ReplicaState::new(
        anlg_api_sync::ReplicaConfig::new(server.uri(), "anon-key", "service-role-key").unwrap(),
    )
}

async fn response_bytes(response: axum::response::Response) -> Vec<u8> {
    to_bytes(response.into_body(), 16 * 1024)
        .await
        .unwrap()
        .to_vec()
}

fn deserialize_api_env(
    additional_values: impl IntoIterator<Item = (&'static str, &'static str)>,
) -> Env {
    let mut values = vec![
        ("SUPABASE_URL", "http://127.0.0.1:54321"),
        ("SUPABASE_ANON_KEY", "anon-key"),
        ("SUPABASE_SERVICE_ROLE_KEY", "service-role-key"),
        ("OPENROUTER_API_KEY", "openrouter-key"),
        ("API_BASE_URL", "http://127.0.0.1:3001"),
        ("RESEND_API_KEY", "resend-key"),
        ("RESEND_FROM_EMAIL", "test@example.com"),
    ];
    values.extend(additional_values);

    envy::from_iter(
        values
            .into_iter()
            .map(|(key, value)| (key.to_string(), value.to_string())),
    )
    .unwrap()
}

fn api_env(with_hosted_integrations: bool) -> &'static crate::env::RuntimeConfig {
    let mut integration_values = Vec::new();
    if with_hosted_integrations {
        integration_values.extend([
            ("NANGO_API_KEY", "nango-key"),
            ("NANGO_WEBHOOK_SIGNING_KEY", "nango-signing-key"),
            ("STRIPE_SECRET_KEY", "sk_test_hosted"),
            ("STRIPE_MONTHLY_PRICE_ID", "price_monthly"),
            ("STRIPE_YEARLY_PRICE_ID", "price_yearly"),
            ("LOOPS_KEY", "loops-key"),
            ("PYANNOTE_API_KEY", "pyannote-key"),
            ("EXA_API_KEY", "exa-key"),
            ("JINA_API_KEY", "jina-key"),
        ]);
    }

    let env = deserialize_api_env(integration_values);
    let config = crate::env::RuntimeConfig::resolve(env)
        .unwrap_or_else(|error| panic!("environment should validate: {error}"));
    Box::leak(Box::new(config))
}

fn api_env_with_subsystems() -> &'static crate::env::RuntimeConfig {
    let env = deserialize_api_env([
        ("SQLITECLOUD_PROJECT_URL", "https://test.sqlite.cloud"),
        ("SQLITECLOUD_TOKEN_ISSUER_API_KEY", "issuer-key"),
        ("ANARLOG_CLOUDSYNC_E2EE_DATABASE_ID", "database-id"),
        ("DEEPGRAM_API_KEY", "deepgram-key"),
    ]);
    let config = crate::env::RuntimeConfig::resolve(env)
        .unwrap_or_else(|error| panic!("environment should validate: {error}"));
    Box::leak(Box::new(config))
}

async fn request_status(app: &Router, method: Method, path: &str) -> StatusCode {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

async fn request_json(app: &Router, path: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(path)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = serde_json::from_slice(&response_bytes(response).await).unwrap();
    (status, body)
}

fn subsystem_health_app(
    cloudsync_configured: bool,
    transcription_configured: bool,
    llm_configured: bool,
    session_gate: anlg_transcribe_proxy::SessionGate,
) -> Router {
    Router::new()
        .route("/health/sync", get(sync_health))
        .route("/health/transcription", get(transcription_health))
        .route("/health/llm", get(llm_health))
        .with_state(SubsystemHealthState {
            cloudsync_configured,
            transcription_configured,
            llm_configured,
            session_gate,
        })
}

#[tokio::test]
async fn boots_with_minimal_self_hosted_configuration() {
    let app = app_with_env(api_env(false)).await;

    assert_eq!(
        request_status(&app, Method::GET, "/health").await,
        StatusCode::OK
    );
    for (method, path) in [
        (Method::POST, "/sync/replica/credentials"),
        (Method::PUT, "/sync/e2ee/identity"),
        (
            Method::GET,
            "/sync/e2ee/witness/11111111-1111-4111-8111-111111111111",
        ),
    ] {
        assert_eq!(
            request_status(&app, method, path).await,
            StatusCode::UNAUTHORIZED,
            "core encrypted replica route should be mounted: {path}"
        );
    }
    assert_eq!(
        request_status(&app, Method::POST, "/sync/token").await,
        StatusCode::NOT_FOUND,
        "SQLiteCloud credential route should remain optional"
    );
    for (method, path) in [
        (Method::POST, "/research/search"),
        (Method::POST, "/pyannote/v1/diarize"),
        (Method::POST, "/nango/session"),
        (Method::POST, "/nango/webhook"),
        (Method::POST, "/calendar/google/list-calendars"),
        (Method::GET, "/subscription/can-start-trial"),
    ] {
        assert_eq!(
            request_status(&app, method, path).await,
            StatusCode::NOT_FOUND,
            "optional route should not be mounted: {path}"
        );
    }
}

#[tokio::test]
async fn subsystem_health_routes_are_mounted_unauthenticated_and_secret_free() {
    let app = app_with_env(api_env_with_subsystems()).await;

    let (sync_status, sync_body) = request_json(&app, "/health/sync").await;
    assert_eq!(sync_status, StatusCode::OK);
    assert_eq!(sync_body, json!({"status": "ok", "configured": true}));

    let (transcription_status, transcription_body) =
        request_json(&app, "/health/transcription").await;
    assert_eq!(transcription_status, StatusCode::OK);
    assert_eq!(
        transcription_body,
        json!({"status": "ok", "configured": true, "draining": false})
    );

    let (llm_status, llm_body) = request_json(&app, "/health/llm").await;
    assert_eq!(llm_status, StatusCode::OK);
    assert_eq!(llm_body, json!({"status": "ok", "configured": true}));

    let bodies = format!("{sync_body}{transcription_body}{llm_body}");
    for secret in [
        "issuer-key",
        "database-id",
        "deepgram-key",
        "openrouter-key",
    ] {
        assert!(!bodies.contains(secret), "health response leaked {secret}");
    }
}

#[tokio::test]
async fn subsystem_health_handlers_report_unavailable_states() {
    let app = subsystem_health_app(
        true,
        false,
        false,
        anlg_transcribe_proxy::SessionGate::new(),
    );

    let (sync_status, sync_body) = request_json(&app, "/health/sync").await;
    assert_eq!(sync_status, StatusCode::OK);
    assert_eq!(sync_body, json!({"status": "ok", "configured": true}));

    let (transcription_status, transcription_body) =
        request_json(&app, "/health/transcription").await;
    assert_eq!(transcription_status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        transcription_body,
        json!({"status": "unavailable", "configured": false, "draining": false})
    );

    let (llm_status, llm_body) = request_json(&app, "/health/llm").await;
    assert_eq!(llm_status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        llm_body,
        json!({"status": "not_configured", "configured": false})
    );
}

#[tokio::test]
async fn transcription_health_reflects_drain_state() {
    let gate = anlg_transcribe_proxy::SessionGate::new();
    let app = subsystem_health_app(false, true, false, gate.clone());

    let (status, body) = request_json(&app, "/health/transcription").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({"status": "ok", "configured": true, "draining": false})
    );

    gate.begin_drain();
    let (status, body) = request_json(&app, "/health/transcription").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        body,
        json!({"status": "unavailable", "configured": true, "draining": true})
    );
}

#[tokio::test]
async fn drain_status_reports_live_streams_without_failing_health() {
    let gate = anlg_transcribe_proxy::SessionGate::new();
    let permit = gate.try_acquire().unwrap();
    let app = app_with_session_gate(api_env(false), gate.clone()).await;

    assert_eq!(
        request_status(&app, Method::GET, "/health").await,
        StatusCode::OK
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/drain")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(&response_bytes(response).await).unwrap();
    assert_eq!(body["draining"], false);
    assert_eq!(body["active_streams"], 1);

    gate.begin_drain();
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/drain")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(&response_bytes(response).await).unwrap();
    assert_eq!(body["draining"], true);
    assert_eq!(body["active_streams"], 1);
    drop(permit);
}

#[tokio::test]
async fn hosted_configuration_keeps_optional_routes_mounted() {
    let app = app_with_env(api_env(true)).await;

    for (method, path) in [
        (Method::POST, "/research/search"),
        (Method::POST, "/pyannote/v1/diarize"),
        (Method::POST, "/nango/session"),
        (Method::POST, "/calendar/google/list-calendars"),
        (Method::GET, "/subscription/can-start-trial"),
    ] {
        assert_eq!(
            request_status(&app, method, path).await,
            StatusCode::UNAUTHORIZED,
            "hosted route should remain mounted: {path}"
        );
    }
    assert_ne!(
        request_status(&app, Method::POST, "/nango/webhook").await,
        StatusCode::NOT_FOUND
    );
}

#[test]
fn partial_optional_integration_configuration_fails_closed() {
    for (values, expected) in [
        (
            vec![("NANGO_API_KEY", "nango-key")],
            "NANGO_WEBHOOK_SIGNING_KEY is required when Nango is configured",
        ),
        (
            vec![("NANGO_WEBHOOK_SIGNING_KEY", "nango-signing-key")],
            "NANGO_API_KEY is required when Nango is configured",
        ),
        (
            vec![("NANGO_API_BASE", "https://nango.example.com")],
            "NANGO_API_KEY is required when Nango is configured",
        ),
        (
            vec![("STRIPE_SECRET_KEY", "sk_test_partial")],
            "STRIPE_MONTHLY_PRICE_ID is required when subscriptions are configured",
        ),
        (
            vec![("STRIPE_MONTHLY_PRICE_ID", "price_monthly")],
            "STRIPE_SECRET_KEY is required when subscriptions are configured",
        ),
        (
            vec![("LOOPS_KEY", "loops-key")],
            "Stripe configuration is required when subscriptions are configured",
        ),
        (
            vec![
                ("STRIPE_SECRET_KEY", "sk_test_partial"),
                ("STRIPE_MONTHLY_PRICE_ID", "price_monthly"),
                ("STRIPE_YEARLY_PRICE_ID", "price_yearly"),
            ],
            "LOOPS_KEY is required when subscriptions are configured",
        ),
        (
            vec![("PYANNOTE_API_BASE", "https://pyannote.example.com")],
            "PYANNOTE_API_KEY is required when pyannote is configured",
        ),
        (
            vec![("EXA_API_KEY", "exa-key")],
            "JINA_API_KEY is required when research is configured",
        ),
        (
            vec![("JINA_API_KEY", "jina-key")],
            "EXA_API_KEY is required when research is configured",
        ),
    ] {
        let env = deserialize_api_env(values);

        assert_eq!(
            crate::env::RuntimeConfig::resolve(env).err(),
            Some(expected.to_string())
        );
    }
}

#[tokio::test]
async fn sync_composition_allows_non_pro_web_edits_but_keeps_other_routes_pro_only() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/auth/v1/.well-known/jwks.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "keys": [{
                "kty": "EC",
                "use": "sig",
                "crv": "P-256",
                "kid": TEST_KEY_ID,
                "x": "w7JAoU_gJbZJvV-zCOvU9yFJq0FNC_edCMRM78P8eQQ",
                "y": "wQg1EytcsEmGrM70Gb53oluoDbVhCZ3Uq3hHMslHVb4",
                "alg": "ES256"
            }]
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/edit_session_share_snapshot_cas"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "code": "42501",
            "message": "editor access required"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let sync_state = test_sync_state(&server);
    let replica_state = test_replica_state(&server);
    let app = Router::new().nest(
        "/sync",
        build_sync_routes(
            Some(sync_state),
            replica_state,
            test_sync_rate_limit(10),
            test_sync_rate_limit(10),
            test_sync_rate_limit(10),
            test_sync_rate_limit(10),
            AuthState::new(&server.uri()),
        ),
    );
    let token = non_pro_token();
    for (method, path) in [
        (Method::GET, "/sync/devices"),
        (Method::POST, "/sync/e2ee/device-enrollments"),
        (
            Method::POST,
            "/sync/e2ee/device-enrollments/11111111-1111-4111-8111-111111111111/seal",
        ),
        (
            Method::POST,
            "/sync/e2ee/device-enrollments/11111111-1111-4111-8111-111111111111/consume",
        ),
    ] {
        let request = |authenticated| {
            let mut builder = Request::builder().method(method.clone()).uri(path);
            if authenticated {
                builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
            }
            builder.body(Body::empty()).unwrap()
        };
        assert_eq!(
            app.clone().oneshot(request(false)).await.unwrap().status(),
            StatusCode::UNAUTHORIZED,
        );
        assert_eq!(
            app.clone().oneshot(request(true)).await.unwrap().status(),
            StatusCode::FORBIDDEN,
        );
    }
    let web_edit_response = app
        .clone()
        .oneshot(
            Request::put("/sync/shares/11111111-1111-4111-8111-111111111111/web-edit")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "baseRevision": 1,
                        "mutationId": "22222222-2222-4222-8222-222222222222",
                        "title": "Title",
                        "body": {
                            "type": "doc",
                            "content": [{ "type": "paragraph" }]
                        },
                        "attachmentIds": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(web_edit_response.status(), StatusCode::FORBIDDEN);
    let web_edit_body: Value =
        serde_json::from_slice(&response_bytes(web_edit_response).await).unwrap();
    assert_eq!(
        web_edit_body["error"]["code"],
        "shared_note_publication_forbidden"
    );

    let token_response = app
        .clone()
        .oneshot(
            Request::post("/sync/token")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(token_response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response_bytes(token_response).await,
        b"subscription_required"
    );

    let witness_response = app
        .oneshot(
            Request::get("/sync/e2ee/witness/editor-user")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(witness_response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response_bytes(witness_response).await,
        b"subscription_required"
    );
    server.verify().await;
}

#[tokio::test]
async fn sync_composition_keeps_devices_and_sharing_available_when_credentials_are_rate_limited() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/auth/v1/.well-known/jwks.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "keys": [{
                "kty": "EC",
                "use": "sig",
                "crv": "P-256",
                "kid": TEST_KEY_ID,
                "x": "w7JAoU_gJbZJvV-zCOvU9yFJq0FNC_edCMRM78P8eQQ",
                "y": "wQg1EytcsEmGrM70Gb53oluoDbVhCZ3Uq3hHMslHVb4",
                "alg": "ES256"
            }]
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/publish_session_share_snapshot_cas"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "code": "42501",
            "message": "share manager access required"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let sync_state = test_sync_state(&server);
    let replica_state = test_replica_state(&server);
    for route in [
        "/rest/v1/sync_devices",
        "/rest/v1/e2ee_device_enrollment_requests",
    ] {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .expect(1)
            .mount(&server)
            .await;
    }
    let app = Router::new().nest(
        "/sync",
        build_sync_routes(
            Some(sync_state),
            replica_state,
            test_sync_rate_limit(1),
            test_sync_rate_limit(1),
            test_sync_rate_limit(1),
            test_sync_rate_limit(1),
            AuthState::new(&server.uri()),
        ),
    );
    let token = token_with_entitlements(&["hyprnote_pro"]);
    let cloudsync_request = || {
        Request::post("/sync/token")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap()
    };

    let first_cloudsync_response = app.clone().oneshot(cloudsync_request()).await.unwrap();
    assert_eq!(
        first_cloudsync_response.status(),
        StatusCode::UPGRADE_REQUIRED
    );
    let second_cloudsync_response = app.clone().oneshot(cloudsync_request()).await.unwrap();
    assert_eq!(
        second_cloudsync_response.status(),
        StatusCode::TOO_MANY_REQUESTS
    );

    let devices_request = || {
        Request::get("/sync/devices")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap()
    };
    let devices_response = app.clone().oneshot(devices_request()).await.unwrap();
    assert_eq!(devices_response.status(), StatusCode::OK);
    let limited_devices_response = app.clone().oneshot(devices_request()).await.unwrap();
    assert_eq!(
        limited_devices_response.status(),
        StatusCode::TOO_MANY_REQUESTS
    );

    let share_response = app
        .oneshot(
            Request::put("/sync/shares/11111111-1111-4111-8111-111111111111/snapshot")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "baseRevision": 0,
                        "mutationId": "22222222-2222-4222-8222-222222222222",
                        "title": "Title",
                        "body": {
                            "type": "doc",
                            "content": [{ "type": "paragraph" }]
                        },
                        "attachmentIds": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(share_response.status(), StatusCode::FORBIDDEN);
    let share_body: Value = serde_json::from_slice(&response_bytes(share_response).await).unwrap();
    assert_eq!(
        share_body["error"]["code"],
        "shared_note_publication_forbidden"
    );
    server.verify().await;
}
