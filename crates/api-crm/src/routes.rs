use anlg_api_auth::AuthContext;
use anlg_api_nango::NangoConnectionState;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::contacts::{CrmContact, CrmContactQuery, matching_contacts};
use crate::error::{CrmError, Result};
use crate::providers::{MAX_CONTACT_RESULTS, resolve};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CrmSearchContactsRequest {
    pub provider: String,
    pub connection_id: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CrmSearchContactsResponse {
    pub contacts: Vec<CrmContact>,
}

#[utoipa::path(
    post,
    path = "/search-contacts",
    operation_id = "crm_search_contacts",
    request_body = CrmSearchContactsRequest,
    responses(
        (status = 200, description = "Matching CRM contacts", body = CrmSearchContactsResponse),
        (status = 400, description = "Unknown provider or missing query"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "No Nango connection for the provider"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "crm",
)]
pub async fn search_contacts(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<CrmSearchContactsRequest>,
) -> Result<Json<CrmSearchContactsResponse>> {
    let provider = resolve(&req.provider)
        .ok_or_else(|| CrmError::BadRequest(format!("unknown CRM provider: {}", req.provider)))?;
    if req.connection_id.trim().is_empty() {
        return Err(CrmError::BadRequest(
            "connection_id is required".to_string(),
        ));
    }

    let query = CrmContactQuery {
        email: req
            .email
            .map(|email| email.trim().to_string())
            .filter(|email| !email.is_empty()),
        name: req
            .name
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty()),
    };
    if query.email.is_none() && query.name.is_none() {
        return Err(CrmError::BadRequest(
            "email or name is required".to_string(),
        ));
    }

    let http = nango_state
        .build_http_client(
            &auth.token,
            &auth.claims.sub,
            provider.nango_integration_id,
            &req.connection_id,
        )
        .await?;

    let limit = req
        .limit
        .map(|limit| limit as usize)
        .unwrap_or(MAX_CONTACT_RESULTS)
        .clamp(1, 50);
    // Fetch extra candidates upstream: `matching_contacts` may drop provider
    // hits, so requesting only `limit` records can hide later matches.
    let fetch = (limit * 2).clamp(10, 100);
    let contacts = (provider.search)(http, query.clone(), fetch).await?;
    Ok(Json(CrmSearchContactsResponse {
        contacts: matching_contacts(contacts, &query, limit),
    }))
}
