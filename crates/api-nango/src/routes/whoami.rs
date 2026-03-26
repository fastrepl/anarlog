use axum::{Extension, Json, extract::State};
use hypr_api_auth::AuthContext;
use hypr_nango::OwnedNangoProxy;
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Deserialize)]
struct GoogleUserInfo {
    email: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OutlookMe {
    mail: Option<String>,
    #[serde(rename = "userPrincipalName")]
    user_principal_name: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

// TODO: cache identity information in the database instead of fetching on every request
async fn fetch_identity(
    nango: &hypr_nango::NangoClient,
    integration_id: &str,
    connection_id: &str,
) -> std::result::Result<(Option<String>, Option<String>), String> {
    let proxy = OwnedNangoProxy::new(nango, integration_id.to_string(), connection_id.to_string());

    match integration_id {
        // https://docs.cloud.google.com/identity-platform/docs/reference/rest/v1/UserInfo
        "google-calendar" | "google-drive" => {
            let resp = proxy
                .get("/oauth2/v1/userinfo?alt=json")
                .map_err(|e| e.to_string())?
                .send()
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?;

            let me: GoogleUserInfo = resp.json().await.map_err(|e| e.to_string())?;
            Ok((me.email, me.name))
        }

        // https://learn.microsoft.com/en-us/graph/api/user-get
        "outlook-calendar" => {
            let resp = proxy
                .get("/v1.0/me?$select=mail,userPrincipalName,displayName")
                .map_err(|e| e.to_string())?
                .send()
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?;

            let me: OutlookMe = resp.json().await.map_err(|e| e.to_string())?;
            Ok((me.mail.or(me.user_principal_name), me.display_name))
        }

        other => Err(format!("unsupported integration: {other}")),
    }
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

    let futures = rows.into_iter().map(|row| {
        let nango = state.nango.clone();
        async move {
            if row.status == "reconnect_required" {
                return WhoAmIItem {
                    integration_id: row.integration_id,
                    connection_id: row.connection_id,
                    email: None,
                    display_name: None,
                    error: Some("reconnect_required".to_string()),
                };
            }

            match fetch_identity(&nango, &row.integration_id, &row.connection_id).await {
                Ok((email, display_name)) => WhoAmIItem {
                    integration_id: row.integration_id,
                    connection_id: row.connection_id,
                    email,
                    display_name,
                    error: None,
                },
                Err(e) => {
                    tracing::warn!(
                        integration_id = %row.integration_id,
                        connection_id = %row.connection_id,
                        error = %e,
                        "failed to fetch identity for connection"
                    );
                    WhoAmIItem {
                        integration_id: row.integration_id,
                        connection_id: row.connection_id,
                        email: None,
                        display_name: None,
                        error: Some(e),
                    }
                }
            }
        }
    });

    let accounts = futures_util::future::join_all(futures).await;

    Ok(Json(WhoAmIResponse { accounts }))
}
