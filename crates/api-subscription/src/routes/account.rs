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
use stripe_core::customer::DeleteCustomer;
use utoipa::ToSchema;

use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAccountResponse {
    pub deleted: bool,
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
        (status = 200, description = "Account deleted successfully", body = DeleteAccountResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "subscription",
)]
pub async fn delete_account(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
) -> Response {
    let user_id = &auth.claims.sub;

    if let Err(response) = try_delete_stripe_customer(&state, user_id).await {
        return response;
    }

    let _ = (|| {
        state
            .supabase
            .admin_delete_storage_objects("audio-files", user_id)
    })
    .retry(retry_policy())
    .sleep(tokio::time::sleep)
    .await
    .inspect_err(|e| {
        tracing::warn!(
            enduser.id = %user_id,
            error = %e,
            "storage_cleanup_failed"
        )
    });

    try_delete_loops_contact(&state, &auth.token, user_id).await;

    match (|| state.supabase.admin_delete_user(user_id))
        .retry(retry_policy())
        .sleep(tokio::time::sleep)
        .await
    {
        Ok(()) => {
            tracing::info!(enduser.id = %user_id, "account_deleted");
            (
                StatusCode::OK,
                Json(DeleteAccountResponse {
                    deleted: true,
                    error: None,
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(
                enduser.id = %user_id,
                error = %e,
                "account_deletion_failed"
            );
            sentry::capture_message(&e.to_string(), sentry::Level::Error);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DeleteAccountResponse {
                    deleted: false,
                    error: Some("account_deletion_failed".to_string()),
                }),
            )
                .into_response()
        }
    }
}

async fn try_delete_stripe_customer(state: &AppState, user_id: &str) -> Result<(), Response> {
    let customer_id = match (|| state.supabase.admin_get_stripe_customer_id(user_id))
        .retry(retry_policy())
        .sleep(tokio::time::sleep)
        .await
    {
        Ok(Some(id)) => id,
        Ok(None) => {
            tracing::info!(enduser.id = %user_id, "no_stripe_customer_to_delete");
            return Ok(());
        }
        Err(e) => {
            tracing::warn!(
                enduser.id = %user_id,
                error = %e,
                "failed_to_lookup_stripe_customer"
            );
            return Ok(());
        }
    };

    match (|| async { DeleteCustomer::new(&*customer_id).send(&state.stripe).await })
        .retry(retry_policy())
        .sleep(tokio::time::sleep)
        .await
    {
        Ok(_) => {
            tracing::info!(
                enduser.id = %user_id,
                hyprnote.billing.customer.id = %customer_id,
                "stripe_customer_deleted"
            );
            Ok(())
        }
        Err(e) => {
            tracing::error!(
                enduser.id = %user_id,
                hyprnote.billing.customer.id = %customer_id,
                error = %e,
                "stripe_customer_deletion_failed"
            );
            sentry::capture_message(
                &format!("stripe customer deletion failed for {}: {}", user_id, e),
                sentry::Level::Error,
            );
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DeleteAccountResponse {
                    deleted: false,
                    error: Some("stripe_customer_deletion_failed".to_string()),
                }),
            )
                .into_response())
        }
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
