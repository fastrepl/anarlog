use axum::{Extension, Json, extract::State};
use hypr_api_auth::AuthContext;
use hypr_nango::ListConnectionsParams;
use serde::Serialize;
use utoipa::ToSchema;

use crate::error::Result;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
pub struct WhoAmIItem {
    pub integration_id: String,
    pub connection_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WhoAmIResponse {
    pub accounts: Vec<WhoAmIItem>,
}

#[utoipa::path(
    get,
    path = "/whoami",
    responses(
        (status = 200, description = "User info for all connections", body = WhoAmIResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "nango",
)]
pub async fn whoami(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<WhoAmIResponse>> {
    let rows = state
        .supabase
        .list_user_connections(&auth.token, &auth.claims.sub)
        .await?;

    let nango_connections = state
        .nango
        .list_connections(ListConnectionsParams {
            end_user_id: Some(auth.claims.sub.clone()),
            ..Default::default()
        })
        .await
        .unwrap_or_default();

    let nango_map: std::collections::HashMap<(&str, &str), _> = nango_connections
        .iter()
        .map(|c| {
            (
                (c.provider_config_key.as_str(), c.connection_id.as_str()),
                c,
            )
        })
        .collect();

    let accounts = rows
        .into_iter()
        .map(|row| {
            let is_reconnect_required = row.status == "reconnect_required";

            let email = nango_map
                .get(&(row.integration_id.as_str(), row.connection_id.as_str()))
                .and_then(|c| c.tags.as_ref())
                .and_then(|tags| tags.get("account_identity"))
                .cloned();

            WhoAmIItem {
                integration_id: row.integration_id,
                connection_id: row.connection_id,
                email,
                display_name: None,
                error: if is_reconnect_required {
                    Some("reconnect_required".to_string())
                } else {
                    None
                },
            }
        })
        .collect();

    Ok(Json(WhoAmIResponse { accounts }))
}
