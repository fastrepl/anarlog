use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::{CloudsyncWorkspace, SUPABASE_REQUEST_TIMEOUT, identity::is_valid_e2ee_key_id};
use crate::{
    error::{Result, SyncError},
    state::ReplicaState,
};

const MAX_WORKSPACE_KEY_GRANTS: usize = 1_024;

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
