use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};

use crate::error::SupportError;
use crate::state::AppState;

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateContactRequest {
    pub identifier: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub custom_attributes: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateContactResponse {
    pub source_id: String,
    pub pubsub_token: String,
}

#[utoipa::path(
    post,
    path = "/support/chatwoot/contact",
    request_body = CreateContactRequest,
    responses(
        (status = 200, description = "Contact created or found", body = CreateContactResponse),
        (status = 500, description = "Chatwoot API error"),
    ),
    tag = "chatwoot",
)]
pub async fn create_contact(
    State(state): State<AppState>,
    Json(payload): Json<CreateContactRequest>,
) -> Result<Json<CreateContactResponse>, SupportError> {
    let inbox_id = &state.config.chatwoot.chatwoot_inbox_identifier;

    let custom_attributes = payload
        .custom_attributes
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    let body = hypr_chatwoot::types::PublicContactCreateUpdatePayload {
        identifier: Some(payload.identifier),
        name: payload.name,
        email: payload.email,
        custom_attributes,
        ..Default::default()
    };

    let contact = state
        .chatwoot
        .create_a_contact(inbox_id, &body)
        .await
        .map_err(|e| SupportError::Chatwoot(e.to_string()))?
        .into_inner();

    Ok(Json(CreateContactResponse {
        source_id: contact.source_id.unwrap_or_default(),
        pubsub_token: contact.pubsub_token.unwrap_or_default(),
    }))
}
