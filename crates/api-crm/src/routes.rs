use anlg_api_auth::AuthContext;
use anlg_api_nango::NangoConnectionState;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::contacts::{
    CrmContact, CrmContactContext, CrmContactQuery, CrmDeal, CrmLoggedNote, CrmNote, CrmNoteInput,
    matching_contacts,
};
use crate::error::{CrmError, Result, map_provider_error};
use crate::providers::{MAX_CONTACT_RESULTS, CrmProvider, resolve};

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

#[derive(Debug, Deserialize, ToSchema)]
pub struct CrmContactContextRequest {
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
#[serde(rename_all = "camelCase")]
pub struct CrmContactContextResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact: Option<CrmContact>,
    pub deals: Vec<CrmDeal>,
    pub notes: Vec<CrmNote>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CrmLogNoteRequest {
    pub provider: String,
    pub connection_id: String,
    #[serde(default)]
    pub contact_id: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub occurred_at: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CrmLogNoteResponse {
    #[serde(flatten)]
    pub logged: CrmLoggedNote,
}

fn validate_provider_connection(provider_id: &str, connection_id: &str) -> Result<&'static CrmProvider> {
    let provider = resolve(provider_id)
        .ok_or_else(|| CrmError::BadRequest(format!("unknown CRM provider: {provider_id}")))?;
    if connection_id.trim().is_empty() {
        return Err(CrmError::BadRequest(
            "connection_id is required".to_string(),
        ));
    }
    Ok(provider)
}

fn build_query(email: Option<String>, name: Option<String>) -> CrmContactQuery {
    CrmContactQuery {
        email: email.map(|v| v.trim().to_string()).filter(|v| !v.is_empty()),
        name: name.map(|v| v.trim().to_string()).filter(|v| !v.is_empty()),
    }
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
        (status = 424, description = "CRM connection requires reconnect"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "crm",
)]
pub async fn search_contacts(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<CrmSearchContactsRequest>,
) -> Result<Json<CrmSearchContactsResponse>> {
    let provider = validate_provider_connection(&req.provider, &req.connection_id)?;
    let query = build_query(req.email, req.name);
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
    let contacts = match (provider.search)(http, query.clone(), fetch).await {
        Ok(contacts) => contacts,
        Err(err) => {
            return Err(map_provider_error(
                &nango_state,
                provider.nango_integration_id,
                &req.connection_id,
                err,
            )
            .await);
        }
    };
    Ok(Json(CrmSearchContactsResponse {
        contacts: matching_contacts(contacts, &query, limit),
    }))
}

#[utoipa::path(
    post,
    path = "/contact-context",
    operation_id = "crm_contact_context",
    request_body = CrmContactContextRequest,
    responses(
        (status = 200, description = "CRM contact profile with recent deals and notes", body = CrmContactContextResponse),
        (status = 400, description = "Unknown provider or missing query"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "No Nango connection for the provider"),
        (status = 424, description = "CRM connection requires reconnect"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "crm",
)]
pub async fn contact_context(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<CrmContactContextRequest>,
) -> Result<Json<CrmContactContextResponse>> {
    let provider = validate_provider_connection(&req.provider, &req.connection_id)?;
    let query = build_query(req.email, req.name);
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
        .clamp(1, 20);
    let CrmContactContext {
        contact,
        deals,
        notes,
    } = match (provider.context)(http, query, limit).await {
        Ok(context) => context,
        Err(err) => {
            return Err(map_provider_error(
                &nango_state,
                provider.nango_integration_id,
                &req.connection_id,
                err,
            )
            .await);
        }
    };
    Ok(Json(CrmContactContextResponse {
        contact,
        deals,
        notes,
    }))
}

#[utoipa::path(
    post,
    path = "/log-note",
    operation_id = "crm_log_note",
    request_body = CrmLogNoteRequest,
    responses(
        (status = 200, description = "Meeting note logged to CRM", body = CrmLogNoteResponse),
        (status = 400, description = "Unknown provider or invalid note request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "No matching contact or connection"),
        (status = 424, description = "CRM connection requires reconnect"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "crm",
)]
pub async fn log_note(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<CrmLogNoteRequest>,
) -> Result<Json<CrmLogNoteResponse>> {
    let provider = validate_provider_connection(&req.provider, &req.connection_id)?;
    let contact_id = req
        .contact_id
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty());
    let query = build_query(req.email, req.name);
    if contact_id.is_none() && query.email.is_none() && query.name.is_none() {
        return Err(CrmError::BadRequest(
            "contact_id, email, or name is required".to_string(),
        ));
    }
    if req.body.trim().is_empty() && req.title.trim().is_empty() {
        return Err(CrmError::BadRequest(
            "title or body is required".to_string(),
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

    let input = CrmNoteInput {
        contact_id,
        query,
        title: req.title,
        body: req.body,
        occurred_at: req.occurred_at,
    };
    let logged = match (provider.log_note)(http, input).await {
        Ok(logged) => logged,
        Err(err) => {
            return Err(map_provider_error(
                &nango_state,
                provider.nango_integration_id,
                &req.connection_id,
                err,
            )
            .await);
        }
    };
    Ok(Json(CrmLogNoteResponse { logged }))
}
