//! Cloud API & Connectors (shipping's `cloud-api/client.ts`): the opt-in server-readable copy of
//! each meeting, its API keys, and the snapshot uploads that keep that copy current.
//!
//! Per-account progress (backfill done, pending uploads/deletes, last seen revisions) is kept in a
//! profile file, like shipping's localStorage, so interrupted work resumes on the next launch. A
//! channel run first merges in the progress the shipping app's bridge release mirrored.

use std::{
    collections::{BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anlg_desktop_identity::webview_storage::{
    CLOUD_API_BACKFILL, CLOUD_API_PENDING, CLOUD_API_REVISIONS, Snapshot,
};
use desktop_runtime::{Result, ServiceError, Services};
use futures::future::BoxFuture;
use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::SqlitePool;

use super::{CloudServices, transport::failure};

/// The per-account progress, in the profile.
pub const PROGRESS_FILE: &str = "cloud-api.json";
const MAX_SNAPSHOT_BYTES: usize = 2 * 1024 * 1024;
const BACKFILL_RETRY: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct CloudApiKey {
    pub id: String,
    pub name: String,
    pub key_prefix: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct CreatedCloudApiKey {
    #[serde(flatten)]
    pub info: CloudApiKey,
    pub key: String,
}

/// The toggle's result: shipping's success/failure toast depends on all three.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Toggled {
    pub enabled: bool,
    pub uploaded: usize,
    pub backfill_failed: bool,
}

/// A session's change marker: `deleted:<at>` once deleted, else its newest related row update.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Revision {
    pub id: String,
    pub deleted_at: Option<String>,
    pub revision: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub deletes: Vec<String>,
    pub upserts: Vec<String>,
}

/// The local revision query from shipping's `CloudApiBackfillLifecycle`.
pub const REVISIONS_SQL: &str = "SELECT session.id, session.deleted_at AS deletedAt, max(\
    session.updated_at, \
    COALESCE((SELECT max(document.updated_at) FROM session_documents AS document WHERE document.session_id = session.id), ''), \
    COALESCE((SELECT max(transcript.updated_at) FROM transcripts AS transcript WHERE transcript.session_id = session.id), ''), \
    COALESCE((SELECT max(participant.updated_at) FROM session_participants AS participant WHERE participant.session_id = session.id), ''), \
    COALESCE((SELECT max(human.updated_at) FROM session_participants AS participant JOIN humans AS human ON human.id = participant.human_id WHERE participant.session_id = session.id), ''), \
    COALESCE((SELECT max(organization.updated_at) FROM session_participants AS participant JOIN humans AS human ON human.id = participant.human_id JOIN organizations AS organization ON organization.id = human.organization_id WHERE participant.session_id = session.id), ''), \
    COALESCE((SELECT max(action_item.updated_at) FROM action_items AS action_item WHERE action_item.session_id = session.id), '')\
    ) AS revision FROM sessions AS session ORDER BY session.id";

#[derive(Default, Serialize, Deserialize)]
struct Account {
    backfilled: bool,
    upserts: BTreeSet<String>,
    deletes: BTreeSet<String>,
    revisions: Option<HashMap<String, String>>,
    /// The shipping app's last mirror merged into this account, so a launch merges each once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shipping_mirror: Option<String>,
    /// How often each session changed since launch: an upload clears its pending entry only if
    /// no newer revision arrived while it was in flight.
    #[serde(skip)]
    changes: HashMap<String, u64>,
}

#[derive(Default)]
struct State {
    accounts: HashMap<String, Account>,
    /// The signed-in account's last known setting, like shipping's `cachedEnabled`.
    enabled: Option<(String, bool)>,
}

struct ApiError {
    code: String,
    message: String,
}

impl ApiError {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    /// Shipping's `shouldRetry`: these errors cannot succeed by retrying later.
    fn retryable(&self) -> bool {
        !matches!(
            self.code.as_str(),
            "subscription_required"
                | "cloud_api_not_enabled"
                | "invalid_request"
                | "not_configured"
        )
    }
}

impl From<ApiError> for ServiceError {
    fn from(error: ApiError) -> Self {
        failure(&error.message)
    }
}

type ApiResult<T> = std::result::Result<T, ApiError>;

#[derive(Clone)]
pub struct CloudApi {
    cloud: CloudServices,
    path: PathBuf,
    state: Arc<std::sync::Mutex<State>>,
    /// Serializes snapshot uploads and deletes, like shipping's request queue.
    queue: Arc<tokio::sync::Mutex<()>>,
    retrying: Arc<AtomicBool>,
}

impl gpui::Global for CloudApi {}

impl CloudApi {
    pub fn new(cloud: CloudServices, path: PathBuf) -> Self {
        let accounts = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            cloud,
            path,
            state: Arc::new(std::sync::Mutex::new(State {
                accounts,
                enabled: None,
            })),
            queue: Arc::default(),
            retrying: Arc::default(),
        }
    }

    /// Shipping's `CLOUD_API_BASE_URL` and `CLOUD_MCP_URL`.
    pub fn endpoints(&self) -> (String, String) {
        let api = &self.cloud.core.auth.transport.config.api;
        let url = |path| {
            api.join(path)
                .map(|url| url.to_string())
                .unwrap_or_default()
        };
        (url("/v1"), url("/mcp"))
    }

    pub fn settings(&self) -> BoxFuture<'static, Result<bool>> {
        self.run(|this, _| async move {
            let user = this.user()?;
            Ok(this.load_enabled(&user).await?)
        })
    }

    /// Turns the readable copy on (then uploads every meeting) or off (the server deletes it).
    pub fn set_enabled(&self, enabled: bool) -> BoxFuture<'static, Result<Toggled>> {
        self.run(move |this, services| async move {
            let user = this.user()?;
            let settings = this
                .request(
                    Method::PUT,
                    "/v1/cloud-api/settings",
                    Some(&json!({ "enabled": enabled })),
                    &user,
                )
                .await?;
            let now = settings["enabled"] == true;
            this.edit(&user, |account| {
                if !enabled {
                    account.backfilled = false;
                    account.upserts.clear();
                    account.deletes.clear();
                }
            });
            this.cache(&user, now);
            if !enabled {
                return Ok(Toggled {
                    enabled: now,
                    uploaded: 0,
                    backfill_failed: false,
                });
            }
            match this.backfill(&services, &user).await {
                Ok(uploaded) => Ok(Toggled {
                    enabled: now,
                    uploaded,
                    backfill_failed: false,
                }),
                Err(_) => {
                    this.schedule_retry();
                    Ok(Toggled {
                        enabled: now,
                        uploaded: 0,
                        backfill_failed: true,
                    })
                }
            }
        })
    }

    pub fn keys(&self) -> BoxFuture<'static, Result<Vec<CloudApiKey>>> {
        self.run(|this, _| async move {
            let user = this.user()?;
            let keys = this
                .request(Method::GET, "/v1/cloud-api/keys", None, &user)
                .await?;
            serde_json::from_value(keys).map_err(|_| failure("Malformed cloud response"))
        })
    }

    pub fn create_key(&self, name: String) -> BoxFuture<'static, Result<CreatedCloudApiKey>> {
        self.run(move |this, _| async move {
            let user = this.user()?;
            let key = this
                .request(
                    Method::POST,
                    "/v1/cloud-api/keys",
                    Some(&json!({ "name": name })),
                    &user,
                )
                .await?;
            serde_json::from_value(key).map_err(|_| failure("Malformed cloud response"))
        })
    }

    pub fn revoke_key(&self, id: String) -> BoxFuture<'static, Result<()>> {
        self.run(move |this, _| async move {
            let user = this.user()?;
            let path = format!("/v1/cloud-api/keys/{}", encode(&id));
            this.request(Method::DELETE, &path, None, &user).await?;
            Ok(())
        })
    }

    /// Shipping's `initializeCloudApiBackfill`, run at sign-in: finishes the first upload or the
    /// pending changes. Failures retry every 30 seconds.
    pub fn initialize(&self) -> BoxFuture<'static, Result<()>> {
        self.run(|this, services| async move {
            let result = async {
                let user = this.user()?;
                if !this.load_enabled(&user).await? {
                    return Ok(());
                }
                this.flush(&services, &user, true).await?;
                if this.with(&user, |account| account.backfilled) {
                    this.flush(&services, &user, false).await?;
                } else {
                    this.backfill(&services, &user).await?;
                }
                Ok::<_, ApiError>(())
            }
            .await;
            if let Err(error) = &result
                && error.code != "unauthorized"
            {
                this.schedule_retry();
            }
            Ok(result?)
        })
    }

    /// Records the library's latest revisions for `user` and returns what changed since the
    /// last snapshot, marking those sessions pending.
    pub fn observe(&self, user: &str, revisions: &[Revision]) -> Changes {
        let mut changes = Changes::default();
        self.edit(user, |account| {
            let (found, next) = snapshot_changes(account.revisions.as_ref(), revisions);
            for id in &found.deletes {
                account.upserts.remove(id);
                account.deletes.insert(id.clone());
            }
            for id in &found.upserts {
                if !account.deletes.contains(id) {
                    account.upserts.insert(id.clone());
                    *account.changes.entry(id.clone()).or_default() += 1;
                }
            }
            account.revisions = Some(next);
            changes = found;
        });
        changes
    }

    /// Uploads or deletes `user`'s pending sessions. `Ok(true)` means a snapshot was rejected as
    /// invalid and may succeed shortly (shipping retries those after 5 and 15 seconds).
    pub fn sync(&self, user: String, deletes_only: bool) -> BoxFuture<'static, Result<bool>> {
        self.run(move |this, services| async move {
            match this.flush(&services, &user, deletes_only).await {
                Ok(()) => Ok(false),
                Err(error) if error.code == "invalid_request" => Ok(true),
                Err(error) => Err(error.into()),
            }
        })
    }

    fn run<T: Send + 'static, F>(
        &self,
        work: impl FnOnce(Self, Services) -> F + Send + 'static,
    ) -> BoxFuture<'static, Result<T>>
    where
        F: Future<Output = Result<T>> + Send + 'static,
    {
        let this = self.clone();
        let runtime = self.cloud.runtime.clone();
        Box::pin(async move {
            runtime
                .service(move |services| work(this, services))?
                .receive()
                .await
        })
    }

    fn user(&self) -> ApiResult<String> {
        self.cloud
            .core
            .auth
            .identity()
            .account_id
            .map(|id| id.to_string())
            .ok_or_else(|| {
                ApiError::new("unauthorized", "Sign in to manage Cloud API & Connectors.")
            })
    }

    fn with<T>(&self, user: &str, read: impl FnOnce(&Account) -> T) -> T {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        read(state.accounts.get(user).unwrap_or(&Account::default()))
    }

    fn edit(&self, user: &str, change: impl FnOnce(&mut Account)) {
        self.update(user, change);
        self.save();
    }

    /// Changes progress in memory only; flushes save once at the end instead of per meeting.
    fn update(&self, user: &str, change: impl FnOnce(&mut Account)) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        change(state.accounts.entry(user.into()).or_default());
    }

    fn save(&self) {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let saved = serde_json::to_string(&state.accounts)
            .map_err(|error| error.to_string())
            .and_then(|json| {
                anlg_storage::fs::atomic_write(&self.path, &json).map_err(|error| error.to_string())
            });
        if let Err(error) = saved {
            tracing::warn!(%error, "could not save Cloud API progress");
        }
    }

    fn cache(&self, user: &str, enabled: bool) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.enabled = Some((user.into(), enabled));
    }

    async fn load_enabled(&self, user: &str) -> ApiResult<bool> {
        let settings = self
            .request(Method::GET, "/v1/cloud-api/settings", None, user)
            .await?;
        let enabled = settings["enabled"] == true;
        self.cache(user, enabled);
        Ok(enabled)
    }

    async fn enabled(&self, user: &str) -> ApiResult<bool> {
        let cached = {
            let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            state
                .enabled
                .as_ref()
                .filter(|(account, _)| account == user)
                .map(|(_, enabled)| *enabled)
        };
        match cached {
            Some(enabled) => Ok(enabled),
            None => self.load_enabled(user).await,
        }
    }

    fn schedule_retry(&self) {
        if self.retrying.swap(true, Ordering::SeqCst) {
            return;
        }
        let this = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(BACKFILL_RETRY).await;
            this.retrying.store(false, Ordering::SeqCst);
            // `initialize` schedules the next attempt itself when this one fails.
            let _ = this.initialize().await;
        });
    }

    async fn backfill(&self, services: &Services, user: &str) -> ApiResult<usize> {
        let ids = local_ids(services.db.pool())
            .await
            .map_err(|error| ApiError::new("local_snapshot_unavailable", error.to_string()))?;
        self.edit(user, |account| {
            for id in ids {
                if !account.deletes.contains(&id) {
                    account.upserts.insert(id);
                }
            }
        });
        let before = self.with(user, |account| account.upserts.len());
        let result = self.flush(services, user, false).await;
        let after = self.with(user, |account| account.upserts.len());
        self.edit(user, |account| account.backfilled = true);
        result.map(|()| before.saturating_sub(after))
    }

    async fn flush(&self, services: &Services, user: &str, deletes_only: bool) -> ApiResult<()> {
        let _queue = self.queue.lock().await;
        let result = self.flush_pending(services, user, deletes_only).await;
        self.save();
        result
    }

    async fn flush_pending(
        &self,
        services: &Services,
        user: &str,
        deletes_only: bool,
    ) -> ApiResult<()> {
        if !self.enabled(user).await? {
            self.update(user, |account| {
                account.upserts.clear();
                account.deletes.clear();
            });
            return Ok(());
        }
        let mut first_error = None;
        for id in self.with(user, |account| account.deletes.clone()) {
            match self.delete(user, &id).await {
                Ok(()) => self.update(user, |account| {
                    account.deletes.remove(&id);
                }),
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
        if !deletes_only {
            let pool = services.db.pool();
            for id in self.with(user, |account| account.upserts.clone()) {
                let (deleted, seen) = self.with(user, |account| {
                    (
                        account.deletes.contains(&id),
                        account.changes.get(&id).copied(),
                    )
                });
                if deleted {
                    continue;
                }
                let acknowledge = |account: &mut Account| {
                    if account.changes.get(&id).copied() == seen {
                        account.upserts.remove(&id);
                    }
                };
                let result = match local_snapshot(pool, &id).await {
                    Ok(snapshot) => {
                        let path = format!("/v1/sync-snapshots/{}", encode(&id));
                        self.request(Method::PUT, &path, Some(&snapshot), user)
                            .await
                            .map(|_| ())
                    }
                    // A session deleted locally since it was queued: remove its copy instead.
                    Err(_) if local_ids(pool).await.is_ok_and(|ids| !ids.contains(&id)) => {
                        self.update(user, |account| {
                            account.deletes.insert(id.clone());
                        });
                        let deleted = self.delete(user, &id).await;
                        if deleted.is_ok() {
                            self.update(user, |account| {
                                account.deletes.remove(&id);
                            });
                        }
                        deleted
                    }
                    Err(error) => Err(ApiError::new("local_snapshot_unavailable", error)),
                };
                match result {
                    Ok(()) => self.update(user, acknowledge),
                    Err(error) => {
                        if !error.retryable() && error.code != "invalid_request" {
                            self.update(user, acknowledge);
                        }
                        first_error.get_or_insert(error);
                    }
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    async fn delete(&self, user: &str, id: &str) -> ApiResult<()> {
        let path = format!("/v1/sync-snapshots/{}", encode(id));
        self.request(Method::DELETE, &path, None, user)
            .await
            .map(|_| ())
    }

    /// One authenticated request: renews the session once on 401 and waits once on 429.
    async fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
        user: &str,
    ) -> ApiResult<Value> {
        let auth = &self.cloud.core.auth;
        let url = auth
            .transport
            .config
            .api
            .join(path)
            .map_err(|_| ApiError::new("not_configured", "Invalid Cloud API route"))?;
        let (mut renewed, mut waited) = (false, false);
        loop {
            let lease = auth.lease(user, false).await.map_err(|error| match error {
                ServiceError::Conflict => ApiError::new(
                    "unauthorized",
                    "The signed-in account changed during the request.",
                ),
                error => ApiError::new("unauthorized", error.to_string()),
            })?;
            let token = lease
                .access_token()
                .map_err(|error| ApiError::new("unauthorized", error.to_string()))?;
            let response = auth
                .transport
                .send(method.clone(), url.clone(), Some(token), body, &[])
                .await
                .map_err(|error| ApiError::new("network", error.to_string()))?;
            match response.status {
                StatusCode::UNAUTHORIZED if !renewed => {
                    renewed = true;
                    if auth.lease(user, true).await.is_ok() {
                        continue;
                    }
                }
                StatusCode::TOO_MANY_REQUESTS if !waited => {
                    waited = true;
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
                status if status.is_success() => return Ok(response.body),
                _ => {}
            }
            let error = &response.body["error"];
            let status = response.status.as_u16();
            return Err(ApiError {
                code: error["code"]
                    .as_str()
                    .map_or_else(|| format!("http_{status}"), str::to_owned),
                message: error["message"].as_str().map_or_else(
                    || format!("Cloud API request failed ({status})."),
                    str::to_owned,
                ),
            });
        }
    }
}

/// Merges the progress the shipping app kept in localStorage (`client.ts`, `lifecycle.tsx`), as its
/// bridge release mirrored it, into the progress file at `path`, once per mirror. Pending changes
/// are a union in which a delete wins, so neither app's pending delete is lost; this app's own
/// revisions are kept when it has them. A mirror merged again after this app finished some of its
/// work only repeats those uploads and deletes.
pub fn import_shipping(path: &Path, mirror: &Snapshot) -> std::io::Result<()> {
    let mut accounts: HashMap<String, Account> = match std::fs::read(path) {
        // `CloudApi::new` also starts over from an unreadable file.
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => HashMap::default(),
        Err(error) => return Err(error),
    };
    let users: BTreeSet<&str> = [CLOUD_API_PENDING, CLOUD_API_BACKFILL, CLOUD_API_REVISIONS]
        .into_iter()
        .flat_map(|prefix| mirror.with_prefix(prefix).map(|(user, _)| user))
        .collect();
    let mut changed = false;
    for user in users {
        let account = accounts.entry(user.to_owned()).or_default();
        if account.shipping_mirror.as_deref() == Some(mirror.id.as_str()) {
            continue;
        }
        let entry = |prefix: &str| mirror.get(&format!("{prefix}{user}"));
        // Shipping's `readPendingChanges`: anything unreadable is no pending change.
        let pending: Value = entry(CLOUD_API_PENDING)
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();
        let ids = |kind: &str| -> Vec<String> {
            pending[kind]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|id| id.as_str().map(str::to_owned))
                .collect()
        };
        account.deletes.extend(ids("deletes"));
        account.upserts.extend(ids("upserts"));
        let deletes = &account.deletes;
        account.upserts.retain(|id| !deletes.contains(id));
        account.backfilled |= entry(CLOUD_API_BACKFILL) == Some("complete");
        if account.revisions.is_none() {
            // `readStoredSnapshotRevisions`: `[[id, revision], ...]`, the same markers as here.
            account.revisions = entry(CLOUD_API_REVISIONS)
                .and_then(|json| serde_json::from_str::<Vec<(String, String)>>(json).ok())
                .map(|pairs| pairs.into_iter().collect());
        }
        account.shipping_mirror = Some(mirror.id.clone());
        changed = true;
    }
    if !changed {
        return Ok(());
    }
    let json = serde_json::to_string(&accounts).map_err(std::io::Error::other)?;
    anlg_storage::fs::atomic_write(path, &json)
}

/// `cloudApiSnapshotChanges`: deleted or vanished sessions become deletes, new or changed ones
/// upserts. Without a previous snapshot, every live session is an upsert.
pub fn snapshot_changes(
    previous: Option<&HashMap<String, String>>,
    revisions: &[Revision],
) -> (Changes, HashMap<String, String>) {
    let mut next = HashMap::with_capacity(revisions.len());
    let mut changes = Changes::default();
    for row in revisions {
        let state = match &row.deleted_at {
            Some(at) => format!("deleted:{at}"),
            None => row.revision.clone(),
        };
        let before = previous.and_then(|previous| previous.get(&row.id));
        if row.deleted_at.is_some() {
            if before != Some(&state) {
                changes.deletes.push(row.id.clone());
            }
        } else if before != Some(&state) {
            changes.upserts.push(row.id.clone());
        }
        next.insert(row.id.clone(), state);
    }
    for id in previous.into_iter().flat_map(HashMap::keys) {
        if !next.contains_key(id) {
            changes.deletes.push(id.clone());
        }
    }
    (changes, next)
}

fn encode(segment: &str) -> String {
    url::form_urlencoded::byte_serialize(segment.as_bytes())
        .collect::<String>()
        .replace('+', "%20")
}

/// `list_cloud_snapshot_ids`: every meeting the local API lists.
async fn local_ids(pool: &SqlitePool) -> std::result::Result<Vec<String>, String> {
    let mut ids = Vec::new();
    let mut offset = 0;
    loop {
        let page = anlg_agent_access::list_meetings(
            pool,
            anlg_agent_access::ListMeetingsInput {
                query: None,
                series_id: None,
                folder_path: None,
                limit: Some(anlg_agent_access::MAX_LIST_LIMIT),
                offset: Some(offset),
            },
        )
        .await
        .map_err(|error| error.to_string())?;
        ids.extend(page.meetings.into_iter().map(|meeting| meeting.id));
        let Some(next) = page.pagination.next_offset else {
            return Ok(ids);
        };
        offset = next;
    }
}

/// `get_cloud_snapshot`: the meeting export, without word timings when it is too large.
async fn local_snapshot(pool: &SqlitePool, id: &str) -> std::result::Result<Value, String> {
    let mut export = anlg_agent_access::get_meeting_export(pool, id.into())
        .await
        .map_err(|error| error.to_string())?;
    let snapshot = serde_json::to_value(&export).map_err(|error| error.to_string())?;
    if jsonb_len(&snapshot) <= MAX_SNAPSHOT_BYTES {
        return Ok(snapshot);
    }
    for transcript in &mut export.transcripts {
        transcript.words.clear();
        transcript.speaker_hints.clear();
    }
    let snapshot = serde_json::to_value(export).map_err(|error| error.to_string())?;
    if jsonb_len(&snapshot) > MAX_SNAPSHOT_BYTES {
        return Err(format!(
            "meeting snapshot exceeds the {MAX_SNAPSHOT_BYTES}-byte limit"
        ));
    }
    Ok(snapshot)
}

/// The hosted limit measures `jsonb::text`, with separator spaces and expanded exponents.
fn jsonb_len(value: &Value) -> usize {
    match value {
        Value::Array(values) => {
            2 + values.len().saturating_sub(1) * 2 + values.iter().map(jsonb_len).sum::<usize>()
        }
        Value::Object(values) => {
            2 + values.len().saturating_sub(1) * 2
                + values
                    .iter()
                    .map(|(key, value)| {
                        Value::from(key.as_str()).to_string().len() + 2 + jsonb_len(value)
                    })
                    .sum::<usize>()
        }
        Value::Number(number) => {
            let text = number.to_string();
            let Some((mantissa, exponent)) = text.split_once('e') else {
                return text.len();
            };
            let exponent = exponent.parse::<i32>().unwrap_or(0);
            let sign = usize::from(mantissa.starts_with('-'));
            let mantissa = mantissa.trim_start_matches('-');
            let digits = mantissa.bytes().filter(u8::is_ascii_digit).count();
            let decimal = mantissa.find('.').unwrap_or(mantissa.len()) as i32 + exponent;
            sign + if decimal <= 0 {
                2 + (-decimal) as usize + digits
            } else if decimal as usize >= digits {
                decimal as usize
            } else {
                digits + 1
            }
        }
        _ => value.to_string().len(),
    }
}

#[cfg(test)]
mod tests {
    use anlg_desktop_identity::webview_storage::{FILENAME, WebviewStorage};

    use super::*;

    /// What the bridge release writes for these localStorage entries, read back as this app does.
    fn mirror(dir: &Path, entries: &[(&str, &str)]) -> Snapshot {
        let storage = WebviewStorage::at(dir.join(FILENAME));
        storage
            .write(
                entries
                    .iter()
                    .map(|(key, value)| (key.to_string(), value.to_string())),
            )
            .unwrap();
        storage.read().unwrap().unwrap()
    }

    fn accounts(path: &Path) -> HashMap<String, Account> {
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }

    fn ids(ids: &BTreeSet<String>) -> Vec<&str> {
        ids.iter().map(String::as_str).collect()
    }

    #[test]
    fn a_pending_delete_from_either_app_survives_the_shipping_import() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cloud-api.json");
        let kept = HashMap::from([("kept".to_owned(), "1".to_owned())]);
        let own = Account {
            upserts: BTreeSet::from(["edited-in-both".to_owned()]),
            deletes: BTreeSet::from(["deleted-here".to_owned()]),
            revisions: Some(kept.clone()),
            ..Account::default()
        };
        std::fs::write(
            &path,
            serde_json::to_string(&HashMap::from([("user-1", own)])).unwrap(),
        )
        .unwrap();
        let shipping = mirror(
            dir.path(),
            &[
                (
                    "anarlog.cloud-api-pending.v1.user-1",
                    r#"{"upserts":["edited-there","deleted-here"],"deletes":["deleted-there","edited-in-both"]}"#,
                ),
                ("anarlog.cloud-api-backfill.v1.user-1", "complete"),
                (
                    "anarlog.cloud-api-revisions.v1.user-1",
                    r#"[["ignored","1"]]"#,
                ),
                (
                    "anarlog.cloud-api-pending.v1.user-2",
                    r#"{"upserts":[],"deletes":["n"]}"#,
                ),
                (
                    "anarlog.cloud-api-revisions.v1.user-2",
                    r#"[["n","deleted:t"]]"#,
                ),
            ],
        );

        import_shipping(&path, &shipping).unwrap();

        let accounts = accounts(&path);
        let user = &accounts["user-1"];
        assert_eq!(
            ids(&user.deletes),
            ["deleted-here", "deleted-there", "edited-in-both"]
        );
        assert_eq!(ids(&user.upserts), ["edited-there"]);
        assert!(user.backfilled);
        assert_eq!(user.revisions, Some(kept));
        let other = &accounts["user-2"];
        assert_eq!(ids(&other.deletes), ["n"]);
        assert!(!other.backfilled);
        let deleted = HashMap::from([("n".to_owned(), "deleted:t".to_owned())]);
        assert_eq!(other.revisions, Some(deleted));
    }

    #[test]
    fn a_merged_mirror_is_not_merged_again_until_a_rollback_writes_a_new_one() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cloud-api.json");
        let pending = |deletes: &str| format!(r#"{{"upserts":[],"deletes":[{deletes}]}}"#);
        let first = mirror(
            dir.path(),
            &[(
                "anarlog.cloud-api-pending.v1.user-1",
                pending(r#""a""#).as_str(),
            )],
        );
        import_shipping(&path, &first).unwrap();
        // This app deletes `a` from the server copy and queues its own delete.
        let mut saved = accounts(&path);
        let account = saved.get_mut("user-1").unwrap();
        account.deletes = BTreeSet::from(["own".to_owned()]);
        std::fs::write(&path, serde_json::to_string(&saved).unwrap()).unwrap();

        import_shipping(&path, &first).unwrap();
        assert_eq!(ids(&accounts(&path)["user-1"].deletes), ["own"]);

        // After a rollback the shipping app runs again and mirrors its own pending delete.
        let after_rollback = mirror(
            dir.path(),
            &[(
                "anarlog.cloud-api-pending.v1.user-1",
                pending(r#""b""#).as_str(),
            )],
        );
        import_shipping(&path, &after_rollback).unwrap();
        assert_eq!(ids(&accounts(&path)["user-1"].deletes), ["b", "own"]);
    }

    #[test]
    fn revision_changes_upload_edits_and_delete_removed_sessions() {
        let row = |id: &str, deleted: Option<&str>, revision: &str| Revision {
            id: id.into(),
            deleted_at: deleted.map(Into::into),
            revision: revision.into(),
        };
        let (first, known) = snapshot_changes(None, &[row("a", None, "1"), row("b", None, "1")]);
        assert_eq!(first.upserts, ["a", "b"]);
        let (changes, _) = snapshot_changes(
            Some(&known),
            &[row("a", None, "2"), row("c", Some("t"), "1")],
        );
        assert_eq!(changes.upserts, ["a"]);
        let mut deletes = changes.deletes;
        deletes.sort();
        assert_eq!(deletes, ["b", "c"]);
    }
}
