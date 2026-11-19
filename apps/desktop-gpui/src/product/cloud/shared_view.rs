//! Network halves of shipping's shared-note routes (`shared-notes/deeplink.ts`): an account
//! link refreshes the durable shared-note cache the shared-note view reads, and a handoff link
//! claims a one-time preview snapshot without signing in.

use desktop_runtime::{CancellationToken, Result, ServiceError};
use reqwest::Method;
use serde_json::{Value, json};

use super::{
    CloudServices,
    auth::SecureAuthProvider,
    sharing::validate_document,
    transport::{Transport, failure, uuid},
};

/// A preview claimed from a handoff link.
pub struct SharedSnapshot {
    pub title: String,
    pub body: Value,
    pub content_revision: u64,
}

impl CloudServices {
    /// `openAccountSharedNote`: reads the account's copy of `share` and stores it in the local
    /// cache, or removes the cached copy when access is gone.
    pub async fn refresh_shared(&self, share: String, cancel: CancellationToken) -> Result<()> {
        uuid(&share)?;
        let account = self
            .identity()
            .account_id
            .ok_or_else(|| failure("Sign in to open this shared note"))?;
        let this = self.clone();
        this.runtime
            .clone()
            .service(move |services| async move {
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => Err(ServiceError::Cancelled),
                    result = async {
                        let lease = this.core.auth.lease(&account, false).await?;
                        let response = this.core.rpc(&lease, "read_my_session_share_snapshot_v2", json!({"p_share_id": share})).await?;
                        let rows = response.as_array().ok_or_else(|| failure("Invalid shared-note snapshot"))?;
                        match rows.as_slice() {
                            [] => {
                                sqlx::query("DELETE FROM shared_session_cache WHERE viewer_user_id = ? AND share_id = ?")
                                    .bind(&*account).bind(&share).execute(services.db.pool()).await
                                    .map_err(|_| failure("Cannot update the shared-note cache"))?;
                                Ok(())
                            }
                            [row] if row["share_id"] == share.as_str() => cache_snapshot(&services, &account, row).await,
                            _ => Err(failure("Invalid shared-note snapshot")),
                        }
                    } => result,
                }
            })?
            .receive()
            .await
    }

    /// `syncDurableSharedNoteCache`: refreshes every note shared with the account and removes
    /// cached notes it can no longer open. Rows cached while the list loads are kept.
    pub async fn reconcile_shared(&self, cancel: CancellationToken) -> Result<()> {
        const PAGE: usize = 8;
        const MAX_BYTES: usize = 64 * 1024 * 1024;
        let account = self
            .identity()
            .account_id
            .ok_or_else(|| failure("Sign in to sync shared notes"))?;
        let this = self.clone();
        this.runtime
            .clone()
            .service(move |services| async move {
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => Err(ServiceError::Cancelled),
                    result = async {
                        let invalid = || failure("Invalid shared-note snapshot list");
                        let unwritable = |_| failure("Cannot update the shared-note cache");
                        let pool = services.db.pool();
                        let cached: Vec<(String, String)> = sqlx::query_as("SELECT share_id, cached_at FROM shared_session_cache WHERE viewer_user_id = ?")
                            .bind(&*account).fetch_all(pool).await.map_err(unwritable)?;
                        let lease = this.core.auth.lease(&account, false).await?;
                        let (mut rows, mut bytes, mut after) = (Vec::new(), 0, None::<String>);
                        loop {
                            let body = json!({"p_after_share_id": after, "p_limit": PAGE});
                            let page = this.core.rpc_page(&lease, "list_my_session_share_snapshot_page_v2", &body, None, Transport::checked).await?;
                            let page = page.as_array().filter(|page| page.len() <= PAGE).ok_or_else(invalid)?;
                            bytes += serde_json::to_vec(page).map_err(|_| invalid())?.len();
                            if bytes > MAX_BYTES {
                                return Err(failure("The shared-note list is too large"));
                            }
                            rows.extend(page.iter().cloned());
                            if page.len() < PAGE {
                                break;
                            }
                            let cursor = page.last().and_then(|row| row["share_id"].as_str())
                                .filter(|id| after.as_deref().is_none_or(|after| *id > after))
                                .ok_or_else(invalid)?;
                            after = Some(cursor.to_owned());
                        }
                        let shares = rows.iter().map(|row| row["share_id"].as_str()).collect::<Option<Vec<_>>>().ok_or_else(invalid)?;
                        for (share, cached_at) in cached.iter().filter(|(share, _)| !shares.contains(&share.as_str())) {
                            sqlx::query("DELETE FROM shared_session_cache WHERE viewer_user_id = ? AND share_id = ? AND cached_at = ?")
                                .bind(&*account).bind(share).bind(cached_at)
                                .execute(pool).await.map_err(unwritable)?;
                        }
                        for row in &rows {
                            cache_snapshot(&services, &account, row).await?;
                        }
                        Ok(())
                    } => result,
                }
            })?
            .receive()
            .await
    }

    /// `claimSharedNoteHandoff`: the anonymous, one-time preview behind a handoff link.
    pub async fn claim_preview(
        &self,
        request: String,
        cancel: CancellationToken,
    ) -> Result<SharedSnapshot> {
        if cancel.is_cancelled() {
            return Err(ServiceError::Cancelled);
        }
        uuid(&request)?;
        let this = self.clone();
        this.runtime
            .clone()
            .service(move |_| async move {
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => Err(ServiceError::Cancelled),
                    result = async {
                        let url = this.core.auth.transport.config.api.join("/shared-notes/handoffs/claim")
                            .map_err(|_| failure("Invalid shared-note endpoint"))?;
                        let value = Transport::checked(this.core.auth.transport.send(Method::POST, url, None,
                            Some(&json!({"request_id": request, "lease_id": uuid::Uuid::new_v4().to_string()})), &[]).await?)?;
                        let body = document(value.get("body").ok_or_else(|| failure("Shared-note body is missing"))?)?;
                        Ok(SharedSnapshot {
                            title: value["title"].as_str().ok_or_else(|| failure("Shared-note title is missing"))?.to_owned(),
                            body,
                            content_revision: value["contentRevision"].as_u64().unwrap_or(1),
                        })
                    } => result,
                }
            })?
            .receive()
            .await
    }
}

fn document(value: &Value) -> Result<Value> {
    let body = match value.as_str() {
        Some(text) => {
            serde_json::from_str(text).map_err(|_| failure("Invalid shared-note document"))?
        }
        None => value.clone(),
    };
    validate_document(&body)?;
    Ok(body)
}

async fn cache_snapshot(
    services: &desktop_runtime::Services,
    account: &str,
    row: &Value,
) -> Result<()> {
    let text = |key: &str| {
        row[key]
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| failure("Invalid shared-note snapshot"))
    };
    let number = |key: &str| {
        row[key]
            .as_i64()
            .filter(|value| *value > 0)
            .ok_or_else(|| failure("Invalid shared-note snapshot"))
    };
    let capability = text("capability")?;
    if !matches!(capability, "viewer" | "commenter" | "editor") || row["schema_version"] != 1 {
        return Err(failure("Invalid shared-note snapshot"));
    }
    let body = document(&row["body_json"])?;
    let attachments = row["attachments_json"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    sqlx::query(
        "INSERT INTO shared_session_cache
          (share_id, viewer_user_id, workspace_id, session_id, schema_version, content_revision, title,
           body_json, attachments_json, capability, manage_access, access_version, web_editable, published_at, cached_at)
         VALUES (?, ?, ?, ?, 1, ?, ?, ?, ?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(viewer_user_id, share_id) DO UPDATE SET
          workspace_id = excluded.workspace_id, session_id = excluded.session_id,
          content_revision = excluded.content_revision, title = excluded.title,
          body_json = excluded.body_json, attachments_json = excluded.attachments_json,
          capability = excluded.capability, manage_access = excluded.manage_access,
          access_version = excluded.access_version, web_editable = excluded.web_editable,
          published_at = excluded.published_at, cached_at = excluded.cached_at",
    )
    .bind(text("share_id")?)
    .bind(account)
    .bind(text("workspace_id")?)
    .bind(text("session_id")?)
    .bind(number("content_revision")?)
    .bind(row["title"].as_str().unwrap_or_default().trim())
    .bind(body.to_string())
    .bind(Value::Array(attachments).to_string())
    .bind(capability)
    .bind(row["manage_access"].as_bool() == Some(true))
    .bind(number("access_version")?)
    .bind(row["web_editable"].as_bool() == Some(true))
    .bind(text("published_at")?)
    .execute(services.db.pool())
    .await
    .map_err(|_| failure("Cannot update the shared-note cache"))?;
    Ok(())
}
