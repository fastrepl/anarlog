use anlg_api_auth::AuthContext;
use anlg_loops::TransactionalEmail;
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};

use super::gateway::canonical_uuid_v4;
use super::{
    INVITATION_TRANSACTIONAL_ID, ListSessionShareAccessRequest, MAX_ACCESS_LIST_RESPONSE_BYTES,
    SessionShareAccessRow, SharedNoteInvitationEmailRequest, SharedNotesState,
};
use crate::error::{Result, SyncError};

#[utoipa::path(
    post,
    path = "/shared-notes/invitations/{invitation_id}/email",
    tag = "shared-notes",
    params(("invitation_id" = String, Path, description = "Session access invitation ID")),
    request_body = SharedNoteInvitationEmailRequest,
    responses(
        (status = 204, description = "Invitation email sent"),
        (status = 400, description = "Invalid invitation email request"),
        (status = 401, description = "Authentication required"),
        (status = 404, description = "Invitation unavailable"),
        (status = 502, description = "Invitation email service unavailable")
    )
)]
pub(super) async fn send_shared_note_invitation_email(
    Extension(auth): Extension<AuthContext>,
    State(state): State<SharedNotesState>,
    Path(invitation_id): Path<String>,
    Json(input): Json<SharedNoteInvitationEmailRequest>,
) -> Result<StatusCode> {
    let invitation_id = canonical_uuid_v4(&invitation_id)
        .ok_or_else(|| SyncError::BadRequest("invalid invitation".to_string()))?;
    let share_id = canonical_uuid_v4(&input.share_id)
        .ok_or_else(|| SyncError::BadRequest("invalid share".to_string()))?;
    if input.invite_token.len() != 43
        || !input
            .invite_token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(SyncError::BadRequest(
            "invalid invitation token".to_string(),
        ));
    }
    let note_title = invitation_note_title(&input.note_title)?;
    let access = list_session_share_access_as_user(&state, &auth, &share_id).await?;
    let recipient = access
        .into_iter()
        .find(|row| {
            row.entry_type == "invitation"
                && row.entry_id == invitation_id
                && row.status == "pending"
        })
        .and_then(|row| row.user_email)
        .filter(|email| is_valid_invitation_email(email))
        .ok_or(SyncError::SharedNoteNotFound)?;
    let sender_name = auth
        .claims
        .email
        .as_deref()
        .filter(|email| is_valid_invitation_email(email))
        .unwrap_or("An Anarlog user")
        .to_string();
    let invitation_url = format!(
        "https://anarlog.so/share/invite/{invitation_id}/#token={}",
        input.invite_token
    );
    let invitation_email = state
        .invitation_email
        .as_ref()
        .ok_or(SyncError::InvitationEmailUnavailable)?;
    invitation_email
        .send_transactional(
            TransactionalEmail {
                email: recipient,
                transactional_id: INVITATION_TRANSACTIONAL_ID.to_string(),
                data_variables: [
                    ("senderName".to_string(), sender_name),
                    ("noteTitle".to_string(), note_title),
                    ("inviteUrl".to_string(), invitation_url),
                ]
                .into_iter()
                .collect(),
            },
            &invitation_id,
        )
        .await
        .map_err(|error| {
            tracing::warn!(%error, invitation_id, "shared-note invitation email failed");
            SyncError::InvitationEmailUnavailable
        })?;
    Ok(StatusCode::NO_CONTENT)
}

fn invitation_note_title(value: &str) -> Result<String> {
    if value.chars().any(char::is_control) {
        return Err(SyncError::BadRequest("invalid note title".to_string()));
    }
    let value = value.trim();
    if value.is_empty() {
        return Ok("Untitled note".to_string());
    }
    let mut title = value.chars().take(160).collect::<String>();
    if value.chars().count() > 160 {
        title.push('…');
    }
    Ok(title)
}

fn is_valid_invitation_email(value: &str) -> bool {
    value.len() <= 320
        && value.trim() == value
        && !value.chars().any(char::is_control)
        && value
            .split_once('@')
            .is_some_and(|(local, domain)| !local.is_empty() && domain.contains('.'))
}

async fn list_session_share_access_as_user(
    state: &SharedNotesState,
    auth: &AuthContext,
    share_id: &str,
) -> Result<Vec<SessionShareAccessRow>> {
    let mut response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/list_session_share_access",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&auth.token)
        .json(&ListSessionShareAccessRequest {
            p_share_id: share_id,
        })
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "shared-note access verification failed");
            SyncError::InvitationEmailUnavailable
        })?;
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > MAX_ACCESS_LIST_RESPONSE_BYTES as u64)
    {
        return Err(SyncError::InvitationEmailUnavailable);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| {
        tracing::warn!(%error, "shared-note access verification response failed");
        SyncError::InvitationEmailUnavailable
    })? {
        if bytes.len().saturating_add(chunk.len()) > MAX_ACCESS_LIST_RESPONSE_BYTES {
            return Err(SyncError::InvitationEmailUnavailable);
        }
        bytes.extend_from_slice(&chunk);
    }
    if !status.is_success() {
        return Err(SyncError::SharedNoteNotFound);
    }
    serde_json::from_slice(&bytes).map_err(|error| {
        tracing::warn!(%error, "shared-note access verification response was invalid");
        SyncError::InvitationEmailUnavailable
    })
}
