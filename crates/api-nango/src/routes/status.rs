use anlg_api_auth::AuthContext;
use anlg_nango::ListConnectionsParams;
use axum::{Extension, Json, extract::State};
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_identity: Option<String>,
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
        .map(|connection| {
            (
                (
                    connection.provider_config_key.as_str(),
                    connection.connection_id.as_str(),
                ),
                connection,
            )
        })
        .collect();

    let mut connections: Vec<ConnectionItem> = rows
        .into_iter()
        .map(|row| {
            let nango = nango_map.get(&(row.integration_id.as_str(), row.connection_id.as_str()));
            let account_identity = nango.and_then(|connection| {
                super::identity::account_identity_from_tags(connection.tags.as_ref())
            });

            ConnectionItem {
                integration_id: row.integration_id,
                connection_id: row.connection_id,
                status: Some(row.status),
                last_error_type: row.last_error_type,
                last_error_description: row.last_error_description,
                last_error_at: row.last_error_at,
                updated_at: row.updated_at,
                account_identity,
            }
        })
        .collect();

    let missing: Vec<(String, String)> = connections
        .iter()
        .filter(|item| {
            item.account_identity.is_none() && item.status.as_deref() != Some("reconnect_required")
        })
        .map(|item| (item.integration_id.clone(), item.connection_id.clone()))
        .collect();

    if !missing.is_empty() {
        let resolved = futures_util::future::join_all(missing.into_iter().map(
            |(integration_id, connection_id)| {
                let nango = state.nango.clone();
                async move {
                    let identity = tokio::time::timeout(
                        std::time::Duration::from_secs(4),
                        super::identity::fetch_and_store_account_identity(
                            &nango,
                            &integration_id,
                            &connection_id,
                        ),
                    )
                    .await
                    .ok()
                    .flatten();
                    (connection_id, identity)
                }
            },
        ))
        .await;

        for (connection_id, identity) in resolved {
            if let Some(identity) = identity
                && let Some(item) = connections
                    .iter_mut()
                    .find(|item| item.connection_id == connection_id)
            {
                item.account_identity = Some(identity);
            }
        }
    }

    Ok(Json(ListConnectionsResponse { connections }))
}
