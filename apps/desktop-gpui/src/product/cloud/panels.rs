use std::sync::Arc;

use desktop_runtime::{Result, Services};
use reqwest::Method;
use serde_json::{Value, json};

use super::{
    Core, account,
    transport::{failure, text},
};
use crate::product::services::{Action, Field, Operation, Panel, Request, Row, Surface};

impl Core {
    pub(super) async fn load(
        &self,
        surface: Surface,
        request: &Request,
        services: &Services,
        page: usize,
    ) -> Result<Panel> {
        let mut panel = Panel {
            scope: request.scope.clone(),
            status: "".into(),
            rows: Arc::from([]),
            fields: Arc::from([]),
            operations: Arc::from([]),
            permissions_ready: false,
        };
        if self.auth.identity().account_id.is_none() {
            if request.scope.account_id.is_some() {
                return Err(desktop_runtime::ServiceError::Conflict);
            }
            panel.status = "Sign in to Anarlog".into();
            panel.fields = Arc::from([input("provider", "Sign-in provider", "google")]);
            panel.operations = Arc::from([operation(Action::SignIn, "Continue with browser")]);
            return Ok(panel);
        }
        let lease = self.lease(&request.scope).await?;
        let mut rows = Vec::new();
        let mut actions = Vec::new();
        let mut fields = Vec::new();
        match surface {
            Surface::Account => {
                rows.push(row(
                    "account",
                    lease.identity.email.as_deref().unwrap_or("Anarlog account"),
                    account(&lease)?,
                ));
                panel.status = "Your account".into();
                actions.extend([
                    operation(Action::RefreshAccount, "Refresh account"),
                    operation(Action::SignOut, "Sign out"),
                ]);
            }
            Surface::Billing => {
                let claims = self.auth.claims(account(&lease)?).await?;
                let plan = if claims.is_pro() {
                    "Pro"
                } else if claims.is_lite() {
                    "Lite"
                } else {
                    "Free"
                };
                panel.status = format!("Your plan: {plan}").into();
                rows.push(row(
                    "plan",
                    plan,
                    if claims.has_active_trial() {
                        "Free trial active"
                    } else {
                        "Current subscription"
                    },
                ));
                if let Some(end) = claims.trial_end {
                    rows.push(row("trial-end", "Trial ends", &end.to_rfc3339()));
                }
                if claims.has_payment_method == Some(false) && claims.has_active_trial() {
                    rows.push(row(
                        "payment",
                        "Add a payment method",
                        "Your subscription pauses when the trial ends without a payment method",
                    ));
                }
                let eligibility = self
                    .api(&lease, Method::GET, "/subscription/can-start-trial", None)
                    .await?;
                if eligibility["canStartTrial"] == true {
                    actions.push(operation(Action::StartTrial, "Start free trial"));
                }
                fields.push(input("interval", "Billing period", "monthly"));
                actions.extend([
                    operation(Action::Checkout, "Get Pro"),
                    operation(Action::BillingPortal, "Manage billing"),
                    operation(Action::RefreshAccount, "Refresh subscription"),
                ]);
            }
            Surface::Teams => {
                panel.status = "Workspaces".into();
                let memberships = self.memberships(&lease).await?;
                for item in bounded_rows(&memberships, 64)? {
                    rows.push(row(
                        &format!("workspace:{}", text(item, "workspace_id")?),
                        item["workspaces"]["name"].as_str().unwrap_or("Workspace"),
                        text(item, "role")?,
                    ));
                }
                let invitations = self
                    .rpc(&lease, "list_my_workspace_invitations", json!({}))
                    .await?;
                for invitation in bounded_rows(&invitations, 32)? {
                    rows.push(row(
                        &format!("invitation:{}", text(invitation, "invitation_id")?),
                        text(invitation, "workspace_name")?,
                        "Invitation",
                    ));
                }
                fields.extend([
                    input("name", "Workspace name", ""),
                    input("target", "Selected member or invitation", ""),
                    input("email", "Invite by email", ""),
                    input("role", "Member role", "member"),
                ]);
                actions.extend([
                    operation(Action::CreateWorkspace, "Create workspace"),
                    operation(Action::AcceptInvitation, "Accept invitation"),
                    operation(Action::DeclineInvitation, "Decline invitation"),
                ]);
                if let Some(workspace) = request.scope.workspace_id.as_deref() {
                    let access = self
                        .rpc(
                            &lease,
                            "get_workspace_access",
                            json!({"p_workspace_id": workspace}),
                        )
                        .await?;
                    let access = super::transport::first(&access)?;
                    let role = text(access, "workspace_role")?;
                    let members = self
                        .rpc(
                            &lease,
                            "list_workspace_members_with_profiles",
                            json!({"p_workspace_id": workspace}),
                        )
                        .await?;
                    for member in bounded_rows(&members, 32)? {
                        rows.push(row(
                            &format!("member:{}", text(member, "user_id")?),
                            text(member, "user_email")?,
                            text(member, "role")?,
                        ));
                    }
                    if matches!(role, "admin" | "owner") {
                        let invitations = self
                            .rpc(
                                &lease,
                                "list_workspace_invitations",
                                json!({"p_workspace_id": workspace}),
                            )
                            .await?;
                        for invitation in bounded_rows(&invitations, 32)?.iter().filter(|row| {
                            row["accepted_at"].is_null() && row["revoked_at"].is_null()
                        }) {
                            rows.push(row(
                                &format!("invitation:{}", text(invitation, "invitation_id")?),
                                text(invitation, "invitee_email")?,
                                "Pending invitation",
                            ));
                        }
                        actions.extend([
                            operation(Action::InviteMember, "Invite member"),
                            operation(Action::ResendInvitation, "Resend invitation"),
                            operation(Action::RevokeInvitation, "Revoke invitation"),
                            operation(Action::SetMemberRole, "Change member role"),
                            operation(Action::SetSharingSlug, "Save sharing URL"),
                            operation(Action::SetWorkspacePolicy, "Save sharing policy"),
                        ]);
                        let policy = self
                            .rpc(
                                &lease,
                                "get_workspace_policy",
                                json!({"p_workspace_id": workspace}),
                            )
                            .await?;
                        let policy = super::transport::first(&policy)?;
                        let scopes = policy["allowed_share_scopes"]
                            .as_array()
                            .ok_or_else(|| failure("Invalid workspace policy"))?
                            .iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join(", ");
                        fields.extend([
                            input("slug", "Sharing URL slug", ""),
                            input("allowed_scopes", "Allowed sharing scopes", &scopes),
                            input(
                                "default_scope",
                                "Default sharing scope",
                                text(policy, "default_share_scope")?,
                            ),
                            input(
                                "retention_days",
                                "Retention days (empty means unlimited)",
                                &policy["retention_days"]
                                    .as_u64()
                                    .map(|value| value.to_string())
                                    .unwrap_or_default(),
                            ),
                            input(
                                "training_opt_out",
                                "Opt out of model training",
                                if policy["model_training_opt_out"] == false {
                                    "false"
                                } else {
                                    "true"
                                },
                            ),
                            input(
                                "consent",
                                "Consent notifications",
                                if policy["consent_notification_enabled"] == false {
                                    "false"
                                } else {
                                    "true"
                                },
                            ),
                            input(
                                "require_sso",
                                "Require SSO",
                                if policy["require_sso"] == true {
                                    "true"
                                } else {
                                    "false"
                                },
                            ),
                        ]);
                    }
                    if role == "owner" {
                        actions.extend([
                            operation(Action::TransferOwnership, "Transfer ownership"),
                            operation(Action::DeleteWorkspace, "Delete workspace"),
                        ]);
                    } else {
                        actions.push(operation(Action::LeaveWorkspace, "Leave workspace"));
                    }
                }
            }
            Surface::CloudSync => {
                let status = self.sync.hook.replica_status();
                panel.status = if status.syncing {
                    "Syncing encrypted changes"
                } else if self.sync.hook.replica_transport_configured() {
                    "Encrypted sync connected"
                } else {
                    "Sync paused"
                }
                .into();
                rows.push(row(
                    "sync-status",
                    "End-to-end encryption",
                    if status.pending_changes {
                        "Local changes pending"
                    } else {
                        "No pending changes reported"
                    },
                ));
                if let Some(last) = status.last_sync_at_ms {
                    rows.push(row(
                        "last-sync",
                        "Last successful sync (Unix ms)",
                        &last.to_string(),
                    ));
                }
                if status.last_error.is_some() {
                    rows.push(row(
                        "sync-error",
                        "Sync needs attention",
                        "Changes are retained. Check your connection and retry",
                    ));
                }
                let devices = self.api(&lease, Method::GET, "/sync/devices", None).await?;
                for device in bounded_rows(&devices["devices"], 32)? {
                    rows.push(row(
                        &format!("device:{}", text(device, "deviceFingerprint")?),
                        device["deviceName"].as_str().unwrap_or("Device"),
                        text(device, "lastSeenAt")?,
                    ));
                }
                for device in bounded_rows(&devices["pendingDevices"], 32)? {
                    rows.push(row(
                        &format!("enrollment:{}", text(device, "requestId")?),
                        device["deviceName"].as_str().unwrap_or("New device"),
                        text(device, "status")?,
                    ));
                }
                fields.extend([
                    input("target", "Selected device", ""),
                    input("device_name", "Device name", ""),
                ]);
                actions.extend([
                    operation(Action::SetupEncryption, "Set up encryption"),
                    operation(Action::ImportRecoveryKey, "Import recovery key"),
                    operation(Action::ExportRecoveryKey, "Save recovery key"),
                    operation(Action::CopyRecoveryKey, "Copy recovery key"),
                    operation(Action::ApproveDevice, "Approve device"),
                    operation(Action::ReplaceDevice, "Enroll / replace device"),
                    operation(Action::RenameDevice, "Rename device"),
                    operation(Action::RemoveDevice, "Remove device"),
                    operation(Action::PauseSync, "Pause sync"),
                    operation(Action::ResumeSync, "Resume sync"),
                    operation(Action::SyncNow, "Sync now"),
                    operation(Action::ConnectLocalLibrary, "Connect this library"),
                    operation(Action::RepairKeychain, "Retry secure storage"),
                ]);
            }
            Surface::Sharing => {
                panel.status = "Share meeting".into();
                let session = request
                    .scope
                    .session_id
                    .as_ref()
                    .ok_or_else(|| failure("Open a meeting to share"))?;
                if let Some(publication) = self.share_state(services, &lease, session).await? {
                    rows.push(row(
                        &publication.share_id,
                        "Published revision",
                        &publication.revision.to_string(),
                    ));
                    if publication.pending.is_some() {
                        rows.push(row(
                            "pending",
                            "Publication pending",
                            "Retry uses the same mutation ID; the local draft is preserved",
                        ));
                    }
                    if publication.conflict.is_some() {
                        rows.push(row(
                            "conflict",
                            "The web copy changed",
                            "Review both copies in the publication conflict view",
                        ));
                    }
                    let management = self
                        .rpc(
                            &lease,
                            "get_session_share_management",
                            json!({"p_share_id": publication.share_id}),
                        )
                        .await?;
                    let management = super::transport::first(&management)?;
                    rows.push(row(
                        "scope",
                        "Who can access",
                        text(management, "general_scope")?,
                    ));
                    let access = self
                        .rpc(
                            &lease,
                            "list_session_share_access",
                            json!({"p_share_id": publication.share_id}),
                        )
                        .await?;
                    for entry in bounded_rows(&access, 64)? {
                        rows.push(row(
                            &format!(
                                "share-{}:{}",
                                text(entry, "entry_type")?,
                                text(entry, "entry_id")?
                            ),
                            entry["user_email"].as_str().unwrap_or("Access request"),
                            text(entry, "capability")?,
                        ));
                    }
                    let comments = self.share_comments(&lease, &publication.share_id).await?;
                    for comment in bounded_rows(&comments, 30)? {
                        rows.push(row(
                            &format!("comment:{}", text(comment, "comment_id")?),
                            "Comment",
                            text(comment, "body")?,
                        ));
                    }
                    fields.extend([
                        input("email", "Invite by email", ""),
                        input("role", "Permission", "viewer"),
                        input(
                            "scope",
                            "General access",
                            text(management, "general_scope")?,
                        ),
                        input("workspace", "Workspace", &publication.workspace_id),
                        input("target", "Selected access entry or comment", ""),
                        input("comment", "Add a comment", ""),
                        input("decision", "Access request decision", "approved"),
                    ]);
                    actions.extend([
                        operation(Action::PublishShare, "Publish changes"),
                        operation(Action::InviteToShare, "Invite"),
                        operation(Action::ResendInvitation, "Resend invitation"),
                        operation(Action::RevokeInvitation, "Revoke invitation"),
                        operation(Action::SetShareRole, "Change permission"),
                        operation(Action::SetShareScope, "Save general access"),
                        operation(Action::EnableShareLink, "Enable link"),
                        operation(Action::CopyShareLink, "Copy link"),
                        operation(Action::RotateShareLink, "Rotate link"),
                        operation(Action::RevokeShareLink, "Turn off link access"),
                        operation(Action::RespondShareRequest, "Respond to access request"),
                        operation(Action::AddComment, "Post comment"),
                        operation(Action::DeleteComment, "Delete comment"),
                        operation(Action::DeleteShare, "Delete share"),
                    ]);
                } else {
                    actions.push(operation(Action::CreateShare, "Create and publish share"));
                }
            }
            _ => return Err(failure("This page belongs to another product service")),
        }
        lease.check()?;
        if let Some(notice) = self.notice.lock().await.as_ref() {
            panel.status = format!("{} · {notice}", panel.status).into();
        }
        let mut seen = std::collections::HashSet::new();
        rows.retain(|row| seen.insert(row.id.clone()));
        let total = rows.len();
        let offset = page.saturating_mul(128);
        if total > 128 {
            panel.status = format!(
                "{} · {}–{} of {}",
                panel.status,
                (offset + 1).min(total),
                (offset + 128).min(total),
                total
            )
            .into();
        }
        panel.rows = rows
            .into_iter()
            .skip(offset)
            .take(128)
            .collect::<Vec<_>>()
            .into();
        panel.fields = fields.into();
        panel.operations = actions.into();
        panel.validate(&request.scope)?;
        Ok(panel)
    }
}

pub(super) fn operation(action: Action, label: &str) -> Operation {
    let confirmation = (action.requires_confirmation()
        || matches!(
            action,
            Action::SignOut | Action::ConnectLocalLibrary | Action::RotateShareLink
        ))
    .then(|| {
        format!(
            "{label}? This changes access or account state. Unsaved local documents are retained."
        )
        .into()
    });
    Operation {
        id: format!("{action:?}").into(),
        label: label.into(),
        action,
        target_id: None,
        disabled_reason: None,
        confirmation,
    }
}

fn row(id: &str, title: &str, detail: &str) -> Row {
    Row {
        id: id.into(),
        title: title.into(),
        detail: detail.into(),
    }
}

fn input(id: &str, label: &str, value: &str) -> Field {
    Field {
        id: id.into(),
        label: label.into(),
        value: value.into(),
    }
}

fn bounded_rows(value: &Value, _limit: usize) -> Result<&[Value]> {
    let rows = value
        .as_array()
        .ok_or_else(|| failure("Invalid cloud list response"))?;
    if rows.len() > 16_384 {
        return Err(failure("Cloud directory exceeds the response limit"));
    }
    Ok(rows)
}

pub(super) fn allowed(surface: Surface, action: &Action) -> bool {
    if *action == Action::SignIn {
        return matches!(
            surface,
            Surface::Account
                | Surface::Billing
                | Surface::Teams
                | Surface::CloudSync
                | Surface::Sharing
        );
    }
    match surface {
        Surface::Account => matches!(
            action,
            Action::SignIn | Action::SignOut | Action::LogoutLibrary | Action::RefreshAccount
        ),
        Surface::Billing => matches!(
            action,
            Action::SignIn
                | Action::StartTrial
                | Action::Checkout
                | Action::BillingPortal
                | Action::RefreshAccount
        ),
        Surface::Teams => matches!(
            action,
            Action::CreateWorkspace
                | Action::AcceptInvitation
                | Action::DeclineInvitation
                | Action::InviteMember
                | Action::ResendInvitation
                | Action::RevokeInvitation
                | Action::SetMemberRole
                | Action::TransferOwnership
                | Action::LeaveWorkspace
                | Action::DeleteWorkspace
                | Action::SetWorkspacePolicy
                | Action::SetSharingSlug
        ),
        Surface::CloudSync => matches!(
            action,
            Action::SetupEncryption
                | Action::ImportRecoveryKey
                | Action::CopyRecoveryKey
                | Action::ExportRecoveryKey
                | Action::ApproveDevice
                | Action::ReplaceDevice
                | Action::RenameDevice
                | Action::RemoveDevice
                | Action::PauseSync
                | Action::ResumeSync
                | Action::SyncNow
                | Action::RepairKeychain
                | Action::ConnectLocalLibrary
        ),
        Surface::Sharing => matches!(
            action,
            Action::CreateShare
                | Action::InviteToShare
                | Action::SetShareRole
                | Action::SetShareScope
                | Action::EnableShareLink
                | Action::CopyShareLink
                | Action::RotateShareLink
                | Action::RevokeShareLink
                | Action::DeleteShare
                | Action::RespondShareRequest
                | Action::AddComment
                | Action::DeleteComment
                | Action::PublishShare
                | Action::ResendInvitation
                | Action::RevokeInvitation
        ),
        _ => false,
    }
}
