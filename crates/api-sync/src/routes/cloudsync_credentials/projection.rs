use std::collections::HashSet;

use anlg_api_auth::AuthContext;
use serde::{Deserialize, Serialize};

use super::SUPABASE_REQUEST_TIMEOUT;
use crate::{
    error::{Result, SyncError},
    state::ReplicaState,
};

pub(in crate::routes) const WORKSPACE_PROJECTION_SELECT: &str = "id,user_id,role,created_at,updated_at,workspace:workspaces!inner(id,owner_user_id,kind,name,created_at,updated_at)";
pub(in crate::routes) const MAX_TOKEN_WORKSPACES: usize = 128;
pub(in crate::routes) const MAX_TOKEN_ATTRIBUTES_BYTES: usize = 8 * 1024;

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CloudsyncWorkspace {
    pub(in crate::routes) id: String,
    pub(in crate::routes) owner_user_id: String,
    pub(in crate::routes) kind: String,
    pub(in crate::routes) name: String,
    pub(in crate::routes) membership_id: String,
    pub(in crate::routes) role: String,
    pub(in crate::routes) membership_created_at: String,
    pub(in crate::routes) membership_updated_at: String,
    pub(in crate::routes) created_at: String,
    pub(in crate::routes) updated_at: String,
}

#[derive(Deserialize)]
pub(super) struct WorkspaceMembershipRow {
    id: String,
    user_id: String,
    role: String,
    created_at: String,
    updated_at: String,
    workspace: WorkspaceRow,
}

#[derive(Deserialize)]
pub(super) struct WorkspaceRow {
    id: String,
    owner_user_id: String,
    kind: String,
    name: String,
    created_at: String,
    updated_at: String,
}

pub(super) async fn fetch_workspace_projection(
    state: &ReplicaState,
    auth: &AuthContext,
) -> Result<Vec<WorkspaceMembershipRow>> {
    let user_filter = format!("eq.{}", auth.claims.sub);
    let response = state
        .client
        .get(format!(
            "{}/rest/v1/workspace_memberships",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_anon_key)
        .bearer_auth(&auth.token)
        .timeout(SUPABASE_REQUEST_TIMEOUT)
        .query(&[
            ("select", WORKSPACE_PROJECTION_SELECT),
            ("user_id", user_filter.as_str()),
            ("deleted_at", "is.null"),
            ("workspace.deleted_at", "is.null"),
        ])
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase workspace projection request failed");
            SyncError::Upstream
        })?;
    if !response.status().is_success() {
        tracing::warn!(
            status = %response.status(),
            "Supabase workspace projection request was rejected"
        );
        return Err(SyncError::Upstream);
    }

    response.json().await.map_err(|error| {
        tracing::warn!(%error, "Supabase workspace projection response was invalid");
        SyncError::Upstream
    })
}

pub(super) fn validate_workspace_projection(
    mut rows: Vec<WorkspaceMembershipRow>,
    account_user_id: &str,
) -> Result<(String, Vec<CloudsyncWorkspace>)> {
    let mut membership_ids = HashSet::with_capacity(rows.len());
    let mut workspace_ids = HashSet::with_capacity(rows.len());

    for row in &rows {
        if row.user_id != account_user_id {
            return invalid_workspace_projection("membership user does not match account");
        }
        if row.id.trim().is_empty()
            || row.workspace.id.trim().is_empty()
            || row.workspace.owner_user_id.trim().is_empty()
        {
            return invalid_workspace_projection("workspace projection contains a blank identity");
        }
        if !matches!(row.role.as_str(), "owner" | "admin" | "member") {
            return invalid_workspace_projection("workspace membership has an invalid role");
        }
        if !matches!(row.workspace.kind.as_str(), "personal" | "shared") {
            return invalid_workspace_projection("workspace has an invalid kind");
        }
        if chrono::DateTime::parse_from_rfc3339(&row.workspace.created_at).is_err()
            || chrono::DateTime::parse_from_rfc3339(&row.workspace.updated_at).is_err()
        {
            return invalid_workspace_projection("workspace has an invalid timestamp");
        }
        if chrono::DateTime::parse_from_rfc3339(&row.created_at).is_err()
            || chrono::DateTime::parse_from_rfc3339(&row.updated_at).is_err()
        {
            return invalid_workspace_projection("workspace membership has an invalid timestamp");
        }
        if !membership_ids.insert(&row.id) || !workspace_ids.insert(&row.workspace.id) {
            return invalid_workspace_projection("workspace projection contains duplicate rows");
        }
    }

    let personal_workspaces = rows
        .iter()
        .filter(|row| row.workspace.kind == "personal")
        .collect::<Vec<_>>();
    if personal_workspaces.len() != 1 {
        return invalid_workspace_projection(
            "account must have exactly one personal owner workspace",
        );
    }

    let personal_workspace = personal_workspaces[0];
    if personal_workspace.role != "owner"
        || personal_workspace.workspace.id != account_user_id
        || personal_workspace.workspace.owner_user_id != account_user_id
    {
        return invalid_workspace_projection("personal workspace identity does not match account");
    }

    let personal_workspace_id = personal_workspace.workspace.id.clone();
    rows.sort_by(|left, right| {
        let left_is_personal = left.workspace.id == personal_workspace_id;
        let right_is_personal = right.workspace.id == personal_workspace_id;
        right_is_personal
            .cmp(&left_is_personal)
            .then_with(|| left.workspace.created_at.cmp(&right.workspace.created_at))
            .then_with(|| left.workspace.id.cmp(&right.workspace.id))
    });

    Ok((
        personal_workspace_id,
        rows.into_iter()
            .map(|row| CloudsyncWorkspace {
                id: row.workspace.id,
                owner_user_id: row.workspace.owner_user_id,
                kind: row.workspace.kind,
                name: row.workspace.name,
                membership_id: row.id,
                role: row.role,
                membership_created_at: row.created_at,
                membership_updated_at: row.updated_at,
                created_at: row.workspace.created_at,
                updated_at: row.workspace.updated_at,
            })
            .collect(),
    ))
}

fn invalid_workspace_projection<T>(reason: &'static str) -> Result<T> {
    tracing::warn!(reason, "Supabase workspace projection failed validation");
    Err(SyncError::Upstream)
}

pub(in crate::routes) fn encode_workspace_token_attributes(
    workspaces: &[CloudsyncWorkspace],
) -> Result<String> {
    if workspaces.len() > MAX_TOKEN_WORKSPACES {
        tracing::warn!(
            workspace_count = workspaces.len(),
            "CloudSync workspace projection exceeds token limit"
        );
        return Err(SyncError::Upstream);
    }

    let attributes = serde_json::to_string(&serde_json::json!({
        "workspace_ids": workspaces
            .iter()
            .map(|workspace| workspace.id.as_str())
            .collect::<Vec<_>>(),
    }))
    .map_err(|error| {
        SyncError::Internal(format!("CloudSync token attributes are invalid: {error}"))
    })?;
    if attributes.len() > MAX_TOKEN_ATTRIBUTES_BYTES {
        tracing::warn!(
            attribute_bytes = attributes.len(),
            "CloudSync workspace projection exceeds token size limit"
        );
        return Err(SyncError::Upstream);
    }

    Ok(attributes)
}
