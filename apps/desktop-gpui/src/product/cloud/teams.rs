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

/// Shipping's `settings/team/client.ts`, over `CloudCalls`. Responses are validated like the
/// TypeScript client; server messages are surfaced because they explain seat or permission
/// failures.
pub mod client {
    use std::sync::Arc;

    use desktop_runtime::Result;
    use reqwest::Method;
    use serde_json::{Value, json};
    use zeroize::Zeroizing;

    use super::super::{
        calls::CloudCalls,
        transport::{failure, first, text, uuid},
    };
    use crate::product::account::WorkspaceTier;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Role {
        Owner,
        Admin,
        Member,
    }

    impl Role {
        pub fn parse(value: &Value) -> Result<Self> {
            match value.as_str() {
                Some("owner") => Ok(Self::Owner),
                Some("admin") => Ok(Self::Admin),
                Some("member") => Ok(Self::Member),
                _ => Err(failure("Workspace request failed")),
            }
        }

        pub fn as_str(self) -> &'static str {
            match self {
                Self::Owner => "owner",
                Self::Admin => "admin",
                Self::Member => "member",
            }
        }

        pub fn label(self) -> &'static str {
            match self {
                Self::Owner => "Owner",
                Self::Admin => "Admin",
                Self::Member => "Member",
            }
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct MyWorkspace {
        pub id: Arc<str>,
        pub name: Arc<str>,
        pub owner_user_id: Arc<str>,
        pub share_slug: Option<Arc<str>>,
        pub logo: Option<Arc<str>>,
        pub role: Role,
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct MyInvitation {
        pub invitation_id: Arc<str>,
        pub workspace_id: Arc<str>,
        pub workspace_name: Arc<str>,
        pub logo: Option<Arc<str>>,
        pub invited_by_email: Option<Arc<str>>,
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct Member {
        pub user_id: Arc<str>,
        pub email: Arc<str>,
        pub name: Option<Arc<str>>,
        pub role: Role,
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct Invitation {
        pub invitation_id: Arc<str>,
        pub email: Arc<str>,
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct Access {
        pub role: Role,
        pub tier: WorkspaceTier,
        pub capabilities: Vec<String>,
        pub used_seats: u32,
    }

    impl Access {
        pub fn has(&self, capability: &str) -> bool {
            self.capabilities.iter().any(|value| value == capability)
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct OwnershipRequest {
        pub id: Arc<str>,
        pub owner_user_id: Arc<str>,
        pub target_user_id: Arc<str>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct Usage {
        pub member_count: u64,
        pub enrolled_devices: u64,
        pub shares_created_30d: u64,
        pub seat_limit: Option<u64>,
        pub used_seats: u64,
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct Policy {
        pub allowed_share_scopes: Vec<String>,
        pub default_share_scope: String,
        pub retention_days: Option<u64>,
        pub model_training_opt_out: bool,
        pub consent_notification_enabled: bool,
        pub require_sso: bool,
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct AutoJoin {
        pub domain: Option<Arc<str>>,
        pub enabled: bool,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum SlugAvailability {
        Available,
        Taken,
        Invalid,
    }

    pub enum Delivery {
        Email,
        Clipboard,
    }

    fn rows(value: &Value) -> Result<&[Value]> {
        value
            .as_array()
            .map(Vec::as_slice)
            .ok_or_else(|| failure("Workspace request failed"))
    }

    fn optional(value: &Value) -> Option<Arc<str>> {
        value.as_str().map(Arc::from)
    }

    fn id(value: &Value, key: &str) -> Result<Arc<str>> {
        Ok(uuid(text(value, key)?)?.into())
    }

    pub async fn list_my_workspaces(calls: &dyn CloudCalls) -> Result<Vec<MyWorkspace>> {
        let value = calls
            .select(
                "workspaces",
                vec![
                    (
                        "select",
                        "id,name,kind,owner_user_id,share_slug,logo_data,workspace_memberships(role)"
                            .into(),
                    ),
                    ("kind", "eq.shared".into()),
                ],
            )
            .await?;
        rows(&value)?
            .iter()
            .map(|row| {
                Ok(MyWorkspace {
                    id: id(row, "id")?,
                    name: text(row, "name")?.into(),
                    owner_user_id: text(row, "owner_user_id")?.into(),
                    share_slug: optional(&row["share_slug"]),
                    logo: optional(&row["logo_data"]),
                    role: match &row["workspace_memberships"][0]["role"] {
                        Value::Null => Role::Member,
                        role => Role::parse(role)?,
                    },
                })
            })
            .collect()
    }

    pub async fn list_my_invitations(calls: &dyn CloudCalls) -> Result<Vec<MyInvitation>> {
        let value = calls
            .rpc("list_my_workspace_invitations", json!({}))
            .await?;
        rows(&value)?
            .iter()
            .map(|row| {
                Ok(MyInvitation {
                    invitation_id: id(row, "invitation_id")?,
                    workspace_id: id(row, "workspace_id")?,
                    workspace_name: text(row, "workspace_name")?.into(),
                    logo: optional(&row["workspace_logo_data"]),
                    invited_by_email: optional(&row["invited_by_email"]),
                })
            })
            .collect()
    }

    pub async fn members(calls: &dyn CloudCalls, workspace: &str) -> Result<Vec<Member>> {
        let value = calls
            .rpc(
                "list_workspace_members_with_profiles",
                json!({"p_workspace_id": uuid(workspace)?}),
            )
            .await?;
        rows(&value)?
            .iter()
            .filter(|row| row["deleted_at"].is_null())
            .map(|row| {
                Ok(Member {
                    user_id: text(row, "user_id")?.into(),
                    email: text(row, "user_email")?.into(),
                    name: optional(&row["user_name"]).filter(|name| !name.is_empty()),
                    role: Role::parse(&row["role"])?,
                })
            })
            .collect()
    }

    pub async fn invitations(calls: &dyn CloudCalls, workspace: &str) -> Result<Vec<Invitation>> {
        let value = calls
            .rpc(
                "list_workspace_invitations",
                json!({"p_workspace_id": uuid(workspace)?}),
            )
            .await?;
        rows(&value)?
            .iter()
            .filter(|row| row["accepted_at"].is_null() && row["revoked_at"].is_null())
            .map(|row| {
                Ok(Invitation {
                    invitation_id: id(row, "invitation_id")?,
                    email: text(row, "invitee_email")?.into(),
                })
            })
            .collect()
    }

    pub async fn access(calls: &dyn CloudCalls, workspace: &str) -> Result<Access> {
        let value = calls
            .rpc(
                "get_workspace_access",
                json!({"p_workspace_id": uuid(workspace)?}),
            )
            .await?;
        let row = first(&value)?;
        Ok(Access {
            role: Role::parse(&row["workspace_role"])?,
            tier: row["workspace_tier"]
                .as_str()
                .and_then(WorkspaceTier::parse)
                .ok_or_else(|| failure("Workspace request failed"))?,
            capabilities: row["capabilities"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|value| value.as_str().map(String::from))
                .collect(),
            used_seats: row["used_seats"].as_u64().unwrap_or(0) as u32,
        })
    }

    pub async fn ownership_requests(
        calls: &dyn CloudCalls,
        workspace: &str,
    ) -> Result<Vec<OwnershipRequest>> {
        let value = calls
            .rpc(
                "list_workspace_ownership_requests",
                json!({"p_workspace_id": uuid(workspace)?}),
            )
            .await?;
        rows(&value)?
            .iter()
            .map(|row| {
                Ok(OwnershipRequest {
                    id: text(row, "id")?.into(),
                    owner_user_id: text(row, "owner_user_id")?.into(),
                    target_user_id: text(row, "target_user_id")?.into(),
                })
            })
            .collect()
    }

    pub async fn usage(calls: &dyn CloudCalls, workspace: &str) -> Result<Usage> {
        let value = calls
            .rpc(
                "get_workspace_usage_overview",
                json!({"p_workspace_id": uuid(workspace)?}),
            )
            .await?;
        let row = first(&value)?;
        let number = |key: &str| row[key].as_u64().unwrap_or(0);
        Ok(Usage {
            member_count: number("member_count"),
            enrolled_devices: number("enrolled_devices"),
            shares_created_30d: number("shares_created_30d"),
            seat_limit: row["seat_limit"].as_u64(),
            used_seats: number("used_seats"),
        })
    }

    pub async fn policy(calls: &dyn CloudCalls, workspace: &str) -> Result<Policy> {
        let value = calls
            .rpc(
                "get_workspace_policy",
                json!({"p_workspace_id": uuid(workspace)?}),
            )
            .await?;
        let row = first(&value)?;
        let scope = |value: &Value| match value.as_str() {
            Some(scope @ ("restricted" | "workspace" | "link" | "public")) => Ok(scope.to_owned()),
            _ => Err(failure("Workspace request failed")),
        };
        Ok(Policy {
            allowed_share_scopes: match row["allowed_share_scopes"].as_array() {
                Some(scopes) => scopes.iter().map(scope).collect::<Result<_>>()?,
                None => vec!["restricted".into()],
            },
            default_share_scope: scope(&row["default_share_scope"])?,
            retention_days: row["retention_days"].as_u64(),
            model_training_opt_out: row["model_training_opt_out"] != false,
            consent_notification_enabled: row["consent_notification_enabled"] != false,
            require_sso: row["require_sso"] == true,
        })
    }

    pub async fn set_policy(
        calls: &dyn CloudCalls,
        workspace: &str,
        policy: &Policy,
    ) -> Result<()> {
        calls
            .rpc(
                "set_workspace_policy",
                json!({
                    "p_workspace_id": uuid(workspace)?,
                    "p_allowed_share_scopes": policy.allowed_share_scopes,
                    "p_default_share_scope": policy.default_share_scope,
                    "p_retention_days": policy.retention_days,
                    "p_model_training_opt_out": policy.model_training_opt_out,
                    "p_consent_notification_enabled": policy.consent_notification_enabled,
                    "p_require_sso": policy.require_sso,
                }),
            )
            .await
            .map(|_| ())
    }

    pub async fn auto_join(calls: &dyn CloudCalls, workspace: &str) -> Result<AutoJoin> {
        let value = calls
            .rpc(
                "get_workspace_email_auto_join",
                json!({"p_workspace_id": uuid(workspace)?}),
            )
            .await?;
        let row = first(&value)?;
        Ok(AutoJoin {
            domain: optional(&row["domain"]),
            enabled: row["enabled"] == true,
        })
    }

    pub async fn slug_availability(
        calls: &dyn CloudCalls,
        workspace: &str,
        slug: &str,
    ) -> Result<SlugAvailability> {
        let value = calls
            .rpc(
                "check_workspace_share_slug_availability",
                json!({"p_workspace_id": uuid(workspace)?, "p_slug": slug}),
            )
            .await?;
        match value.as_str() {
            Some("available") => Ok(SlugAvailability::Available),
            Some("taken") => Ok(SlugAvailability::Taken),
            Some("invalid") => Ok(SlugAvailability::Invalid),
            _ => Err(failure("Workspace request failed")),
        }
    }

    /// Accepting returns the workspace joined.
    pub async fn accept_invitation(calls: &dyn CloudCalls, invitation: &str) -> Result<Arc<str>> {
        let value = calls
            .rpc(
                "accept_my_workspace_invitation",
                json!({"p_invitation_id": uuid(invitation)?}),
            )
            .await?;
        id(first(&value)?, "workspace_id")
    }

    pub async fn create_workspace(calls: &dyn CloudCalls, name: &str) -> Result<Arc<str>> {
        let value = calls
            .rpc("create_workspace", json!({"p_name": name}))
            .await?;
        id(first(&value)?, "workspace_id")
    }

    /// Calls a workspace RPC whose result the page ignores.
    pub async fn call(calls: &dyn CloudCalls, rpc: &'static str, body: Value) -> Result<()> {
        calls.rpc(rpc, body).await.map(|_| ())
    }

    fn valid_token(token: &str) -> bool {
        token.len() == 43
            && token
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    }

    /// `deliverWorkspaceInvitation`: create (or re-issue) the invitation, email it, and fall
    /// back to copying the link. If both fail the invitation is revoked.
    pub async fn deliver_invitation(
        calls: &dyn CloudCalls,
        workspace: &str,
        workspace_name: &str,
        email: &str,
        sender: &str,
    ) -> Result<Delivery> {
        let email = email.trim().to_lowercase();
        if email.is_empty()
            || email.len() > 320
            || email.chars().any(|c| c.is_whitespace() || c.is_control())
            || email
                .split_once('@')
                .is_none_or(|(user, host)| user.is_empty() || host.is_empty() || host.contains('@'))
        {
            return Err(failure("Workspace request failed"));
        }
        let created = calls
            .rpc(
                "create_workspace_invitation",
                json!({"p_workspace_id": uuid(workspace)?, "p_invitee_email": email}),
            )
            .await?;
        let mut row = first(&created)?.clone();
        let invitation = id(&row, "invitation_id")?;
        if row["invite_token"].is_null() {
            let resent = calls
                .rpc(
                    "resend_workspace_invitation",
                    json!({"p_invitation_id": invitation.as_ref()}),
                )
                .await?;
            row = first(&resent)?.clone();
        }
        let invitation = id(&row, "invitation_id")?;
        let token = text(&row, "invite_token")?.to_owned();
        if !valid_token(&token) {
            return Err(failure("Workspace request failed"));
        }
        let sent = calls
            .api(
                Method::POST,
                format!("/workspaces/invitations/{invitation}/email"),
                Some(json!({
                    "workspaceId": workspace,
                    "inviteToken": token,
                    "workspaceName": workspace_name.trim(),
                    "fromName": sender.trim(),
                })),
            )
            .await;
        if sent.is_ok() {
            return Ok(Delivery::Email);
        }
        let copied = match calls.invitation_url(&invitation, &token) {
            Ok(url) => calls.copy_text(Zeroizing::new(url)).await,
            Err(error) => Err(error),
        };
        if copied.is_ok() {
            return Ok(Delivery::Clipboard);
        }
        let _ = calls
            .rpc(
                "revoke_workspace_invitation",
                json!({"p_invitation_id": invitation.as_ref()}),
            )
            .await;
        Err(failure("Workspace request failed"))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use desktop_runtime::{Result, ServiceError};
    use futures::future::BoxFuture;
    use serde_json::{Value, json};
    use zeroize::Zeroizing;

    use super::{
        super::calls::{CloudCalls, SyncCommand, SyncState},
        client,
    };

    /// Creates invitations, but neither email nor the clipboard works.
    #[derive(Default)]
    struct Offline {
        revoked: Mutex<Vec<Value>>,
    }

    impl CloudCalls for Offline {
        fn rpc(&self, name: &'static str, body: Value) -> BoxFuture<'static, Result<Value>> {
            let value = match name {
                "create_workspace_invitation" => json!([{
                    "invitation_id": "6e7f8a9b-0c1d-4e2f-8a3b-4c5d6e7f8a9b",
                    "invite_token": "a".repeat(43),
                }]),
                "revoke_workspace_invitation" => {
                    self.revoked.lock().unwrap().push(body);
                    json!(null)
                }
                _ => json!([]),
            };
            Box::pin(async move { Ok(value) })
        }
        fn api(
            &self,
            _: reqwest::Method,
            _: String,
            _: Option<Value>,
        ) -> BoxFuture<'static, Result<Value>> {
            Box::pin(async { Err(ServiceError::Failed("offline".into())) })
        }
        fn select(
            &self,
            _: &'static str,
            _: Vec<(&'static str, String)>,
        ) -> BoxFuture<'static, Result<Value>> {
            Box::pin(async { Ok(json!([])) })
        }
        fn open_web(
            &self,
            _: &'static str,
            _: Vec<(&'static str, String)>,
            _: Option<&'static str>,
        ) -> BoxFuture<'static, Result<()>> {
            Box::pin(async { Ok(()) })
        }
        fn copy_text(&self, _: Zeroizing<String>) -> BoxFuture<'static, Result<()>> {
            Box::pin(async { Err(ServiceError::Failed("no clipboard".into())) })
        }
        fn invitation_url(&self, invitation: &str, token: &str) -> Result<String> {
            Ok(format!(
                "https://anarlog.invalid/team/invite/{invitation}/#token={token}"
            ))
        }
        fn sync_state(&self) -> BoxFuture<'static, Result<SyncState>> {
            Box::pin(async { Ok(SyncState::default()) })
        }
        fn sync(&self, _: SyncCommand) -> BoxFuture<'static, Result<Option<Zeroizing<String>>>> {
            Box::pin(async { Ok(None) })
        }
        fn refresh_account(&self, _: &mut gpui::App) {}
    }

    #[tokio::test]
    async fn an_invitation_nobody_can_receive_is_revoked() {
        let calls = Offline::default();
        let delivered = client::deliver_invitation(
            &calls,
            "5d8f2c1a-7b3e-4f6a-9c2d-1e0b7a6f5c4d",
            "Northwind",
            "Maya@Northwind.example",
            "alex@example.com",
        )
        .await;
        assert!(delivered.is_err());
        assert_eq!(
            *calls.revoked.lock().unwrap(),
            [json!({"p_invitation_id": "6e7f8a9b-0c1d-4e2f-8a3b-4c5d6e7f8a9b"})]
        );
    }
}
