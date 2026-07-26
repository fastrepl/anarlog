use axum::{Extension, Json, extract::State};
use hypr_api_auth::AuthContext;
use serde::Serialize;
use utoipa::ToSchema;

use crate::error::Result;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
pub struct ConnectionItem {
    pub integration_id: String,
    pub connection_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ListConnectionsResponse {
    pub connections: Vec<ConnectionItem>,
}

#[utoipa::path(
    get,
    path = "/connections",
    responses(
        (status = 200, description = "List of active connections", body = ListConnectionsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "nango",
)]
pub async fn list_connections(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<ListConnectionsResponse>> {
    let rows = state
        .supabase
        .list_user_connections(&auth.token, &auth.claims.sub)
        .await?;

    let connections = rows
        .into_iter()
        .map(|row| ConnectionItem {
            integration_id: row.integration_id,
            connection_id: row.connection_id,
            status: Some(row.status),
            last_error_type: row.last_error_type,
            last_error_description: row.last_error_description,
            last_error_at: row.last_error_at,
            updated_at: row.updated_at,
        })
        .collect();

    Ok(Json(ListConnectionsResponse { connections }))
}
