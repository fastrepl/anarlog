use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};

use crate::chatwoot;
use crate::error::SupportError;
use crate::state::AppState;

use super::conversation::ListConversationsQuery;

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageRequest {
    pub content: String,
    #[serde(default = "default_message_type")]
    pub message_type: String,
    #[serde(default)]
    pub source_id: Option<String>,
}

fn default_message_type() -> String {
    "incoming".to_string()
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MessageResponse {
    pub id: String,
    pub content: Option<String>,
    pub message_type: Option<String>,
    pub created_at: Option<String>,
}

#[utoipa::path(
    post,
    path = "/support/chatwoot/conversations/{conversation_id}/messages",
    params(("conversation_id" = i64, Path, description = "Conversation ID")),
    request_body = SendMessageRequest,
    responses(
        (status = 200, description = "Message sent", body = MessageResponse),
        (status = 500, description = "Chatwoot API error"),
    ),
    tag = "chatwoot",
)]
pub async fn send_message(
    State(state): State<AppState>,
    Path(conversation_id): Path<i64>,
    Json(payload): Json<SendMessageRequest>,
) -> Result<Json<MessageResponse>, SupportError> {
    let inbox_id = &state.config.chatwoot.chatwoot_inbox_identifier;

    if payload.message_type == "outgoing" {
        let account_id = state.config.chatwoot.chatwoot_account_id as i64;

        let body = hypr_chatwoot::types::ConversationMessageCreatePayload {
            content: payload.content,
            message_type: Some(
                hypr_chatwoot::types::ConversationMessageCreatePayloadMessageType::Outgoing,
            ),
            private: None,
            content_type: None,
            content_attributes: chatwoot::ai_content_attributes(),
            campaign_id: None,
            template_params: Default::default(),
        };

        let msg = state
            .chatwoot
            .create_a_new_message_in_a_conversation(account_id, conversation_id, &body)
            .await
            .map_err(|e| SupportError::Chatwoot(e.to_string()))?
            .into_inner();

        return Ok(Json(MessageResponse {
            id: msg.id.map(|v| v.to_string()).unwrap_or_default(),
            content: msg.content.clone(),
            message_type: Some("outgoing".to_string()),
            created_at: msg.created_at.map(|v| v.to_string()),
        }));
    }

    let source_id = payload.source_id.as_deref().ok_or_else(|| {
        SupportError::InvalidRequest("source_id required for incoming messages".into())
    })?;

    let body = hypr_chatwoot::types::PublicMessageCreatePayload {
        content: Some(payload.content),
        echo_id: None,
    };

    let msg = state
        .chatwoot
        .create_a_message(inbox_id, source_id, conversation_id, &body)
        .await
        .map_err(|e| SupportError::Chatwoot(e.to_string()))?
        .into_inner();

    Ok(Json(MessageResponse {
        id: msg.id.unwrap_or_default(),
        content: msg.content.clone(),
        message_type: msg.message_type.clone(),
        created_at: msg.created_at.clone(),
    }))
}

#[utoipa::path(
    get,
    path = "/support/chatwoot/conversations/{conversation_id}/messages",
    params(
        ("conversation_id" = i64, Path, description = "Conversation ID"),
        ("source_id" = String, Query, description = "Contact source ID"),
    ),
    responses(
        (status = 200, description = "List of messages", body = Vec<MessageResponse>),
        (status = 500, description = "Chatwoot API error"),
    ),
    tag = "chatwoot",
)]
pub async fn get_messages(
    State(state): State<AppState>,
    Path(conversation_id): Path<i64>,
    axum::extract::Query(params): axum::extract::Query<ListConversationsQuery>,
) -> Result<Json<Vec<MessageResponse>>, SupportError> {
    let inbox_id = &state.config.chatwoot.chatwoot_inbox_identifier;

    // Chatwoot's OpenAPI spec has a typo: "list_all_converation_messages"
    let messages = state
        .chatwoot
        .list_all_converation_messages(inbox_id, &params.source_id, conversation_id)
        .await
        .map_err(|e| SupportError::Chatwoot(e.to_string()))?
        .into_inner();

    let responses = messages
        .into_iter()
        .map(|m| MessageResponse {
            id: m.id.unwrap_or_default(),
            content: m.content.clone(),
            message_type: m.message_type.clone(),
            created_at: m.created_at.clone(),
        })
        .collect();

    Ok(Json(responses))
}
