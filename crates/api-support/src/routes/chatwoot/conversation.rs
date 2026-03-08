use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};

use crate::error::SupportError;
use crate::state::AppState;

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct CreateConversationRequest {
    pub source_id: String,
    #[serde(default)]
    pub custom_attributes: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateConversationResponse {
    pub conversation_id: i64,
}

#[utoipa::path(
    post,
    path = "/support/chatwoot/conversations",
    request_body = CreateConversationRequest,
    responses(
        (status = 200, description = "Conversation created", body = CreateConversationResponse),
        (status = 500, description = "Chatwoot API error"),
    ),
    tag = "chatwoot",
)]
pub async fn create_conversation(
    State(state): State<AppState>,
    Json(payload): Json<CreateConversationRequest>,
) -> Result<Json<CreateConversationResponse>, SupportError> {
    let inbox_id = &state.config.chatwoot.chatwoot_inbox_identifier;

    let body = hypr_chatwoot::types::PublicConversationCreatePayload {
        ..Default::default()
    };

    let conv = state
        .chatwoot
        .create_a_conversation(inbox_id, &payload.source_id, &body)
        .await
        .map_err(|e| SupportError::Chatwoot(e.to_string()))?
        .into_inner();

    Ok(Json(CreateConversationResponse {
        conversation_id: conv.id.unwrap_or_default(),
    }))
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListConversationsQuery {
    pub source_id: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummary {
    pub id: i64,
    pub inbox_id: Option<String>,
}

#[utoipa::path(
    get,
    path = "/support/chatwoot/conversations",
    params(("source_id" = String, Query, description = "Contact source ID")),
    responses(
        (status = 200, description = "List of conversations", body = Vec<ConversationSummary>),
        (status = 500, description = "Chatwoot API error"),
    ),
    tag = "chatwoot",
)]
pub async fn list_conversations(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<ListConversationsQuery>,
) -> Result<Json<Vec<ConversationSummary>>, SupportError> {
    let inbox_id = &state.config.chatwoot.chatwoot_inbox_identifier;

    let conversations = state
        .chatwoot
        .list_all_contact_conversations(inbox_id, &params.source_id)
        .await
        .map_err(|e| SupportError::Chatwoot(e.to_string()))?
        .into_inner();

    let summaries = conversations
        .into_iter()
        .map(|c| ConversationSummary {
            id: c.id.unwrap_or_default(),
            inbox_id: c.inbox_id.clone(),
        })
        .collect();

    Ok(Json(summaries))
}
