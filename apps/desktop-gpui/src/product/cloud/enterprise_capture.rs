//! Shipping's enterprise capture delivery (`enterprise-capture/{lifecycle,sync,client,store}`):
//! every 15 seconds, shared workspaces with the `enterprise.capture` capability pull captured
//! sessions from the enterprise server, apply them locally, acknowledge them, and queue the
//! finished meetings for their meeting-completed automations.

use std::time::Duration;

use desktop_runtime::{Result, ServiceError, Services, SessionId};
use futures::{StreamExt, future::BoxFuture};
use reqwest::{Method, Url, header::HeaderValue};
use serde_json::{Value, json};

use super::{CloudServices, Core, auth::AuthLease};

pub const POLL_INTERVAL: Duration = Duration::from_secs(15);
const MAX_PAGES_PER_SYNC: usize = 100;
const MAX_PAGE_BYTES: usize = 24 * 1024 * 1024;
const PAGE_LIMIT: u32 = 10;

/// `VITE_ENTERPRISE_API_URL`; without it there is no enterprise delivery.
pub fn server() -> Option<Url> {
    std::env::var("VITE_ENTERPRISE_API_URL")
        .ok()
        .or_else(|| option_env!("VITE_ENTERPRISE_API_URL").map(str::to_owned))
        .filter(|url| !url.is_empty())?
        .parse()
        .ok()
}

/// A finished captured meeting whose automations have not run yet.
#[derive(Clone, Debug, PartialEq)]
pub struct Completion {
    pub source_id: String,
    pub session: SessionId,
}

/// One poll's result: the meetings to complete, and the first failure, which shipping reports
/// only after dispatching what it could.
#[derive(Debug, Default)]
pub struct Delivery {
    pub completions: Vec<Completion>,
    pub error: Option<ServiceError>,
}

struct Item {
    cursor: i64,
    job_id: String,
    revision: i64,
    finalized: bool,
    content_hash: String,
    source_id: String,
    session_id: String,
    envelope: Value,
}

fn failure(message: impl std::fmt::Display) -> ServiceError {
    ServiceError::Failed(message.to_string().into())
}

fn invalid() -> ServiceError {
    failure("The capture server returned an invalid delivery envelope.")
}

fn integer(value: &Value, allow_zero: bool) -> Result<i64> {
    value
        .as_i64()
        .filter(|value| if allow_zero { *value >= 0 } else { *value >= 1 })
        .filter(|value| *value < 1 << 53)
        .ok_or_else(invalid)
}

fn nonempty(value: &Value) -> Option<&str> {
    value.as_str().filter(|text| !text.is_empty())
}

/// `parseDeliveryItem`.
fn item(value: &Value) -> Result<Item> {
    let envelope = &value["envelope"];
    if !envelope.is_object()
        || integer(&envelope["schema_version"], false).is_err()
        || nonempty(&envelope["source_id"]).is_none()
        || !envelope["finalized"].is_boolean()
        || !envelope["workspace_id"].is_string()
        || !envelope["session"].is_object()
        || !envelope["session"]["id"].is_string()
        || !envelope["session"]["status"].is_string()
    {
        return Err(invalid());
    }
    let revision = integer(&value["revision"], false)?;
    if integer(&envelope["revision"], false)? != revision {
        return Err(invalid());
    }
    let hash = value["contentHash"].as_str().unwrap_or_default();
    if nonempty(&value["jobId"]).is_none()
        || hash.len() != 64
        || !hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || !value["acknowledged"].is_boolean()
        || value["finalized"].as_bool().is_none()
        || value["finalized"] != envelope["finalized"]
        || value["createdAt"]
            .as_str()
            .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
            .is_none()
    {
        return Err(invalid());
    }
    Ok(Item {
        cursor: integer(&value["cursor"], false)?,
        job_id: value["jobId"].as_str().unwrap_or_default().to_owned(),
        revision,
        finalized: value["finalized"] == true,
        content_hash: hash.to_owned(),
        source_id: envelope["source_id"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        session_id: envelope["session"]["id"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        envelope: envelope.clone(),
    })
}

/// `parseDeliveryPage`: items, the next cursor and whether more remain.
fn page(value: &Value) -> Result<(Vec<Item>, i64, bool)> {
    let items = value["items"]
        .as_array()
        .ok_or_else(invalid)?
        .iter()
        .map(item)
        .collect::<Result<Vec<_>>>()?;
    let next = integer(&value["nextCursor"], true)?;
    let more = value["hasMore"].as_bool().ok_or_else(invalid)?;
    Ok((items, next, more))
}

fn endpoint(server: &Url, path: &str) -> Result<Url> {
    let mut base = server.clone();
    if !base.path().ends_with('/') {
        base.set_path(&format!("{}/", base.path()));
    }
    base.join(path)
        .map_err(|_| failure("Invalid capture server URL"))
}

fn segment(text: &str) -> String {
    url::form_urlencoded::byte_serialize(text.as_bytes())
        .collect::<String>()
        .replace('+', "%20")
}

struct Context<'a> {
    server: &'a Url,
    lease: &'a AuthLease,
    workspace: &'a str,
    consumer: &'a str,
}

impl Core {
    /// `client.ts`'s `request`: bearer auth, a 30-second limit and a 24 MiB bounded body.
    async fn capture_request(
        &self,
        lease: &AuthLease,
        method: Method,
        url: Url,
        body: Option<Value>,
    ) -> Result<Value> {
        let mut header = HeaderValue::from_str(&format!("Bearer {}", lease.access_token()?))
            .map_err(|_| failure("Invalid authorization credential"))?;
        header.set_sensitive(true);
        let mut request = self
            .auth
            .transport
            .client
            .request(method, url)
            .header("Authorization", header)
            .timeout(Duration::from_secs(30));
        if let Some(body) = &body {
            request = request.json(body);
        }
        let response = request.send().await.map_err(|error| {
            if error.is_timeout() {
                failure("The capture server request timed out.")
            } else {
                failure("Capture server request failed; retry when online")
            }
        })?;
        let status = response.status();
        if response
            .content_length()
            .is_some_and(|length| length > MAX_PAGE_BYTES as u64)
        {
            return Err(failure("The capture server response is too large."));
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| failure("Capture server response interrupted"))?;
            if bytes.len() + chunk.len() > MAX_PAGE_BYTES {
                return Err(failure("The capture server response is too large."));
            }
            bytes.extend_from_slice(&chunk);
        }
        lease.check()?;
        let parsed = serde_json::from_slice::<Value>(&bytes);
        if !status.is_success() {
            let error = parsed
                .ok()
                .map(|body| body["error"].clone())
                .unwrap_or_default();
            return Err(failure(error["message"].as_str().map_or_else(
                || format!("Capture server request failed ({}).", status.as_u16()),
                str::to_owned,
            )));
        }
        parsed.map_err(|_| failure("The capture server returned invalid JSON."))
    }

    async fn acknowledge(
        &self,
        context: &Context<'_>,
        job: &str,
        revision: i64,
        hash: &str,
    ) -> Result<()> {
        let url = endpoint(
            context.server,
            &format!(
                "v1/workspaces/{}/session-envelopes/{}/ack",
                segment(context.workspace),
                segment(job)
            ),
        )?;
        let body = self
            .capture_request(
                context.lease,
                Method::POST,
                url,
                Some(json!({"consumerId": context.consumer, "revision": revision, "contentHash": hash})),
            )
            .await?;
        if body["acknowledged"] != true {
            return Err(failure(
                "The capture server returned an invalid acknowledgement.",
            ));
        }
        Ok(())
    }

    /// `syncEnterpriseWorkspace`.
    async fn sync_capture_workspace(
        &self,
        context: &Context<'_>,
        services: &Services,
    ) -> Result<()> {
        let key = (context.server.as_str(), context.workspace, context.consumer);
        for row in query(
            services,
            "SELECT job_id, revision, content_hash FROM enterprise_session_delivery_receipts WHERE server_url = ? AND workspace_id = ? AND consumer_id = ? AND acknowledged_at IS NULL ORDER BY revision, job_id",
            vec![json!(key.0), json!(key.1), json!(key.2)],
        )
        .await?
        {
            let job = row["job_id"].as_str().unwrap_or_default();
            let revision = row["revision"].as_i64().unwrap_or_default();
            self.acknowledge(context, job, revision, row["content_hash"].as_str().unwrap_or_default())
                .await?;
            mark_acknowledged(services, key, job, revision).await?;
        }
        let mut cursor = query(
            services,
            "SELECT cursor FROM enterprise_session_delivery_state WHERE server_url = ? AND workspace_id = ? AND consumer_id = ?",
            vec![json!(key.0), json!(key.1), json!(key.2)],
        )
        .await?
        .first()
        .and_then(|row| row["cursor"].as_i64())
        .unwrap_or(0);
        for _ in 0..MAX_PAGES_PER_SYNC {
            let mut url = endpoint(
                context.server,
                &format!(
                    "v1/workspaces/{}/session-envelopes",
                    segment(context.workspace)
                ),
            )?;
            url.query_pairs_mut()
                .append_pair("consumerId", context.consumer)
                .append_pair("after", &cursor.to_string())
                .append_pair("limit", &PAGE_LIMIT.to_string());
            let (items, next, more) = page(
                &self
                    .capture_request(context.lease, Method::GET, url, None)
                    .await?,
            )?;
            if items.is_empty() {
                return if more {
                    Err(failure("capture delivery cursor did not advance"))
                } else {
                    Ok(())
                };
            }
            for item in items {
                if item.cursor <= cursor || item.envelope["workspace_id"] != context.workspace {
                    return Err(failure(
                        "capture delivery is outside the requested cursor or workspace",
                    ));
                }
                let applied = apply(services, context.workspace, &item.envelope).await?;
                record(services, key, &item, applied).await?;
                self.acknowledge(context, &item.job_id, item.revision, &item.content_hash)
                    .await?;
                mark_acknowledged(services, key, &item.job_id, item.revision).await?;
                cursor = item.cursor;
            }
            if next != cursor {
                return Err(failure(
                    "capture delivery response has an inconsistent cursor",
                ));
            }
            if !more {
                return Ok(());
            }
        }
        Err(failure("capture delivery exceeded the bounded page limit"))
    }

    async fn deliver_captures(&self, server: &Url, services: &Services) -> Result<Delivery> {
        let Some(account) = self.auth.identity().account_id else {
            return Ok(Delivery::default());
        };
        let lease = self.auth.lease(&account, false).await?;
        let mut error = None;
        let mut eligible = Vec::new();
        for membership in self
            .memberships(&lease)
            .await?
            .as_array()
            .into_iter()
            .flatten()
        {
            if membership["workspaces"]["kind"] != "shared" {
                continue;
            }
            let Some(id) = membership["workspace_id"].as_str() else {
                continue;
            };
            let access = self
                .rpc(
                    &lease,
                    "get_workspace_access",
                    json!({"p_workspace_id": id}),
                )
                .await
                .and_then(|access| super::transport::first(&access).cloned());
            match access {
                Ok(access)
                    if access["capabilities"]
                        .as_array()
                        .is_some_and(|all| all.iter().any(|c| c == "enterprise.capture")) =>
                {
                    eligible.push(id.to_owned())
                }
                Ok(_) => {}
                Err(failed) => {
                    error.get_or_insert(failed);
                }
            }
        }
        if eligible.is_empty() {
            return error.map_or_else(|| Ok(Delivery::default()), Err);
        }
        let consumer = self.auth.transport.config.device_fingerprint.clone();
        for workspace in &eligible {
            let context = Context {
                server,
                lease: &lease,
                workspace,
                consumer: &consumer,
            };
            if let Err(failed) = self.sync_capture_workspace(&context, services).await {
                error.get_or_insert(failed);
            }
        }
        let completions = query(
            services,
            "SELECT source_id, session_id FROM enterprise_session_completion_outbox WHERE dispatched_at IS NULL ORDER BY created_at, source_id",
            vec![],
        )
        .await?
        .into_iter()
        .filter_map(|row| {
            Some(Completion {
                source_id: row["source_id"].as_str()?.to_owned(),
                session: SessionId(row["session_id"].as_str()?.into()),
            })
        })
        .collect();
        Ok(Delivery { completions, error })
    }
}

async fn query(services: &Services, sql: &str, params: Vec<Value>) -> Result<Vec<Value>> {
    services
        .executor
        .execute(sql.into(), params)
        .await
        .map_err(failure)
}

/// `applySessionIngest`: `true` when applied (or already applied), `false` when rejected.
async fn apply(services: &Services, workspace: &str, envelope: &Value) -> Result<bool> {
    let Ok(envelope) = serde_json::from_value(envelope.clone()) else {
        tracing::warn!(%workspace, "rejected malformed session ingest envelope");
        return Ok(false);
    };
    match anlg_session_ingest::apply_session_envelope(services.db.pool(), workspace, &envelope)
        .await
    {
        Ok(_) => Ok(true),
        Err(error) if error.is_retryable() => Err(failure(error)),
        Err(error) => {
            tracing::warn!(%workspace, %error, "rejected permanent session ingest envelope");
            Ok(false)
        }
    }
}

/// `recordAppliedDelivery` / `recordRejectedDelivery`: the cursor, a receipt to acknowledge,
/// and for an applied finished meeting, its completion.
async fn record(
    services: &Services,
    key: (&str, &str, &str),
    item: &Item,
    applied: bool,
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let statement = |sql: &str, params: Vec<Value>| anlg_db_execute::TransactionStatement {
        sql: sql.into(),
        params,
        expected_rows_affected: None,
    };
    let mut statements = vec![
        statement(
            "INSERT INTO enterprise_session_delivery_state (server_url, workspace_id, consumer_id, cursor, updated_at) VALUES (?, ?, ?, ?, ?) ON CONFLICT(server_url, workspace_id, consumer_id) DO UPDATE SET cursor = MAX(cursor, excluded.cursor), updated_at = excluded.updated_at",
            vec![
                json!(key.0),
                json!(key.1),
                json!(key.2),
                json!(item.cursor),
                json!(now),
            ],
        ),
        statement(
            "INSERT INTO enterprise_session_delivery_receipts (server_url, workspace_id, consumer_id, job_id, revision, content_hash, acknowledged_at, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, NULL, ?, ?) ON CONFLICT(server_url, workspace_id, consumer_id, job_id, revision) DO UPDATE SET updated_at = excluded.updated_at",
            vec![
                json!(key.0),
                json!(key.1),
                json!(key.2),
                json!(item.job_id),
                json!(item.revision),
                json!(item.content_hash),
                json!(now),
                json!(now),
            ],
        ),
    ];
    if applied && item.finalized {
        statements.push(statement(
            "INSERT INTO enterprise_session_completion_outbox (source_id, workspace_id, session_id, revision, created_at, dispatched_at) VALUES (?, ?, ?, ?, ?, NULL) ON CONFLICT(source_id) DO NOTHING",
            vec![json!(item.source_id), json!(key.1), json!(item.session_id), json!(item.revision), json!(now)],
        ));
    }
    services
        .executor
        .execute_transaction(statements)
        .await
        .map(|_| ())
        .map_err(failure)
}

async fn mark_acknowledged(
    services: &Services,
    key: (&str, &str, &str),
    job: &str,
    revision: i64,
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    query(
        services,
        "UPDATE enterprise_session_delivery_receipts SET acknowledged_at = ?, updated_at = ? WHERE server_url = ? AND workspace_id = ? AND consumer_id = ? AND job_id = ? AND revision = ?",
        vec![json!(now), json!(now), json!(key.0), json!(key.1), json!(key.2), json!(job), json!(revision)],
    )
    .await
    .map(|_| ())
}

impl CloudServices {
    /// One poll of every eligible workspace; the host runs the completions' automations and
    /// then calls `complete_capture`.
    pub fn deliver_captures(&self, server: Url) -> BoxFuture<'static, Result<Delivery>> {
        let this = self.clone();
        Box::pin(async move {
            this.runtime
                .service(move |services| async move {
                    this.core.deliver_captures(&server, &services).await
                })?
                .receive()
                .await
        })
    }

    /// `markCompletionDispatched`.
    pub fn complete_capture(&self, source_id: String) -> BoxFuture<'static, Result<()>> {
        let this = self.clone();
        Box::pin(async move {
            this.runtime
                .service(move |services| async move {
                    query(
                        &services,
                        "UPDATE enterprise_session_completion_outbox SET dispatched_at = ? WHERE source_id = ? AND dispatched_at IS NULL",
                        vec![
                            json!(chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
                            json!(source_id),
                        ],
                    )
                    .await
                    .map(|_| ())
                })?
                .receive()
                .await
        })
    }
}
