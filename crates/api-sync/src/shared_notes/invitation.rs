use anlg_api_auth::AuthContext;
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use utoipa::OpenApi;

use super::gateway::canonical_uuid_v4;
use super::{
    ListSessionShareAccessRequest, MAX_ACCESS_LIST_RESPONSE_BYTES, MeetingRecapEmailRequest,
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
    let owner_email = auth
        .claims
        .email
        .as_deref()
        .filter(|email| is_valid_invitation_email(email))
        .ok_or(SyncError::InvitationEmailUnavailable)?;
    let sender_name = if input.from_name.trim().is_empty() {
        owner_email
    } else {
        &input.from_name
    };
    let workspace_share_slug =
        get_session_share_workspace_slug_as_user(&state, &auth, &share_id).await?;
    let share_origin = workspace_share_origin(workspace_share_slug.as_deref())?;
    let invitation_url = format!(
        "{share_origin}/share/invite/{invitation_id}/#token={}",
        input.invite_token
    );
    let email_delivery = state
        .email_delivery
        .as_ref()
        .ok_or(SyncError::InvitationEmailUnavailable)?;
    email_delivery
        .send_invitation(
            &recipient,
            owner_email,
            sender_name,
            &note_title,
            &invitation_url,
            &invitation_id,
        )
        .await
        .map_err(|error| {
            tracing::warn!(%error, invitation_id, "shared-note invitation email failed");
            SyncError::InvitationEmailUnavailable
        })?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/shared-notes/{share_id}/recap/email",
    tag = "shared-notes",
    params(("share_id" = String, Path, description = "Session share ID")),
    request_body = MeetingRecapEmailRequest,
    responses(
        (status = 204, description = "Meeting recap email sent"),
        (status = 400, description = "Invalid recap email request"),
        (status = 401, description = "Authentication required"),
        (status = 404, description = "Shared note unavailable"),
        (status = 502, description = "Recap email service unavailable")
    )
)]
pub(super) async fn send_shared_note_recap_email(
    Extension(auth): Extension<AuthContext>,
    State(state): State<SharedNotesState>,
    Path(share_id): Path<String>,
    Json(input): Json<MeetingRecapEmailRequest>,
) -> Result<StatusCode> {
    let share_id = canonical_uuid_v4(&share_id)
        .ok_or_else(|| SyncError::BadRequest("invalid share".to_string()))?;
    let delivery_id = canonical_uuid_v4(&input.delivery_id)
        .ok_or_else(|| SyncError::BadRequest("invalid delivery".to_string()))?;
    let recipients = recap_recipients(input.recipients)?;
    let note_title = invitation_note_title(&input.note_title)?;
    let note_body = recap_note_body(&input.note_body)?;
    let owner_email = auth
        .claims
        .email
        .as_deref()
        .filter(|email| is_valid_invitation_email(email))
        .ok_or(SyncError::RecapEmailUnavailable)?;
    let sender_name = if input.sender_name.trim().is_empty() {
        owner_email
    } else {
        &input.sender_name
    };

    list_session_share_access_as_user(&state, &auth, &share_id)
        .await
        .map_err(|error| match error {
            SyncError::SharedNoteNotFound => error,
            _ => SyncError::RecapEmailUnavailable,
        })?;
    let email_delivery = state
        .email_delivery
        .as_ref()
        .ok_or(SyncError::RecapEmailUnavailable)?;
    email_delivery
        .send_recap(
            &recipients,
            owner_email,
            sender_name,
            &note_title,
            note_body,
            &delivery_id,
        )
        .await
        .map_err(|error| {
            tracing::warn!(%error, delivery_id, "shared-note recap email failed");
            SyncError::RecapEmailUnavailable
        })?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(OpenApi)]
#[openapi(
    paths(send_shared_note_recap_email),
    components(schemas(MeetingRecapEmailRequest))
)]
struct RecapApiDoc;

pub(super) fn recap_openapi() -> utoipa::openapi::OpenApi {
    RecapApiDoc::openapi()
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

fn recap_recipients(values: Vec<String>) -> Result<Vec<String>> {
    if values.is_empty() || values.len() > 20 {
        return Err(SyncError::BadRequest(
            "provide between 1 and 20 recipients".to_string(),
        ));
    }
    let mut recipients = Vec::with_capacity(values.len());
    for value in values {
        let value = value.trim().to_string();
        if !is_valid_invitation_email(&value) {
            return Err(SyncError::BadRequest("invalid recipient".to_string()));
        }
        if recipients
            .iter()
            .any(|recipient: &String| recipient.eq_ignore_ascii_case(&value))
        {
            continue;
        }
        recipients.push(value);
    }
    if recipients.is_empty() {
        return Err(SyncError::BadRequest("invalid recipients".to_string()));
    }
    Ok(recipients)
}

fn recap_note_body(value: &str) -> Result<&str> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 100 * 1024
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(SyncError::BadRequest("invalid recap body".to_string()));
    }
    Ok(value)
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

async fn get_session_share_workspace_slug_as_user(
    state: &SharedNotesState,
    auth: &AuthContext,
    share_id: &str,
) -> Result<Option<String>> {
    let mut response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/get_session_share_workspace_slug",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&auth.token)
        .json(&GetSessionShareWorkspaceSlugRequest {
            p_share_id: share_id,
        })
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "shared-note workspace subdomain lookup failed");
            SyncError::InvitationEmailUnavailable
        })?;
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > 1024)
    {
        return Err(SyncError::InvitationEmailUnavailable);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| {
        tracing::warn!(%error, "shared-note workspace subdomain response failed");
        SyncError::InvitationEmailUnavailable
    })? {
        if bytes.len().saturating_add(chunk.len()) > 1024 {
            return Err(SyncError::InvitationEmailUnavailable);
        }
        bytes.extend_from_slice(&chunk);
    }
    if !status.is_success() {
        return Err(SyncError::SharedNoteNotFound);
    }
    let mut rows: Vec<SessionShareWorkspaceSlugRow> =
        serde_json::from_slice(&bytes).map_err(|error| {
            tracing::warn!(%error, "shared-note workspace subdomain response was invalid");
            SyncError::InvitationEmailUnavailable
        })?;
    if rows.len() != 1 {
        return Err(SyncError::InvitationEmailUnavailable);
    }
    Ok(rows.pop().and_then(|row| row.workspace_share_slug))
}

#[derive(serde::Serialize)]
struct GetSessionShareWorkspaceSlugRequest<'a> {
    p_share_id: &'a str,
}

#[derive(serde::Deserialize)]
struct SessionShareWorkspaceSlugRow {
    workspace_share_slug: Option<String>,
}

fn workspace_share_origin(slug: Option<&str>) -> Result<String> {
    let Some(slug) = slug else {
        return Ok("https://anarlog.so".to_string());
    };
    let reserved = [
        "admin", "api", "app", "assets", "auth", "cdn", "dev", "docs", "mail", "staging", "static",
        "status", "support", "www",
    ];
    if !(3..=63).contains(&slug.len())
        || !slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || !slug
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        || !slug
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
        || reserved.contains(&slug)
    {
        return Err(SyncError::InvitationEmailUnavailable);
    }
    Ok(format!("https://{slug}.anarlog.so"))
}
