use axum::{Json, extract::State, http::HeaderMap};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use hypr_nango::{AuthOperation, WebhookType};

use crate::error::{NangoError, Result};
use crate::state::AppState;

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
    let webhook_type = envelope.webhook_type;

    if webhook_type != WebhookType::Auth {
        tracing::info!(webhook_type = ?webhook_type, "nango webhook received (ignored)");
        return Ok(Json(WebhookResponse {
            status: "ok".to_string(),
        }));
    }

    let payload: hypr_nango::NangoAuthWebhook =
        serde_json::from_str(&body).map_err(|e| NangoError::BadRequest(e.to_string()))?;

    tracing::info!(
        webhook_type = ?payload.r#type,
        operation = ?payload.operation,
        connection_id = %payload.connection_id,
        end_user_id = payload.end_user_id().unwrap_or("unknown"),
        "nango webhook received"
    );

    if !state.supabase.is_configured() {
        tracing::warn!("supabase_service_role_key not configured, skipping connection persistence");
        return Ok(Json(WebhookResponse {
            status: "ok".to_string(),
        }));
    }

    if payload.operation == AuthOperation::Refresh && !payload.success {
        let error_type = payload.error.as_ref().map(|error| error.r#type.as_str());
        let error_description = payload
            .error
            .as_ref()
            .map(|error| error.description.as_str());

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
                tracing::error!(
                    error = %e,
                    "failed_to_persist_nango_refresh_failure_state"
                );
                NangoError::Internal(e.to_string())
            })?;

        tracing::warn!(
            hyprnote.connection.id = %payload.connection_id,
            hyprnote.integration.id = %payload.provider_config_key,
            error.type = error_type,
            error = error_description,
            "nango token refresh failed"
        );
    }

    if payload.success && payload.operation != AuthOperation::Deletion {
        let Some(end_user_id) = payload.end_user_id() else {
            tracing::warn!(
                hyprnote.connection.id = %payload.connection_id,
                hyprnote.integration.id = %payload.provider_config_key,
                "nango auth webhook missing end user id, skipping persistence"
            );
            return Ok(Json(WebhookResponse {
                status: "ok".to_string(),
            }));
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
    }

    Ok(Json(WebhookResponse {
        status: "ok".to_string(),
    }))
}
