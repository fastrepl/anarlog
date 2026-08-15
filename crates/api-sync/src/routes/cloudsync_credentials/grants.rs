use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::{CloudsyncWorkspace, SUPABASE_REQUEST_TIMEOUT, identity::is_valid_e2ee_key_id};
use crate::{
    error::{Result, SyncError},
    state::ReplicaState,
};

const MAX_WORKSPACE_KEY_GRANTS: usize = 1_024;
const MAX_WORKSPACE_KEY_RECIPIENTS: usize = 256;

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceE2eeKeyGrant {
    workspace_id: String,
    key_id: String,
    ephemeral_public_key: String,
    nonce: String,
    ciphertext: String,
    is_active: bool,
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceE2eeKeyRecipient {
    #[serde(alias = "user_id")]
    user_id: String,
    #[serde(alias = "user_email")]
    user_email: String,
    role: String,
    #[serde(alias = "public_key")]
    public_key: Option<String>,
    #[serde(alias = "granted_key_ids")]
    granted_key_ids: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceE2eeKeyGrantUpload {
    user_id: String,
    ephemeral_public_key: String,
    nonce: String,
    ciphertext: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetWorkspaceE2eeKeyRequest {
    pub(super) key_id: String,
    pub(super) grants: Vec<WorkspaceE2eeKeyGrantUpload>,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetWorkspaceE2eeKeyResult {
    key_id: String,
    granted_member_count: i64,
}

#[derive(Serialize)]
struct ListWorkspaceKeyRecipientsRpcRequest<'a> {
    p_workspace_id: &'a str,
}

#[derive(Serialize)]
struct SetWorkspaceE2eeKeyRpcRequest<'a> {
    p_workspace_id: &'a str,
    p_key_id: &'a str,
    p_grants: &'a [WorkspaceE2eeKeyGrantUpload],
}

#[derive(Deserialize)]
struct SetWorkspaceE2eeKeyRow {
    key_id: String,
    granted_member_count: i64,
}

pub(super) async fn fetch_workspace_key_recipients(
    state: &ReplicaState,
    access_token: &str,
    workspace_id: &str,
) -> Result<Vec<WorkspaceE2eeKeyRecipient>> {
    validate_workspace_id(workspace_id)?;
    let response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/list_workspace_key_recipients",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_anon_key)
        .bearer_auth(access_token)
        .timeout(SUPABASE_REQUEST_TIMEOUT)
        .json(&ListWorkspaceKeyRecipientsRpcRequest {
            p_workspace_id: workspace_id,
        })
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase workspace E2EE recipients request failed");
            SyncError::Upstream
        })?;
    if !response.status().is_success() {
        tracing::warn!(
            status = %response.status(),
            "Supabase workspace E2EE recipients request was rejected"
        );
        return Err(SyncError::Upstream);
    }
    let recipients = response
        .json::<Vec<WorkspaceE2eeKeyRecipient>>()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase workspace E2EE recipients response was invalid");
            SyncError::Upstream
        })?;
    validate_workspace_key_recipients(recipients)
}

pub(super) async fn publish_workspace_e2ee_key(
    state: &ReplicaState,
    access_token: &str,
    workspace_id: &str,
    request: SetWorkspaceE2eeKeyRequest,
) -> Result<SetWorkspaceE2eeKeyResult> {
    validate_workspace_id(workspace_id)?;
    validate_workspace_key_grant_uploads(&request)?;
    let response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/set_workspace_e2ee_key",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_anon_key)
        .bearer_auth(access_token)
        .timeout(SUPABASE_REQUEST_TIMEOUT)
        .json(&SetWorkspaceE2eeKeyRpcRequest {
            p_workspace_id: workspace_id,
            p_key_id: &request.key_id,
            p_grants: &request.grants,
        })
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase workspace E2EE key publication failed");
            SyncError::Upstream
        })?;
    if !response.status().is_success() {
        tracing::warn!(
            status = %response.status(),
            "Supabase workspace E2EE key publication was rejected"
        );
        return Err(SyncError::Upstream);
    }
    let mut rows = response
        .json::<Vec<SetWorkspaceE2eeKeyRow>>()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase workspace E2EE key response was invalid");
            SyncError::Upstream
        })?;
    if rows.len() != 1
        || rows[0].key_id != request.key_id
        || !(1..=MAX_WORKSPACE_KEY_RECIPIENTS as i64).contains(&rows[0].granted_member_count)
    {
        tracing::warn!("Supabase workspace E2EE key response failed validation");
        return Err(SyncError::Upstream);
    }
    let row = rows.pop().expect("row count was checked");
    Ok(SetWorkspaceE2eeKeyResult {
        key_id: row.key_id,
        granted_member_count: row.granted_member_count,
    })
}

#[derive(Deserialize)]
struct WorkspaceE2eeKeyGrantRow {
    workspace_id: String,
    key_id: String,
    ephemeral_public_key: String,
    nonce: String,
    ciphertext: String,
    is_active: bool,
}

pub(super) async fn fetch_workspace_key_grants(
    state: &ReplicaState,
    access_token: &str,
    workspaces: &[CloudsyncWorkspace],
) -> Result<Vec<WorkspaceE2eeKeyGrant>> {
    let shared_workspace_ids = workspaces
        .iter()
        .filter(|workspace| workspace.kind == "shared")
        .map(|workspace| workspace.id.as_str())
        .collect::<HashSet<_>>();
    if shared_workspace_ids.is_empty() {
        return Ok(Vec::new());
    }

    let response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/list_all_my_workspace_e2ee_grants",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_anon_key)
        .bearer_auth(access_token)
        .timeout(SUPABASE_REQUEST_TIMEOUT)
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase workspace E2EE grants request failed");
            SyncError::Upstream
        })?;
    if !response.status().is_success() {
        tracing::warn!(
            status = %response.status(),
            "Supabase workspace E2EE grants request was rejected"
        );
        return Err(SyncError::Upstream);
    }
    let rows = response
        .json::<Vec<WorkspaceE2eeKeyGrantRow>>()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase workspace E2EE grants response was invalid");
            SyncError::Upstream
        })?;
    validate_workspace_key_grants(rows, &shared_workspace_ids)
}

fn validate_workspace_key_grants(
    rows: Vec<WorkspaceE2eeKeyGrantRow>,
    shared_workspace_ids: &HashSet<&str>,
) -> Result<Vec<WorkspaceE2eeKeyGrant>> {
    if rows.len() > MAX_WORKSPACE_KEY_GRANTS {
        return Err(SyncError::Upstream);
    }
    let mut identities = HashSet::with_capacity(rows.len());
    let mut active_workspaces = HashSet::new();
    let mut grants = Vec::with_capacity(rows.len());
    for row in rows {
        if !shared_workspace_ids.contains(row.workspace_id.as_str())
            || !is_valid_e2ee_key_id(&row.key_id)
            || !is_valid_base64url(&row.ephemeral_public_key, 43)
            || !is_valid_base64url(&row.nonce, 32)
            || !is_valid_base64url(&row.ciphertext, 64)
            || !identities.insert((row.workspace_id.clone(), row.key_id.clone()))
            || (row.is_active && !active_workspaces.insert(row.workspace_id.clone()))
        {
            tracing::warn!("Supabase workspace E2EE grants response failed validation");
            return Err(SyncError::Upstream);
        }
        grants.push(WorkspaceE2eeKeyGrant {
            workspace_id: row.workspace_id,
            key_id: row.key_id,
            ephemeral_public_key: row.ephemeral_public_key,
            nonce: row.nonce,
            ciphertext: row.ciphertext,
            is_active: row.is_active,
        });
    }
    Ok(grants)
}

fn is_valid_base64url(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn validate_workspace_id(workspace_id: &str) -> Result<()> {
    uuid::Uuid::parse_str(workspace_id)
        .map(|_| ())
        .map_err(|_| SyncError::BadRequest("Workspace ID is invalid".to_string()))
}

fn validate_workspace_key_recipients(
    recipients: Vec<WorkspaceE2eeKeyRecipient>,
) -> Result<Vec<WorkspaceE2eeKeyRecipient>> {
    if recipients.is_empty() || recipients.len() > MAX_WORKSPACE_KEY_RECIPIENTS {
        return Err(SyncError::Upstream);
    }
    let mut user_ids = HashSet::with_capacity(recipients.len());
    for recipient in &recipients {
        let mut granted_key_ids = HashSet::with_capacity(recipient.granted_key_ids.len());
        if uuid::Uuid::parse_str(&recipient.user_id).is_err()
            || !user_ids.insert(recipient.user_id.as_str())
            || recipient.user_email.trim().is_empty()
            || recipient.user_email.len() > 320
            || !matches!(recipient.role.as_str(), "owner" | "admin" | "member")
            || recipient
                .public_key
                .as_ref()
                .is_some_and(|key| !is_valid_base64url(key, 43))
            || recipient.granted_key_ids.iter().any(|key_id| {
                !is_valid_e2ee_key_id(key_id) || !granted_key_ids.insert(key_id.as_str())
            })
        {
            tracing::warn!("Supabase workspace E2EE recipients response failed validation");
            return Err(SyncError::Upstream);
        }
    }
    Ok(recipients)
}

fn validate_workspace_key_grant_uploads(request: &SetWorkspaceE2eeKeyRequest) -> Result<()> {
    if !is_valid_e2ee_key_id(&request.key_id)
        || request.grants.is_empty()
        || request.grants.len() > MAX_WORKSPACE_KEY_RECIPIENTS
    {
        return Err(SyncError::BadRequest(
            "Workspace E2EE key grants are invalid".to_string(),
        ));
    }
    let mut user_ids = HashSet::with_capacity(request.grants.len());
    if request.grants.iter().any(|grant| {
        uuid::Uuid::parse_str(&grant.user_id).is_err()
            || !user_ids.insert(grant.user_id.as_str())
            || !is_valid_base64url(&grant.ephemeral_public_key, 43)
            || !is_valid_base64url(&grant.nonce, 32)
            || !is_valid_base64url(&grant.ciphertext, 64)
    }) {
        return Err(SyncError::BadRequest(
            "Workspace E2EE key grants are invalid".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(workspace_id: &str, key_id: &str, is_active: bool) -> WorkspaceE2eeKeyGrantRow {
        WorkspaceE2eeKeyGrantRow {
            workspace_id: workspace_id.to_string(),
            key_id: key_id.to_string(),
            ephemeral_public_key: "A".repeat(43),
            nonce: "B".repeat(32),
            ciphertext: "C".repeat(64),
            is_active,
        }
    }

    #[test]
    fn rejects_unknown_workspaces_duplicate_generations_and_multiple_active_keys() {
        let shared = HashSet::from(["workspace-a"]);
        assert!(
            validate_workspace_key_grants(
                vec![row("workspace-b", "AAAAAAAAAAAAAAAAAAAAAA", true)],
                &shared,
            )
            .is_err()
        );
        assert!(
            validate_workspace_key_grants(
                vec![
                    row("workspace-a", "AAAAAAAAAAAAAAAAAAAAAA", false),
                    row("workspace-a", "AAAAAAAAAAAAAAAAAAAAAA", true),
                ],
                &shared,
            )
            .is_err()
        );
        assert!(
            validate_workspace_key_grants(
                vec![
                    row("workspace-a", "AAAAAAAAAAAAAAAAAAAAAA", true),
                    row("workspace-a", "BBBBBBBBBBBBBBBBBBBBBB", true),
                ],
                &shared,
            )
            .is_err()
        );
    }
}
