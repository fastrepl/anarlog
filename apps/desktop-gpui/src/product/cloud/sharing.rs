use desktop_runtime::{Result, Services, SessionId};
use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row as _;
use zeroize::Zeroizing;

use super::{
    Core, account,
    auth::AuthLease,
    choice, field, target,
    transport::{Transport, failure, first, text, uuid},
};
use crate::product::services::{Action, Mutation, Scope};

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Publication {
    pub share_id: String,
    pub workspace_id: String,
    pub revision: u64,
    pub pending: Option<Value>,
    pub conflict: Option<Value>,
    #[serde(default)]
    pub attachments: Vec<Value>,
}

pub(super) struct ShareSource {
    pub title: String,
    pub body: Value,
    pub workspace: String,
    pub participants: Vec<String>,
    pub meeting_at: String,
}

impl Core {
    pub(super) async fn share_comments(&self, lease: &AuthLease, share: &str) -> Result<Value> {
        let mut comments = Vec::new();
        let mut bytes = 0usize;
        let mut cursor = (Value::Null, Value::Null);
        loop {
            let page = self.rpc(lease, "list_session_share_comments", json!({"p_share_id": share, "p_before_created_at": cursor.0, "p_before_comment_id": cursor.1, "p_limit": 30})).await?;
            let page = page
                .as_array()
                .filter(|rows| rows.len() <= 30)
                .ok_or_else(|| failure("Invalid comments page"))?;
            super::transport::check_directory_size(comments.len(), page, &mut bytes)?;
            comments.extend(page.iter().cloned());
            if page.len() < 30 {
                comments.reverse();
                return Ok(Value::Array(comments));
            }
            let last = page
                .last()
                .ok_or_else(|| failure("Missing comment cursor"))?;
            let next = (last["created_at"].clone(), last["comment_id"].clone());
            if next == cursor || next.0.as_str().is_none() || next.1.as_str().is_none() {
                return Err(failure("Invalid comment cursor"));
            }
            cursor = next;
        }
    }
    pub(super) async fn share_state(
        &self,
        services: &Services,
        lease: &AuthLease,
        session: &SessionId,
    ) -> Result<Option<Publication>> {
        let value: Option<String> =
            sqlx::query_scalar("SELECT value_json FROM app_settings WHERE id = ?")
                .bind(journal_id(account(lease)?, session))
                .fetch_optional(services.db.pool())
                .await
                .map_err(|_| failure("Cannot load share publication journal"))?;
        value
            .map(|value| {
                serde_json::from_str(&value).map_err(|_| {
                    failure("Share publication journal is damaged; original data retained")
                })
            })
            .transpose()
    }

    pub(super) async fn save_share_state(
        &self,
        services: &Services,
        lease: &AuthLease,
        session: &SessionId,
        publication: &Publication,
    ) -> Result<()> {
        lease.check()?;
        sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, ?) ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json")
            .bind(journal_id(account(lease)?, session))
            .bind(serde_json::to_string(publication).map_err(|_| failure("Cannot serialize share publication"))?)
            .execute(services.db.pool()).await.map_err(|_| failure("Cannot save publication state; retry without closing the app"))?;
        Ok(())
    }

    pub(super) async fn share_source(
        &self,
        services: &Services,
        lease: &AuthLease,
        scope: &Scope,
    ) -> Result<ShareSource> {
        let session = scope
            .session_id
            .as_ref()
            .ok_or_else(|| failure("Open a meeting to share"))?;
        self.host.flush_editor(session.clone()).await?;
        lease.check()?;
        let row = sqlx::query(
            "SELECT s.workspace_id, s.owner_user_id, s.title, d.body, d.body_format,
              CASE WHEN datetime(s.started_at) IS NOT NULL THEN s.started_at ELSE s.created_at END AS meeting_at
             FROM sessions s JOIN session_documents d ON d.id = (
               SELECT id FROM session_documents WHERE session_id = s.id AND deleted_at IS NULL
                 AND kind IN ('summary', 'template_output') ORDER BY sort_order, id LIMIT 1)
             WHERE s.id = ? AND s.deleted_at IS NULL",
        )
        .bind(session.0.as_ref())
        .fetch_optional(services.db.pool())
        .await
        .map_err(|_| failure("Cannot read meeting"))?
        .ok_or_else(|| failure("This meeting has no summary to share"))?;
        let workspace: String = row
            .try_get("workspace_id")
            .map_err(|_| failure("Invalid meeting workspace"))?;
        let owner: String = row
            .try_get("owner_user_id")
            .map_err(|_| failure("Invalid meeting owner"))?;
        let local_access: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM workspace_memberships m JOIN workspaces w ON w.id = m.workspace_id WHERE m.workspace_id = ? AND m.user_id = ? AND m.role IN ('owner', 'admin') AND m.deleted_at IS NULL AND w.deleted_at IS NULL)")
            .bind(&workspace).bind(account(lease)?).fetch_one(services.db.pool()).await
            .map_err(|_| failure("Cannot verify workspace membership"))?;
        let routed = anlg_db_app::local_library_remote_workspace(services.db.pool(), &workspace)
            .await
            .map_err(|_| failure("Cannot verify local library routing"))?;
        let personal = routed == account(lease)?;
        if !local_access
            && !(personal
                && (owner == account(lease)? || owner == "00000000-0000-0000-0000-000000000000"))
        {
            return Err(failure("This account does not own this meeting"));
        }
        let remote_workspace = scope.workspace_id.as_deref().unwrap_or(&routed).to_owned();
        uuid(&remote_workspace)?;
        if remote_workspace != routed {
            let can_share: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workspace_memberships m JOIN workspaces w ON w.id = m.workspace_id WHERE m.workspace_id = ? AND m.user_id = ? AND m.role IN ('owner', 'admin') AND m.deleted_at IS NULL AND w.deleted_at IS NULL)")
                .bind(&remote_workspace).bind(account(lease)?).fetch_one(services.db.pool()).await.map_err(|_| failure("Cannot verify destination workspace"))?;
            if !can_share {
                return Err(failure(
                    "This account cannot publish into the selected workspace",
                ));
            }
        }
        let body: String = row
            .try_get("body")
            .map_err(|_| failure("Invalid meeting body"))?;
        let format: String = row
            .try_get("body_format")
            .map_err(|_| failure("Invalid meeting format"))?;
        if body.len() > 2 * 1024 * 1024 {
            return Err(failure("Meeting exceeds sharing size limit"));
        }
        let document = match format.as_str() {
            "prosemirror_json" => {
                serde_json::from_str(&body).map_err(|_| failure("Meeting document is malformed"))?
            }
            "markdown" => anlg_tiptap::md_to_tiptap_json(&body)
                .map_err(|_| failure("Meeting markdown is malformed"))?,
            _ => return Err(failure("Meeting document format cannot be shared")),
        };
        validate_document(&document)?;
        if document["content"].as_array().is_none_or(Vec::is_empty) {
            return Err(failure("Generate a summary before sharing"));
        }
        let title = row
            .try_get::<String, _>("title")
            .map_err(|_| failure("Invalid meeting title"))?;
        let names: Vec<String> = sqlx::query_scalar(
            "SELECT COALESCE(NULLIF(h.name, ''), p.display_name)
             FROM session_participants p LEFT JOIN humans h ON h.id = p.human_id AND h.deleted_at IS NULL
             WHERE p.session_id = ? AND p.human_id <> '' AND p.source <> 'excluded' AND p.deleted_at IS NULL
             ORDER BY p.created_at, p.id LIMIT 1025",
        ).bind(session.0.as_ref()).fetch_all(services.db.pool()).await.map_err(|_| failure("Cannot read meeting participants"))?;
        if names.len() > 1024 {
            return Err(failure("Meeting exceeds participant limit"));
        }
        let mut participants = Vec::new();
        for name in names {
            let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
            if !name.is_empty()
                && name.encode_utf16().count() <= 100
                && !participants.contains(&name)
            {
                participants.push(name);
                if participants.len() == 32 {
                    break;
                }
            }
        }
        Ok(ShareSource {
            title,
            body: document,
            workspace: remote_workspace,
            participants,
            meeting_at: row
                .try_get("meeting_at")
                .map_err(|_| failure("Invalid meeting timestamp"))?,
        })
    }

    pub(super) async fn share_action(
        &self,
        services: &Services,
        lease: &AuthLease,
        scope: &Scope,
        mutation: &Mutation,
    ) -> Result<()> {
        let session = scope
            .session_id
            .as_ref()
            .ok_or_else(|| failure("Open a meeting to share"))?;
        let existing = self.share_state(services, lease, session).await?;
        if mutation.operation.action == Action::CreateShare {
            let workspace = self.share_source(services, lease, scope).await?.workspace;
            let value = self
                .rpc(
                    lease,
                    "create_session_share",
                    json!({"p_workspace_id": workspace, "p_session_id": session.0.as_ref()}),
                )
                .await?;
            let share = uuid(text(first(&value)?, "share_id")?)?;
            if existing
                .as_ref()
                .is_some_and(|old| old.share_id != share && old.pending.is_some())
            {
                return Err(failure(
                    "Existing publication has pending changes; resolve it before creating another share",
                ));
            }
            let publication = existing.unwrap_or(Publication {
                share_id: share.into(),
                workspace_id: workspace,
                revision: 0,
                pending: None,
                conflict: None,
                attachments: Vec::new(),
            });
            self.save_share_state(services, lease, session, &publication)
                .await?;
            return self.publish(services, lease, scope, publication).await;
        }
        let publication = existing.ok_or_else(|| failure("Create a share first"))?;
        let share = &publication.share_id;
        let id = || uuid(target(mutation)?);
        let (rpc, body) = match mutation.operation.action {
            Action::PublishShare => return self.publish(services, lease, scope, publication).await,
            Action::SetShareScope => {
                let next = choice(mutation, "scope", &["restricted", "workspace", "public"])?;
                (
                    "set_session_share_scope",
                    json!({"p_share_id": share, "p_general_scope": next,
                    "p_general_workspace_id": if next == "workspace" { Some(uuid(field(mutation, "workspace")?)?) } else { None }}),
                )
            }
            Action::SetShareRole => (
                "update_session_access_grant",
                json!({"p_grant_id": id()?, "p_capability": choice(mutation, "role", &["viewer", "commenter", "editor"])?}),
            ),
            Action::RespondShareRequest => {
                let decision = choice(mutation, "decision", &["approved", "denied"])?;
                (
                    "review_session_access_request",
                    json!({"p_request_id": id()?, "p_decision": decision,
                    "p_capability": if decision == "approved" { Some(choice(mutation, "role", &["viewer", "commenter", "editor"])?) } else { None }}),
                )
            }
            Action::AddComment => (
                "create_session_share_comment",
                json!({"p_share_id": share, "p_body": field(mutation, "comment")?,
                "p_anchor_quote_exact": null, "p_anchor_quote_prefix": null, "p_anchor_quote_suffix": null,
                "p_anchor_from_hint": null, "p_anchor_to_hint": null}),
            ),
            Action::DeleteComment => (
                "delete_session_share_comment",
                json!({"p_comment_id": id()?}),
            ),
            Action::RevokeShareLink => (
                "set_session_share_scope",
                json!({"p_share_id": share, "p_general_scope": "restricted", "p_general_workspace_id": null}),
            ),
            Action::RevokeInvitation => (
                "revoke_session_access_invitation",
                json!({"p_invitation_id": id()?}),
            ),
            Action::DeleteShare => {
                self.rpc(lease, "delete_session_share_by_session", json!({"p_workspace_id": publication.workspace_id, "p_session_id": session.0.as_ref()})).await?;
                sqlx::query("DELETE FROM app_settings WHERE id = ?")
                    .bind(journal_id(account(lease)?, session))
                    .execute(services.db.pool())
                    .await
                    .map_err(|_| {
                        failure("Share deleted remotely; local journal cleanup needs retry")
                    })?;
                sqlx::query(
                    "DELETE FROM shared_session_cache WHERE viewer_user_id = ? AND share_id = ?",
                )
                .bind(account(lease)?)
                .bind(share)
                .execute(services.db.pool())
                .await
                .map_err(|_| failure("Share deleted remotely; local cache cleanup needs retry"))?;
                return Ok(());
            }
            Action::EnableShareLink | Action::RotateShareLink | Action::CopyShareLink => {
                let rpc = if mutation.operation.action == Action::RotateShareLink {
                    "rotate_session_share_link"
                } else {
                    "enable_session_share_link"
                };
                let response = self.rpc(lease, rpc, json!({"p_share_id": share})).await?;
                let row = first(&response)?;
                let link = uuid(text(row, "link_id")?)?;
                let slot = self
                    .auth
                    .key(&format!("{}:share:{share}:link", account(lease)?));
                let url = if let Some(token) = row["link_token"].as_str() {
                    let url = self.capability_url(&format!("/t/{link}/"), token)?;
                    self.auth
                        .store
                        .write(slot.clone(), Zeroizing::new(url.clone()))
                        .await?;
                    url
                } else {
                    self.auth.store.read(slot).await?.ok_or_else(|| failure("Link token exists on another device. Rotate the link to copy a new one"))?.to_string()
                };
                self.host.copy_text(Zeroizing::new(url)).await?;
                return Ok(());
            }
            Action::InviteToShare | Action::ResendInvitation => {
                let mut response = if mutation.operation.action == Action::InviteToShare {
                    self.rpc(lease, "create_session_access_invitation", json!({"p_share_id": share,
                        "p_invitee_email": field(mutation, "email")?.to_lowercase(),
                        "p_capability": choice(mutation, "role", &["viewer", "commenter", "editor"])?})).await?
                } else {
                    self.rpc(
                        lease,
                        "resend_session_access_invitation",
                        json!({"p_invitation_id": id()?}),
                    )
                    .await?
                };
                let invitation = uuid(text(first(&response)?, "invitation_id")?)?.to_owned();
                if first(&response)?["invite_token"].as_str().is_none() {
                    response = self
                        .rpc(
                            lease,
                            "resend_session_access_invitation",
                            json!({"p_invitation_id":invitation}),
                        )
                        .await?;
                }
                let token = text(first(&response)?, "invite_token")?;
                let url = self.capability_url(&format!("/share/invite/{invitation}/"), token)?;
                let title = self.share_source(services, lease, scope).await?.title;
                let sent = self.api(lease, Method::POST, &format!("/shared-notes/invitations/{invitation}/email"),
                    Some(&json!({"shareId":share,"inviteToken":token,"noteTitle":title,"fromName":lease.identity.email.as_deref().unwrap_or_default()}))).await;
                if sent.is_err() {
                    lease.check()?;
                    if self.host.copy_text(Zeroizing::new(url)).await.is_err() {
                        self.rpc(
                            lease,
                            "revoke_session_access_invitation",
                            json!({"p_invitation_id": invitation}),
                        )
                        .await?;
                        return Err(failure("Could not deliver invitation; invitation revoked"));
                    }
                    *self.notice.lock().await =
                        Some("Email unavailable. Invitation link copied instead".into());
                }
                return Ok(());
            }
            _ => return Err(failure("Invalid sharing action")),
        };
        self.rpc(lease, rpc, body).await?;
        Ok(())
    }

    pub(super) fn capability_url(&self, path: &str, token: &str) -> Result<String> {
        if token.len() != 43
            || !token
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        {
            return Err(failure("Invalid invitation/link capability"));
        }
        let mut url = self
            .auth
            .transport
            .config
            .web
            .join(path)
            .map_err(|_| failure("Invalid sharing URL"))?;
        url.set_fragment(Some(&format!("token={token}")));
        if self.auth.transport.config.callback.scheme() != "anarlog" {
            url.query_pairs_mut()
                .append_pair("scheme", self.auth.transport.config.callback.scheme());
        }
        Ok(url.to_string())
    }

    pub(super) async fn publish(
        &self,
        services: &Services,
        lease: &AuthLease,
        scope: &Scope,
        mut state: Publication,
    ) -> Result<()> {
        let session = scope
            .session_id
            .as_ref()
            .ok_or_else(|| failure("Select a meeting"))?;
        if state.conflict.is_some() {
            return Err(failure(
                "The web copy changed. Review the saved remote snapshot before retrying; your local draft is intact",
            ));
        }
        if state.pending.is_none() {
            let source = self.share_source(services, lease, scope).await?;
            let ids = state
                .attachments
                .iter()
                .map(|attachment| text(attachment, "id").and_then(uuid))
                .collect::<Result<Vec<_>>>()?;
            state.pending = Some(
                json!({"baseRevision": state.revision, "mutationId": uuid::Uuid::new_v4().to_string(), "title": source.title, "body": source.body, "attachmentIds": ids, "participants": source.participants, "meetingAt": source.meeting_at}),
            );
            self.save_share_state(services, lease, session, &state)
                .await?;
        }
        let url = self
            .auth
            .transport
            .config
            .api
            .join(&format!("/sync/shares/{}/snapshot", state.share_id))
            .map_err(|_| failure("Invalid share route"))?;
        let response = self
            .auth
            .transport
            .send(
                Method::PUT,
                url,
                Some(lease.access_token()?),
                state.pending.as_ref(),
                &[],
            )
            .await?;
        lease.check()?;
        if response.status == StatusCode::CONFLICT {
            if response.body["code"] != "snapshot_conflict" {
                return Err(failure("Share conflict response is malformed"));
            }
            validate_snapshot(&response.body["snapshot"], &state.share_id)?;
            state.conflict = Some(response.body["snapshot"].clone());
            self.save_share_state(services, lease, session, &state)
                .await?;
            return Err(desktop_runtime::ServiceError::Conflict);
        }
        let snapshot = Transport::checked(response)?;
        validate_snapshot(&snapshot, &state.share_id)?;
        persist_snapshot(
            services,
            account(lease)?,
            session,
            &state.workspace_id,
            &snapshot,
        )
        .await?;
        state.revision = snapshot["contentRevision"]
            .as_u64()
            .ok_or_else(|| failure("Invalid published revision"))?;
        state.pending = None;
        state.attachments = snapshot["attachments"]
            .as_array()
            .cloned()
            .ok_or_else(|| failure("Invalid attachment list"))?;
        self.save_share_state(services, lease, session, &state)
            .await
    }
}

fn journal_id(account: &str, session: &SessionId) -> String {
    format!("gpui.share.{account}.{}", session.0.as_ref())
}

pub(super) fn validate_document(value: &Value) -> Result<()> {
    if value["type"] != "doc" {
        return Err(failure("Invalid document root"));
    }
    let mut stack = vec![(value, 0usize)];
    let mut count = 0;
    while let Some((node, depth)) = stack.pop() {
        count += 1;
        if count > 50_000 || depth > 64 || !node.is_object() || node["type"].as_str().is_none() {
            return Err(failure("Invalid or excessively complex document"));
        }
        if let Some(children) = node.get("content") {
            let children = children
                .as_array()
                .ok_or_else(|| failure("Invalid document content"))?;
            stack.extend(children.iter().map(|child| (child, depth + 1)));
        }
    }
    Ok(())
}

fn validate_snapshot(value: &Value, share: &str) -> Result<()> {
    if value["shareId"] != share
        || value["schemaVersion"] != 1
        || value["contentRevision"]
            .as_u64()
            .is_none_or(|revision| revision == 0)
        || value["accessVersion"]
            .as_u64()
            .is_none_or(|version| version == 0)
        || value["publishedAt"].as_str().is_none()
        || value["attachments"]
            .as_array()
            .is_none_or(|rows| rows.len() > 64)
        || value["webEditable"].as_bool().is_none()
    {
        return Err(failure("Invalid published snapshot"));
    }
    validate_document(&value["body"])
}

pub(super) async fn persist_snapshot(
    services: &Services,
    account: &str,
    session: &SessionId,
    workspace: &str,
    value: &Value,
) -> Result<()> {
    let written = sqlx::query("INSERT INTO shared_session_cache
      (share_id, viewer_user_id, workspace_id, session_id, schema_version, content_revision, title, body_json, capability, manage_access, access_version, published_at, attachments_json, web_editable)
      VALUES (?, ?, ?, ?, 1, ?, ?, ?, 'editor', 1, ?, ?, ?, ?)
      ON CONFLICT(viewer_user_id, share_id) DO UPDATE SET
      content_revision = excluded.content_revision, title = excluded.title, body_json = excluded.body_json,
      access_version = excluded.access_version, published_at = excluded.published_at, attachments_json = excluded.attachments_json, web_editable = excluded.web_editable
      WHERE shared_session_cache.content_revision <= excluded.content_revision AND shared_session_cache.web_edit_base_content_revision IS NULL")
        .bind(text(value, "shareId")?).bind(account).bind(workspace).bind(session.0.as_ref())
        .bind(value["contentRevision"].as_i64()).bind(value["title"].as_str().unwrap_or_default())
        .bind(serde_json::to_string(&value["body"]).map_err(|_| failure("Cannot persist snapshot"))?)
        .bind(value["accessVersion"].as_i64()).bind(text(value, "publishedAt")?)
        .bind(value["attachments"].to_string()).bind(value["webEditable"].as_bool().unwrap_or(false))
        .execute(services.db.pool()).await.map_err(|_| failure("Share published but local receipt could not be saved. Retry to reconcile"))?;
    if written.rows_affected() == 0 {
        return Err(failure(
            "A newer or edited shared cache exists. Its draft was retained; reconcile it before publishing again",
        ));
    }
    Ok(())
}
