use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::{Json, extract::State, http::HeaderMap};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use hypr_nango::{AuthOperation, NangoAuthWebhook, WebhookType};

use crate::error::{NangoError, Result};
use crate::state::AppState;

pub type ForwardHandler =
    Arc<dyn Fn(serde_json::Value) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

pub type ForwardHandlerRegistry = HashMap<String, ForwardHandler>;

pub fn forward_handler<F, Fut>(f: F) -> ForwardHandler
where
    F: Fn(serde_json::Value) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    Arc::new(move |payload| Box::pin(f(payload)))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WebhookResponse {
    pub status: String,
}

#[derive(Debug, Deserialize)]
struct WebhookTypeEnvelope {
    #[serde(rename = "type")]
    webhook_type: WebhookType,
}

#[utoipa::path(
    post,
    path = "/webhook",
    responses(
        (status = 200, description = "Webhook processed", body = WebhookResponse),
        (status = 401, description = "Invalid signature"),
        (status = 400, description = "Bad request"),
    ),
    tag = "nango",
)]
pub async fn nango_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<WebhookResponse>> {
    let signature = headers
        .get("x-nango-hmac-sha256")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| NangoError::Auth("Missing X-Nango-Hmac-Sha256 header".to_string()))?;

    let valid = hypr_nango::verify_webhook_signature(
        &state.config.nango.nango_secret_key,
        body.as_bytes(),
        signature,
    );
    if !valid {
        return Err(NangoError::Auth("Invalid webhook signature".to_string()));
    }

    let envelope: WebhookTypeEnvelope =
        serde_json::from_str(&body).map_err(|e| NangoError::BadRequest(e.to_string()))?;

    match envelope.webhook_type {
        WebhookType::Forward => handle_forward_webhook(&state, &body)?,
        WebhookType::Auth => {
            let payload: NangoAuthWebhook =
                serde_json::from_str(&body).map_err(|e| NangoError::BadRequest(e.to_string()))?;
            handle_auth_webhook(&state, payload).await?;
        }
        other => {
            tracing::info!(webhook_type = ?other, "nango webhook received (ignored)");
        }
    }

    Ok(Json(WebhookResponse {
        status: "ok".to_string(),
    }))
}

fn handle_forward_webhook(state: &AppState, body: &str) -> Result<()> {
    let forward: hypr_nango::NangoForwardWebhook =
        serde_json::from_str(body).map_err(|e| NangoError::BadRequest(e.to_string()))?;

    tracing::info!(
        provider = %forward.provider,
        connection_id = %forward.connection_id,
        "nango forward webhook received"
    );

    if let Some(handler) = state
        .forward_handlers
        .get(forward.provider_config_key.as_str())
    {
        let handler = handler.clone();
        tokio::spawn(async move {
            handler(forward.payload).await;
        });
    } else {
        tracing::info!(
            provider_config_key = %forward.provider_config_key,
            "unhandled forward webhook provider"
        );
    }

    Ok(())
}

pub(crate) async fn handle_auth_webhook(state: &AppState, payload: NangoAuthWebhook) -> Result<()> {
    tracing::info!(
        webhook_type = ?payload.r#type,
        operation = ?payload.operation,
        connection_id = %payload.connection_id,
        end_user_id = payload.end_user_id().unwrap_or("unknown"),
        "nango webhook received"
    );

    if !state.supabase.is_configured() {
        tracing::warn!("supabase_service_role_key not configured, skipping connection persistence");
        return Ok(());
    }

    if payload.operation == AuthOperation::Refresh && !payload.success {
        let error_type = payload.error.as_ref().map(|e| e.r#type.as_str());
        let error_description = payload.error.as_ref().map(|e| e.description.as_str());

        state
            .supabase
            .mark_connection_refresh_failed(
                &payload.provider_config_key,
                &payload.connection_id,
                error_type,
                error_description,
            )
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "failed_to_persist_nango_refresh_failure_state");
                NangoError::Internal(e.to_string())
            })?;

        tracing::warn!(
            hyprnote.connection.id = %payload.connection_id,
            hyprnote.integration.id = %payload.provider_config_key,
            error.type = error_type,
            error = error_description,
            "nango token refresh failed"
        );

        return Ok(());
    }

    if payload.success && payload.operation == AuthOperation::Deletion {
        state
            .supabase
            .delete_connection_by_connection(&payload.provider_config_key, &payload.connection_id)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "failed_to_delete_nango_connection");
                NangoError::Internal(e.to_string())
            })?;

        tracing::info!(
            hyprnote.integration.id = %payload.provider_config_key,
            hyprnote.connection.id = %payload.connection_id,
            "nango connection deleted locally from webhook"
        );

        return Ok(());
    }

    if payload.success && payload.operation != AuthOperation::Deletion {
        let Some(end_user_id) = payload.end_user_id() else {
            tracing::warn!(
                hyprnote.connection.id = %payload.connection_id,
                hyprnote.integration.id = %payload.provider_config_key,
                "nango auth webhook missing end user id, skipping persistence"
            );
            return Ok(());
        };

        state
            .supabase
            .upsert_connection(
                end_user_id,
                &payload.provider_config_key,
                &payload.connection_id,
                &payload.provider,
            )
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "failed_to_upsert_nango_connection");
                NangoError::Internal(e.to_string())
            })?;

        tracing::info!(
            enduser.id = end_user_id,
            hyprnote.integration.id = %payload.provider_config_key,
            hyprnote.connection.id = %payload.connection_id,
            hyprnote.auth.operation = ?payload.operation,
            "nango connection upserted"
        );

        if matches!(
            payload.operation,
            AuthOperation::Creation | AuthOperation::Override
        ) {
            spawn_identity_task(
                state.nango.clone(),
                payload.provider_config_key.clone(),
                payload.connection_id.clone(),
            );
        }
    }

    Ok(())
}

fn spawn_identity_task(
    nango: hypr_nango::NangoClient,
    integration_id: String,
    connection_id: String,
) {
    tokio::spawn(async move {
        match super::identity::fetch_identity(&nango, &integration_id, &connection_id).await {
            Ok((email, _display_name)) => {
                let Some(identity) = email else {
                    return;
                };

                let mut tags = match nango.get_connection(&connection_id, &integration_id).await {
                    Ok(connection) => connection.tags.unwrap_or_default(),
                    Err(e) => {
                        tracing::warn!(
                            hyprnote.connection.id = %connection_id,
                            hyprnote.integration.id = %integration_id,
                            error = %e,
                            "failed to fetch connection before patching account_identity tag"
                        );
                        return;
                    }
                };
                tags.insert("account_identity".to_string(), identity.clone());

                let req = hypr_nango::PatchConnectionRequest {
                    end_user: None,
                    tags: Some(tags),
                };

                match nango
                    .patch_connection(&connection_id, &integration_id, req)
                    .await
                {
                    Ok(()) => {
                        tracing::info!(
                            hyprnote.connection.id = %connection_id,
                            hyprnote.integration.id = %integration_id,
                            account_identity = %identity,
                            "account_identity tag set"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            hyprnote.connection.id = %connection_id,
                            hyprnote.integration.id = %integration_id,
                            error = %e,
                            "failed to patch account_identity tag"
                        );
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    hyprnote.connection.id = %connection_id,
                    hyprnote.integration.id = %integration_id,
                    error = %e,
                    "failed to fetch identity for account_identity tag"
                );
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{body_json, method, path, path_regex, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use hypr_nango::{
        AuthOperation, NangoAuthWebhook, NangoWebhookEndUser, NangoWebhookError, WebhookType,
    };

    use crate::config::NangoConfig;
    use crate::error::NangoError;
    use crate::state::AppState;

    use super::{handle_auth_webhook, handle_forward_webhook};

    const SECRET: &str = "test-secret";

    fn sign_body(body: &str) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        let mut mac = Hmac::<Sha256>::new_from_slice(SECRET.as_bytes()).unwrap();
        mac.update(body.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    async fn make_fixture() -> (MockServer, MockServer, AppState) {
        let nango_mock = MockServer::start().await;
        let supabase_mock = MockServer::start().await;
        let config = NangoConfig::for_test(&nango_mock.uri(), &supabase_mock.uri());
        let state = AppState::new(config);
        (nango_mock, supabase_mock, state)
    }

    fn auth_payload(operation: AuthOperation, success: bool) -> NangoAuthWebhook {
        NangoAuthWebhook {
            r#type: WebhookType::Auth,
            operation,
            connection_id: "conn-123".to_string(),
            auth_mode: "OAUTH2".to_string(),
            provider_config_key: "google-calendar".to_string(),
            provider: "google-calendar".to_string(),
            environment: "DEV".to_string(),
            success,
            tags: None,
            end_user: Some(NangoWebhookEndUser {
                end_user_id: "user-abc".to_string(),
                end_user_email: None,
                tags: None,
            }),
            error: None,
        }
    }

    // --- Signature validation ---

    #[test]
    fn sign_body_produces_valid_signature() {
        let body = r#"{"type":"auth"}"#;
        let sig = sign_body(body);
        assert!(hypr_nango::verify_webhook_signature(
            SECRET,
            body.as_bytes(),
            &sig
        ));
    }

    #[test]
    fn wrong_signature_is_invalid() {
        let body = r#"{"type":"auth"}"#;
        assert!(!hypr_nango::verify_webhook_signature(
            SECRET,
            body.as_bytes(),
            "bad-sig"
        ));
    }

    // --- handle_auth_webhook ---

    #[tokio::test]
    async fn creation_upserts_connection_and_patches_identity() {
        let (nango_mock, supabase_mock, state) = make_fixture().await;

        Mock::given(method("POST"))
            .and(path("/rest/v1/nango_connections"))
            .respond_with(ResponseTemplate::new(201))
            .expect(1)
            .mount(&supabase_mock)
            .await;

        Mock::given(method("GET"))
            .and(path_regex("/proxy/.*userinfo.*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({"email": "user@example.com", "name": "Test User"}),
            ))
            .mount(&nango_mock)
            .await;

        Mock::given(method("GET"))
            .and(path("/connections/conn-123"))
            .and(query_param("provider_config_key", "google-calendar"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 1,
                "connection_id": "conn-123",
                "provider_config_key": "google-calendar",
                "provider": "google-calendar",
                "errors": [],
                "end_user": null,
                "tags": {
                    "end_user_id": "user-abc",
                    "end_user_email": "user@old.example.com"
                },
                "metadata": {},
                "connection_config": {},
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-01T00:00:00Z",
                "last_fetched_at": "2026-01-01T00:00:00Z",
                "credentials": {}
            })))
            .expect(1)
            .mount(&nango_mock)
            .await;

        Mock::given(method("PATCH"))
            .and(path_regex("/connections/conn-123"))
            .and(query_param("provider_config_key", "google-calendar"))
            .and(body_json(serde_json::json!({
                "tags": {
                    "end_user_id": "user-abc",
                    "end_user_email": "user@old.example.com",
                    "account_identity": "user@example.com"
                }
            })))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&nango_mock)
            .await;

        handle_auth_webhook(&state, auth_payload(AuthOperation::Creation, true))
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        supabase_mock.verify().await;
        nango_mock.verify().await;
    }

    #[tokio::test]
    async fn refresh_failure_marks_reconnect_required() {
        let (_nango_mock, supabase_mock, state) = make_fixture().await;

        Mock::given(method("PATCH"))
            .and(path_regex("/rest/v1/nango_connections"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&supabase_mock)
            .await;

        let mut payload = auth_payload(AuthOperation::Refresh, false);
        payload.error = Some(NangoWebhookError {
            r#type: "refresh_token_error".to_string(),
            description: "Token expired".to_string(),
        });

        handle_auth_webhook(&state, payload).await.unwrap();

        supabase_mock.verify().await;
    }

    #[tokio::test]
    async fn deletion_deletes_connection() {
        let (_nango_mock, supabase_mock, state) = make_fixture().await;

        Mock::given(method("DELETE"))
            .and(path_regex("/rest/v1/nango_connections"))
            .and(query_param("integration_id", "eq.google-calendar"))
            .and(query_param("connection_id", "eq.conn-123"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&supabase_mock)
            .await;

        handle_auth_webhook(&state, auth_payload(AuthOperation::Deletion, true))
            .await
            .unwrap();

        supabase_mock.verify().await;
    }

    #[tokio::test]
    async fn missing_end_user_id_skips_persistence() {
        let (_nango_mock, supabase_mock, state) = make_fixture().await;

        // No mocks mounted — any HTTP call would return connection refused
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500))
            .expect(0)
            .mount(&supabase_mock)
            .await;

        let mut payload = auth_payload(AuthOperation::Creation, true);
        payload.end_user = None;

        let result = handle_auth_webhook(&state, payload).await;

        assert!(result.is_ok());
        supabase_mock.verify().await;
    }

    #[tokio::test]
    async fn supabase_not_configured_skips_all_persistence() {
        let nango_mock = MockServer::start().await;
        let supabase_mock = MockServer::start().await;

        let mut config = NangoConfig::for_test(&nango_mock.uri(), &supabase_mock.uri());
        config.supabase_service_role_key = None;
        let state = AppState::new(config);

        // No mocks — any HTTP would panic
        let result = handle_auth_webhook(&state, auth_payload(AuthOperation::Creation, true)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn malformed_forward_webhook_returns_bad_request() {
        let (_nango_mock, _supabase_mock, state) = make_fixture().await;

        let result = handle_forward_webhook(&state, r#"{"type":"forward"}"#);

        assert!(matches!(result, Err(NangoError::BadRequest(_))));
    }
}
