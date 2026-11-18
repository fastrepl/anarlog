//! Sharing folders and templates (`resource-sharing/client.ts`, `payloads.ts`): a copy of the
//! item is published for a Team or individual guests, who can add their own copy.

use std::sync::Arc;

use desktop_runtime::{Result, ServiceError, Services};
use futures::future::BoxFuture;
use serde_json::{Value, json};

use reqwest::Method;

use super::{
    CloudServices,
    auth::SecureAuthProvider,
    transport::{Transport, failure},
};

const MAX_PAYLOAD_BYTES: usize = 1_900_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceKind {
    Folder,
    Template,
    Automation,
}

impl ResourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Folder => "folder",
            Self::Template => "template",
            Self::Automation => "automation",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SharedResource {
    pub share_id: Arc<str>,
    pub source_id: Arc<str>,
    pub title: Arc<str>,
    pub owner_email: Arc<str>,
    pub general_workspace_id: Option<Arc<str>>,
    pub workspace_name: Option<Arc<str>>,
    pub owned: bool,
    pub payload: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Guest {
    pub id: Arc<str>,
    pub email: Arc<str>,
}

#[derive(Clone, Debug)]
pub enum ResourceCommand {
    /// Publishes the current copy into a Team library, or for guests only.
    Publish {
        workspace: Option<Arc<str>>,
    },
    /// Publishes, then grants one guest access.
    Invite {
        workspace: Option<Arc<str>>,
        email: String,
    },
    Revoke(Arc<str>),
    StopSharing(Arc<str>),
}

impl CloudServices {
    pub fn shared_resources(
        &self,
        kind: ResourceKind,
    ) -> BoxFuture<'static, Result<Vec<SharedResource>>> {
        let this = self.clone();
        Box::pin(async move {
            this.runtime
                .clone()
                .service(move |_| async move {
                    let rows = this
                        .resource_rpc(
                            "list_shared_resources",
                            json!({"p_resource_type": kind.as_str()}),
                        )
                        .await?;
                    rows.iter().map(parse_resource).collect()
                })?
                .receive()
                .await
        })
    }

    pub fn resource_guests(&self, share: Arc<str>) -> BoxFuture<'static, Result<Vec<Guest>>> {
        let this = self.clone();
        Box::pin(async move {
            this.runtime
                .clone()
                .service(move |_| async move {
                    let rows = this
                        .resource_rpc("list_shared_resource_guests", json!({"p_share_id": share}))
                        .await?;
                    rows.iter()
                        .map(|row| {
                            Ok(Guest {
                                id: text(row, "guest_id")?,
                                email: text(row, "invitee_email")?,
                            })
                        })
                        .collect()
                })?
                .receive()
                .await
        })
    }

    pub fn resource_command(
        &self,
        kind: ResourceKind,
        source: Arc<str>,
        title: Arc<str>,
        command: ResourceCommand,
    ) -> BoxFuture<'static, Result<()>> {
        let this = self.clone();
        Box::pin(async move {
            this.runtime
                .clone()
                .service(move |services| async move {
                    let publish = |workspace: Option<Arc<str>>| {
                        let this = this.clone();
                        let services = services.clone();
                        let source = source.clone();
                        let title = title.clone();
                        async move {
                            let payload = match kind {
                                ResourceKind::Folder => folder_payload(&services, &source).await?,
                                ResourceKind::Template => {
                                    template_payload(&services, &source).await?
                                }
                                ResourceKind::Automation => {
                                    automation_payload(&services, &source).await?
                                }
                            };
                            let rows = this
                                .resource_rpc(
                                    "upsert_shared_resource",
                                    json!({"p_resource_type": kind.as_str(), "p_source_id": source,
                                        "p_title": title, "p_payload": payload,
                                        "p_general_workspace_id": workspace}),
                                )
                                .await?;
                            rows.first()
                                .map(|row| text(row, "share_id"))
                                .ok_or_else(|| failure("Shared resource request failed"))?
                        }
                    };
                    match command {
                        ResourceCommand::Publish { workspace } => {
                            publish(workspace).await?;
                        }
                        ResourceCommand::Invite { workspace, email } => {
                            let share = publish(workspace).await?;
                            this.resource_rpc(
                                "grant_shared_resource_access",
                                json!({"p_share_id": share, "p_invitee_email": email.trim().to_lowercase()}),
                            )
                            .await?;
                        }
                        ResourceCommand::Revoke(guest) => {
                            this.resource_rpc(
                                "revoke_shared_resource_access",
                                json!({"p_guest_id": guest}),
                            )
                            .await?;
                        }
                        ResourceCommand::StopSharing(share) => {
                            this.resource_rpc("delete_shared_resource", json!({"p_share_id": share}))
                                .await?;
                        }
                    }
                    Ok(())
                })?
                .receive()
                .await
        })
    }

    async fn resource_rpc(&self, name: &str, body: Value) -> Result<Vec<Value>> {
        let account = self
            .identity()
            .account_id
            .ok_or_else(|| failure("Sign in to share"))?;
        let lease = self.core.auth.lease(&account, false).await?;
        let transport = &self.core.auth.transport;
        let url = transport
            .config
            .supabase
            .join(&format!("/rest/v1/rpc/{name}"))
            .map_err(|_| failure("Invalid RPC route"))?;
        let response = transport
            .send(
                Method::POST,
                url,
                Some(lease.access_token()?),
                Some(&body),
                &[("apikey", transport.config.anon_key.clone())],
            )
            .await?;
        lease.check()?;
        // Keep only the server messages shipping turns into its Team upsell.
        if let Some(message) = response.body["message"].as_str().filter(|message| {
            !response.status.is_success() && UPSELLS.iter().any(|u| message.contains(u))
        }) {
            return Err(failure(message));
        }
        match Transport::checked(response)? {
            Value::Array(rows) => Ok(rows),
            Value::Null => Ok(Vec::new()),
            _ => Err(failure("Shared resource request failed")),
        }
    }
}

const UPSELLS: [&str; 2] = [
    "multi-resource guest requires Team membership",
    "workspace capability required",
];

/// The Team-upsell errors shipping turns into its "needs Team access" panel.
pub fn is_team_upsell(error: &ServiceError) -> bool {
    let message = error.to_string();
    UPSELLS.iter().any(|upsell| message.contains(upsell))
}

fn text(row: &Value, key: &str) -> Result<Arc<str>> {
    row[key]
        .as_str()
        .filter(|value| !value.is_empty())
        .map(Arc::from)
        .ok_or_else(|| failure("Shared resource request failed"))
}

fn parse_resource(row: &Value) -> Result<SharedResource> {
    let access = row["access_kind"].as_str().unwrap_or_default();
    if !matches!(access, "owner" | "team" | "guest") || !row["payload"].is_object() {
        return Err(failure("Shared resource request failed"));
    }
    Ok(SharedResource {
        share_id: text(row, "share_id")?,
        source_id: text(row, "source_id")?,
        title: text(row, "resource_title")?,
        owner_email: text(row, "owner_email")?,
        general_workspace_id: row["general_workspace_id"].as_str().map(Arc::from),
        workspace_name: row["workspace_name"].as_str().map(Arc::from),
        owned: access == "owner",
        payload: row["payload"].clone(),
    })
}

fn checked(payload: Value) -> Result<Value> {
    if payload.to_string().len() > MAX_PAYLOAD_BYTES {
        return Err(failure("This item is too large to share"));
    }
    Ok(payload)
}

/// `sharedFolderPayload`: the folder's context and every note inside it, nested folders
/// included, with each note's summary (or memo) as ProseMirror JSON.
pub(super) async fn folder_payload(services: &Services, path: &str) -> Result<Value> {
    let path = path.trim().trim_matches('/');
    if path.is_empty() {
        return Err(failure("This shared folder is invalid"));
    }
    let error = |_| failure("Cannot read this folder");
    let instructions: Option<String> = sqlx::query_scalar(
        "SELECT instructions FROM folders WHERE path = ? AND deleted_at IS NULL LIMIT 1",
    )
    .bind(path)
    .fetch_optional(services.db.pool())
    .await
    .map_err(error)?;
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT s.title, s.folder_path,
           COALESCE(enhanced.body, note.body, '') AS body,
           COALESCE(enhanced.body_format, note.body_format, 'prosemirror_json') AS body_format
         FROM sessions s
         LEFT JOIN session_documents enhanced ON enhanced.id = (
           SELECT c.id FROM session_documents c WHERE c.session_id = s.id
             AND c.kind IN ('summary', 'template_output') AND c.deleted_at IS NULL
           ORDER BY c.sort_order, c.id LIMIT 1)
         LEFT JOIN session_documents note ON note.id = (
           SELECT c.id FROM session_documents c WHERE c.session_id = s.id
             AND c.kind = 'note' AND c.deleted_at IS NULL
           ORDER BY c.sort_order, c.id LIMIT 1)
         WHERE s.deleted_at IS NULL
           AND (s.folder_path = ?1
             OR substr(s.folder_path, 1, length(?1) + 1) IN (?1 || '/', ?1 || '\\'))
         ORDER BY s.created_at, s.id",
    )
    .bind(path)
    .fetch_all(services.db.pool())
    .await
    .map_err(error)?;
    let notes = rows
        .into_iter()
        .map(|(title, folder, body, format)| {
            let body = if format == "markdown" {
                anlg_tiptap::md_to_tiptap_json(&body)
                    .map_err(|_| failure("A note in this folder cannot be shared"))?
            } else if body.is_empty() {
                json!({"type": "doc", "content": []})
            } else {
                serde_json::from_str(&body)
                    .map_err(|_| failure("A note in this folder cannot be shared"))?
            };
            let folder = folder.trim().trim_matches('/');
            let relative = folder.strip_prefix(&format!("{path}/")).unwrap_or_default();
            Ok(json!({"title": title, "relativeFolderPath": relative, "body": body}))
        })
        .collect::<Result<Vec<_>>>()?;
    checked(json!({
        "version": 1,
        "path": path.rsplit('/').next().unwrap_or(path),
        "instructions": instructions.unwrap_or_default(),
        "notes": notes,
    }))
}

/// `sharedTemplatePayload`: the template's editable fields.
pub(super) async fn template_payload(services: &Services, id: &str) -> Result<Value> {
    use sqlx::Row as _;
    let row = sqlx::query(
        "SELECT title, description, category, targets_json, sections_json, icon_json
         FROM templates WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(services.db.pool())
    .await
    .map_err(|_| failure("Cannot read this template"))?
    .ok_or_else(|| failure("This template no longer exists"))?;
    let column = |name: &str| -> Option<String> { row.try_get(name).ok().flatten() };
    let parse = |value: Option<String>| {
        value
            .and_then(|value| serde_json::from_str::<Value>(&value).ok())
            .unwrap_or(Value::Null)
    };
    checked(json!({
        "version": 1,
        "template": {
            "title": column("title").unwrap_or_default(),
            "description": column("description").unwrap_or_default(),
            "category": column("category"),
            "icon": parse(column("icon_json")),
            "targets": parse(column("targets_json")),
            "sections": parse(column("sections_json")),
        },
    }))
}

/// `sharedAutomationPayload`: the stored workflow as shipping shares it.
pub(super) async fn automation_payload(services: &Services, id: &str) -> Result<Value> {
    let mut workflow = crate::workspace::automations::stored_workflow(&services.executor, id)
        .await?
        .ok_or_else(|| failure("This automation no longer exists"))?;
    crate::workspace::automations::clear_shared_state(&mut workflow);
    checked(json!({"version": 1, "workflow": workflow}))
}
