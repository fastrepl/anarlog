use axum::{Extension, Json, extract::State};
use hypr_api_auth::AuthContext;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::error::Result;
use crate::state::AppState;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateSessionRequest {
    pub integration_id: String,
    #[serde(default = "default_session_mode")]
    pub mode: SessionMode,
    #[serde(default)]
    pub connection_id: Option<String>,
}

fn default_session_mode() -> SessionMode {
    SessionMode::Auto
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionMode {
    Auto,
    Connect,
    Reconnect,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SessionResponse {
    pub token: String,
    pub expires_at: String,
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<String>,
}

#[utoipa::path(
    post,
    path = "/session",
    request_body(content = CreateSessionRequest, content_type = "application/json"),
    responses(
        (status = 200, description = "Session created", body = SessionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "nango",
)]
pub async fn create_session(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<CreateSessionRequest>,
) -> Result<Json<SessionResponse>> {
    let user_id = auth.claims.sub;

    match body.mode {
        SessionMode::Connect => {
            // Always create a new connection
        }
        SessionMode::Reconnect => {
            let connection_id = body.connection_id.as_deref().ok_or_else(|| {
                crate::error::NangoError::BadRequest(
                    "connection_id is required for reconnect mode".to_string(),
                )
            })?;

            let owns = state
                .supabase
                .verify_connection_ownership(
                    &auth.token,
                    &user_id,
                    connection_id,
                    &body.integration_id,
                )
                .await?;

            if !owns {
                return Err(crate::error::NangoError::Forbidden(
                    "connection not found or not owned by user".to_string(),
                ));
            }

            let reconnect_req = hypr_nango::ReconnectSessionRequest {
                connection_id: connection_id.to_string(),
                integration_id: body.integration_id.clone(),
            };

            let session = state.nango.reconnect_session(reconnect_req).await?;

            return Ok(Json(SessionResponse {
                token: session.token,
                expires_at: session.expires_at,
                mode: "reconnect".to_string(),
                connection_id: Some(connection_id.to_string()),
            }));
        }
        SessionMode::Auto => {
            if let Some(existing) = state
                .supabase
                .lookup_connection(&auth.token, &user_id, &body.integration_id)
                .await?
            {
                let reconnect_req = hypr_nango::ReconnectSessionRequest {
                    connection_id: existing.connection_id.clone(),
                    integration_id: body.integration_id.clone(),
                };

                match state.nango.reconnect_session(reconnect_req).await {
                    Ok(session) => {
                        return Ok(Json(SessionResponse {
                            token: session.token,
                            expires_at: session.expires_at,
                            mode: "reconnect".to_string(),
                            connection_id: Some(existing.connection_id),
                        }));
                    }
                    Err(hypr_nango::Error::Api(404, response_body)) => {
                        tracing::warn!(
                            enduser.id = %user_id,
                            hyprnote.integration.id = %body.integration_id,
                            hyprnote.connection.id = %existing.connection_id,
                            hyprnote.connection.status = %existing.status,
                            hyprnote.http.response.body = %response_body,
                            "reconnect session failed with not found, cleaning stale local row"
                        );
                        state
                            .supabase
                            .delete_connection(&user_id, &body.integration_id)
                            .await?;
                    }
                    Err(err) => {
                        return Err(err.into());
                    }
                }
            }
        }
    }

    let email = auth.claims.email;

    let mut tags = std::collections::HashMap::new();
    tags.insert("end_user_id".to_string(), user_id.clone());
    if let Some(ref e) = email {
        tags.insert("end_user_email".to_string(), e.clone());
    }

    let req = hypr_nango::CreateConnectSessionRequest {
        end_user: hypr_nango::EndUser {
            id: user_id,
            display_name: None,
            email,
            tags: Some(tags),
        },
        organization: None,
        allowed_integrations: Some(vec![body.integration_id]),
        integrations_config_defaults: None,
    };

    let session = state.nango.create_connect_session(req).await?;

    Ok(Json(SessionResponse {
        token: session.token,
        expires_at: session.expires_at,
        mode: "connect".to_string(),
        connection_id: None,
    }))
}
