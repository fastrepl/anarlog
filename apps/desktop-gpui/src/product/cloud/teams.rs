use desktop_runtime::Result;
use serde_json::{Value, json};

use super::{
    Core, account,
    auth::AuthLease,
    choice, field, optional_field, target,
    transport::{failure, first, text, uuid},
};
use crate::product::services::{Action, Mutation, Scope};

impl Core {
    pub(super) async fn team_action(
        &self,
        lease: &AuthLease,
        scope: &Scope,
        mutation: &Mutation,
    ) -> Result<()> {
        let workspace = || -> Result<&str> {
            uuid(
                scope
                    .workspace_id
                    .as_deref()
                    .ok_or_else(|| failure("Choose a workspace"))?,
            )
        };
        let id = || uuid(target(mutation)?);
        let (rpc, body) = match mutation.operation.action {
            Action::CreateWorkspace => (
                "create_workspace",
                json!({"p_name": field(mutation, "name")?}),
            ),
            Action::AcceptInvitation => (
                "accept_my_workspace_invitation",
                json!({"p_invitation_id": id()?}),
            ),
            Action::DeclineInvitation => (
                "decline_my_workspace_invitation",
                json!({"p_invitation_id": id()?}),
            ),
            Action::RevokeInvitation => (
                "revoke_workspace_invitation",
                json!({"p_invitation_id": id()?}),
            ),
            Action::InviteMember | Action::ResendInvitation => {
                let mut response = if mutation.operation.action == Action::InviteMember {
                    self.rpc(lease, "create_workspace_invitation", json!({
                        "p_workspace_id": workspace()?, "p_invitee_email": field(mutation, "email")?.to_lowercase()
                    })).await?
                } else {
                    self.rpc(
                        lease,
                        "resend_workspace_invitation",
                        json!({"p_invitation_id": id()?}),
                    )
                    .await?
                };
                let invitation = uuid(text(first(&response)?, "invitation_id")?)?.to_owned();
                if first(&response)?["invite_token"].as_str().is_none() {
                    response = self
                        .rpc(
                            lease,
                            "resend_workspace_invitation",
                            json!({"p_invitation_id": invitation}),
                        )
                        .await?;
                }
                let token = text(first(&response)?, "invite_token")?;
                let url = self.capability_url(&format!("/team/invite/{invitation}/"), token)?;
                let memberships = self.memberships(lease).await?;
                let name = memberships
                    .as_array()
                    .and_then(|rows| {
                        rows.iter()
                            .find(|row| row["workspace_id"] == workspace().unwrap_or_default())
                    })
                    .and_then(|row| row["workspaces"]["name"].as_str())
                    .ok_or_else(|| failure("Cannot find invitation workspace"))?;
                let sent = self.api(lease, reqwest::Method::POST, &format!("/workspaces/invitations/{invitation}/email"),
                    Some(&json!({"workspaceId":workspace()?, "inviteToken":token, "workspaceName":name, "fromName":lease.identity.email.as_deref().unwrap_or_default()}))).await;
                if sent.is_err() {
                    lease.check()?;
                    if self
                        .host
                        .copy_text(zeroize::Zeroizing::new(url))
                        .await
                        .is_err()
                    {
                        self.rpc(
                            lease,
                            "revoke_workspace_invitation",
                            json!({"p_invitation_id":invitation}),
                        )
                        .await?;
                        return Err(failure(
                            "Email and clipboard unavailable; invitation revoked",
                        ));
                    }
                    *self.notice.lock().await =
                        Some("Email unavailable. Invitation link copied instead".into());
                }
                return Ok(());
            }
            Action::SetMemberRole => (
                "set_workspace_membership_role",
                json!({
                    "p_workspace_id": workspace()?, "p_user_id": id()?,
                    "p_role": choice(mutation, "role", &["admin", "member"])?
                }),
            ),
            Action::TransferOwnership => (
                "transfer_workspace_ownership",
                json!({"p_workspace_id": workspace()?, "p_user_id": id()?}),
            ),
            Action::LeaveWorkspace => ("leave_workspace", json!({"p_workspace_id": workspace()?})),
            Action::DeleteWorkspace => {
                ("delete_workspace", json!({"p_workspace_id": workspace()?}))
            }
            Action::SetSharingSlug => (
                "set_workspace_share_slug",
                json!({"p_workspace_id": workspace()?, "p_slug": field(mutation, "slug")?}),
            ),
            Action::SetWorkspacePolicy => {
                let value = self
                    .rpc(
                        lease,
                        "get_workspace_policy",
                        json!({"p_workspace_id": workspace()?}),
                    )
                    .await?;
                let policy = first(&value)?;
                let retention = match optional_field(mutation, "retention_days") {
                    None => policy["retention_days"].clone(),
                    Some("") => Value::Null,
                    Some(value) => json!(
                        value
                            .parse::<u32>()
                            .map_err(|_| failure("Invalid retention days"))?
                    ),
                };
                let scopes = field(mutation, "allowed_scopes")?
                    .split(',')
                    .map(str::trim)
                    .collect::<Vec<_>>();
                if scopes.is_empty()
                    || scopes
                        .iter()
                        .any(|scope| !["restricted", "workspace", "link", "public"].contains(scope))
                {
                    return Err(failure("Invalid allowed sharing scopes"));
                }
                let default = choice(
                    mutation,
                    "default_scope",
                    &["restricted", "workspace", "link", "public"],
                )?;
                if !scopes.contains(&default) {
                    return Err(failure("Default scope must be allowed"));
                }
                (
                    "set_workspace_policy",
                    json!({
                        "p_workspace_id": workspace()?, "p_allowed_share_scopes": scopes,
                        "p_default_share_scope": default, "p_retention_days": retention,
                        "p_model_training_opt_out": policy_bool(mutation, "training_opt_out", &policy["model_training_opt_out"])?,
                        "p_consent_notification_enabled": policy_bool(mutation, "consent", &policy["consent_notification_enabled"])?,
                        "p_require_sso": policy_bool(mutation, "require_sso", &policy["require_sso"])?
                    }),
                )
            }
            _ => return Err(failure("Invalid workspace action")),
        };
        self.rpc(lease, rpc, body).await?;
        Ok(())
    }

    pub(super) async fn memberships(&self, lease: &AuthLease) -> Result<Value> {
        let mut url = self
            .auth
            .transport
            .config
            .supabase
            .join("/rest/v1/workspace_memberships")
            .map_err(|_| failure("Invalid membership route"))?;
        url.query_pairs_mut()
            .append_pair("select", "workspace_id,role,workspaces(id,name,kind)")
            .append_pair("user_id", &format!("eq.{}", account(lease)?))
            .append_pair("deleted_at", "is.null")
            .append_pair("limit", "128")
            .append_pair("order", "workspace_id");
        let mut rows = Vec::new();
        let mut bytes = 0usize;
        loop {
            let mut page = url.clone();
            page.query_pairs_mut()
                .append_pair("offset", &rows.len().to_string());
            let response = self
                .auth
                .transport
                .send(
                    reqwest::Method::GET,
                    page,
                    Some(lease.access_token()?),
                    None,
                    &[("apikey", self.auth.transport.config.anon_key.clone())],
                )
                .await?;
            lease.check()?;
            let value = super::Transport::checked(response)?;
            let batch = value
                .as_array()
                .ok_or_else(|| failure("Invalid membership list"))?;
            super::transport::check_directory_size(rows.len(), batch, &mut bytes)?;
            rows.extend(batch.iter().cloned());
            if batch.len() < 128 {
                return Ok(Value::Array(rows));
            }
        }
    }
}

fn policy_bool(mutation: &Mutation, field: &str, current: &Value) -> Result<Value> {
    match optional_field(mutation, field) {
        Some("true") => Ok(json!(true)),
        Some("false") => Ok(json!(false)),
        None => Ok(current.clone()),
        _ => Err(failure("Choose true or false")),
    }
}
