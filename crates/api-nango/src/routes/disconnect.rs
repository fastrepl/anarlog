use axum::{Extension, Json, extract::State};
use hypr_api_auth::AuthContext;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::error::Result;
use crate::state::AppState;

#[derive(Debug, Deserialize, ToSchema)]
pub struct DeleteConnectionRequest {
    pub connection_id: String,
    pub integration_id: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DeleteConnectionResponse {
    pub status: String,
}

#[utoipa::path(
    delete,
    path = "/connections",
    request_body(content = DeleteConnectionRequest, content_type = "application/json"),
    responses(
        (status = 200, description = "Connection disconnected", body = DeleteConnectionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "nango",
)]
pub async fn delete_connection(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<DeleteConnectionRequest>,
) -> Result<Json<DeleteConnectionResponse>> {
    let owns = state
        .supabase
        .verify_connection_ownership(
            &auth.token,
            &auth.claims.sub,
            &body.connection_id,
            &body.integration_id,
        )
        .await?;

    if !owns {
        tracing::warn!(
            enduser.id = %auth.claims.sub,
            hyprnote.connection.id = %body.connection_id,
            hyprnote.integration.id = %body.integration_id,
            "disconnect denied: connection not owned by user"
        );
        return Err(crate::error::NangoError::Forbidden(
            "connection not found or not owned by user".to_string(),
        ));
    }

    match state
        .nango
        .delete_connection(&body.connection_id, &body.integration_id)
        .await
    {
        Ok(()) => {}
        Err(hypr_nango::Error::Api(404, response_body)) => {
            tracing::warn!(
                enduser.id = %auth.claims.sub,
                hyprnote.connection.id = %body.connection_id,
                hyprnote.integration.id = %body.integration_id,
                hyprnote.http.response.body = %response_body,
                "nango connection already deleted, cleaning local row"
            );
        }
        Err(err) => return Err(err.into()),
    }

    state
        .supabase
        .delete_connection(&auth.claims.sub, &body.integration_id)
        .await?;

    Ok(Json(DeleteConnectionResponse {
        status: "ok".to_string(),
    }))
}
