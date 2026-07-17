use std::time::Duration;

use axum::{
    Extension, Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use backon::{ExponentialBuilder, Retryable};
use hypr_api_auth::AuthContext;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAccountResponse {
    pub deleted: bool,
    pub pending: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn retry_policy() -> ExponentialBuilder {
    ExponentialBuilder::default()
        .with_min_delay(Duration::from_millis(100))
        .with_max_times(3)
}

#[utoipa::path(
    delete,
    path = "/delete-account",
    responses(
        (status = 202, description = "Account deletion durably accepted", body = DeleteAccountResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
        (status = 503, description = "Durable account cleanup unavailable"),
    ),
    tag = "subscription",
)]
pub async fn delete_account(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
) -> Response {
    if !state.config.durable_cleanup_enabled {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(DeleteAccountResponse {
                deleted: false,
                pending: false,
                error: Some("account_deletion_unavailable".to_string()),
            }),
        )
            .into_response();
    }

    let Some(user_id) = canonical_user_id(&auth.claims.sub) else {
        tracing::warn!("Authenticated account deletion subject was not a canonical UUID");
        return (
            StatusCode::UNAUTHORIZED,
            Json(DeleteAccountResponse {
                deleted: false,
                pending: false,
                error: Some("invalid_account".to_string()),
            }),
        )
            .into_response();
    };

    if let Err(error) = (|| state.supabase.begin_account_deletion(&user_id))
        .retry(retry_policy())
        .sleep(tokio::time::sleep)
        .await
    {
        tracing::error!(
            enduser.id = %user_id,
            error = %error,
            "account_deletion_handoff_failed"
        );
        sentry::capture_message(&error.to_string(), sentry::Level::Error);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(DeleteAccountResponse {
                deleted: false,
                pending: false,
                error: Some("account_deletion_failed".to_string()),
            }),
        )
            .into_response();
    }

    try_delete_loops_contact(&state, &auth.token, &user_id).await;
    let _ = (|| state.supabase.revoke_user_sessions(&auth.token))
        .retry(retry_policy())
        .sleep(tokio::time::sleep)
        .await
        .inspect_err(|error| {
            tracing::warn!(
                enduser.id = %user_id,
                error = %error,
                "account_session_revocation_failed"
            )
        });
    tracing::info!(enduser.id = %user_id, "account_deletion_accepted");
    (
        StatusCode::ACCEPTED,
        Json(DeleteAccountResponse {
            deleted: false,
            pending: true,
            error: None,
        }),
    )
        .into_response()
}

fn canonical_user_id(value: &str) -> Option<String> {
    let uuid = Uuid::parse_str(value).ok()?;
    let canonical = uuid.to_string();
    (canonical == value).then_some(canonical)
}

#[cfg(test)]
mod tests {
    use axum::{
        Extension,
        body::{Body, to_bytes},
        http::{Method, Request, StatusCode},
    };
    use hypr_api_auth::{AuthContext, Claims};
    use hypr_api_env::{LoopsEnv, StripeEnv, SupabaseEnv};
    use serde_json::{Value, json};
    use tower::ServiceExt;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use crate::{SubscriptionConfig, router};

    const OWNER: &str = "00000000-0000-4000-8000-000000000401";

    fn test_router(server: &MockServer, owner: &str) -> axum::Router {
        test_router_with_cleanup(server, owner, true)
    }

    fn test_router_with_cleanup(
        server: &MockServer,
        owner: &str,
        cleanup_enabled: bool,
    ) -> axum::Router {
        router(
            SubscriptionConfig::new(
                &SupabaseEnv {
                    supabase_url: server.uri(),
                    supabase_anon_key: "anon-key".to_string(),
                    supabase_service_role_key: "service-role-key".to_string(),
                },
                &StripeEnv {
                    stripe_secret_key: "sk_test_fake".to_string(),
                    stripe_monthly_price_id: "price_monthly".to_string(),
                    stripe_yearly_price_id: "price_yearly".to_string(),
                },
                &LoopsEnv {
                    loops_key: "loops-key".to_string(),
                },
            )
            .with_durable_cleanup_enabled(cleanup_enabled),
        )
        .layer(Extension(AuthContext {
            token: "user-token".to_string(),
            claims: Claims {
                sub: owner.to_string(),
                email: None,
                entitlements: Vec::new(),
                subscription_status: None,
                trial_end: None,
                has_payment_method: None,
            },
        }))
    }

    fn request() -> Request<Body> {
        Request::builder()
            .method(Method::DELETE)
            .uri("/delete-account")
            .body(Body::empty())
            .unwrap()
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), 16 * 1024).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn accepts_deletion_only_after_the_durable_handoff() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/begin_account_deletion"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "owner_user_id": OWNER,
                "final_sweep_not_before": "2026-07-18T00:00:00Z",
                "was_created": true
            }])))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/auth/v1/logout"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/auth/v1/user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "email": null })))
            .mount(&server)
            .await;

        let response = test_router(&server, OWNER)
            .oneshot(request())
            .await
            .unwrap();
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(body, json!({ "deleted": false, "pending": true }));
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 3);
        assert_eq!(
            requests[0].url.path(),
            "/rest/v1/rpc/begin_account_deletion"
        );
        assert_eq!(requests[1].url.path(), "/auth/v1/user");
        assert_eq!(requests[2].url.path(), "/auth/v1/logout");
        assert!(
            requests
                .iter()
                .all(|request| request.method != Method::DELETE)
        );
    }

    #[tokio::test]
    async fn keeps_the_durable_acceptance_when_best_effort_cleanup_fails() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/begin_account_deletion"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "owner_user_id": OWNER,
                "final_sweep_not_before": "2026-07-18T00:00:00Z",
                "was_created": true
            }])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/auth/v1/user"))
            .respond_with(ResponseTemplate::new(503))
            .expect(4)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/auth/v1/logout"))
            .respond_with(ResponseTemplate::new(503))
            .expect(4)
            .mount(&server)
            .await;

        let response = test_router(&server, OWNER)
            .oneshot(request())
            .await
            .unwrap();
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(body, json!({ "deleted": false, "pending": true }));
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 9);
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.url.path() == "/auth/v1/user")
                .count(),
            4
        );
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.url.path() == "/auth/v1/logout")
                .count(),
            4
        );
    }

    #[tokio::test]
    async fn does_not_report_acceptance_when_the_durable_handoff_fails() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/begin_account_deletion"))
            .respond_with(ResponseTemplate::new(503))
            .expect(4)
            .mount(&server)
            .await;

        let response = test_router(&server, OWNER)
            .oneshot(request())
            .await
            .unwrap();
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            body,
            json!({
                "deleted": false,
                "pending": false,
                "error": "account_deletion_failed"
            })
        );
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 4);
        assert!(
            requests
                .iter()
                .all(|request| { request.url.path() == "/rest/v1/rpc/begin_account_deletion" })
        );
    }

    #[tokio::test]
    async fn rejects_noncanonical_subjects_without_external_calls() {
        let server = MockServer::start().await;

        let response = test_router(&server, "not-a-uuid")
            .oneshot(request())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn rejects_deletion_when_durable_cleanup_is_disabled() {
        let server = MockServer::start().await;

        let response = test_router_with_cleanup(&server, OWNER, false)
            .oneshot(request())
            .await
            .unwrap();
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            body,
            json!({
                "deleted": false,
                "pending": false,
                "error": "account_deletion_unavailable"
            })
        );
        assert!(server.received_requests().await.unwrap().is_empty());
    }
}

async fn try_delete_loops_contact(state: &AppState, token: &str, user_id: &str) {
    let email = match (|| state.supabase.get_user_email(token))
        .retry(retry_policy())
        .sleep(tokio::time::sleep)
        .await
    {
        Ok(Some(email)) => email,
        Ok(None) => {
            tracing::warn!(enduser.id = %user_id, "no_email_for_loops_deletion");
            return;
        }
        Err(e) => {
            tracing::warn!(
                enduser.id = %user_id,
                error = %e,
                "failed_to_get_email_for_loops"
            );
            return;
        }
    };

    let _ = (|| state.loops.delete_contact_by_email(&email))
        .retry(retry_policy())
        .sleep(tokio::time::sleep)
        .await
        .inspect_err(|e| {
            tracing::warn!(
                enduser.id = %user_id,
                error = %e,
                "loops_contact_deletion_failed"
            )
        });
}
