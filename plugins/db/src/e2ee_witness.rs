use std::io;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

const MAX_EVENTS_PER_BATCH: usize = 16;
const MAX_EVENT_BYTES: usize = 16 * 1024 * 1024;
const MAX_BATCH_BYTES: usize = 48 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

#[derive(Clone)]
pub(crate) struct E2eeWitnessClient {
    client: reqwest::Client,
    endpoint: reqwest::Url,
    access_token: String,
    workspace_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublishRequest<'a> {
    initialize: bool,
    events: Vec<PublishEvent<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublishEvent<'a> {
    record_id: &'a str,
    payload_hash: &'a str,
    payload: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PublishResponse {
    initialized_at: String,
    head_sequence: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadPage {
    initialized: bool,
    initialized_at: Option<String>,
    head_sequence: u64,
    through_sequence: u64,
    next_after_sequence: u64,
    events: Vec<ReadEvent>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadEvent {
    sequence: u64,
    record_id: String,
    payload_hash: String,
    payload: String,
}

impl E2eeWitnessClient {
    pub(crate) fn new(config: crate::CloudsyncE2eeWitness, workspace_id: &str) -> io::Result<Self> {
        let endpoint = reqwest::Url::parse(&config.endpoint)
            .map_err(|_| invalid_data("E2EE witness endpoint is invalid"))?;
        if !matches!(endpoint.scheme(), "https" | "http")
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
            || endpoint.path_segments().and_then(Iterator::last) != Some(workspace_id)
            || config.access_token.is_empty()
        {
            return Err(invalid_data("E2EE witness configuration is invalid"));
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|error| io::Error::other(format!("E2EE witness client failed: {error}")))?;
        Ok(Self {
            client,
            endpoint,
            access_token: config.access_token,
            workspace_id: workspace_id.to_string(),
        })
    }

    pub(crate) fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    pub(crate) async fn initialize(
        &self,
        pool: &sqlx::SqlitePool,
        key: &hypr_e2ee::WorkspaceKey,
    ) -> io::Result<()> {
        let cursor = witness_cursor(pool, &self.workspace_id).await?;
        let status = self.read_page(cursor, None).await?;
        self.validate_page(&status, cursor, None)?;
        if status.head_sequence < cursor {
            return Err(rollback_error());
        }

        if status.initialized {
            self.publish_pending(pool, key, false).await?;
        } else {
            if !hypr_db_app::has_e2ee_local_state(pool, &self.workspace_id)
                .await
                .map_err(replica_error)?
            {
                return Err(io::Error::other(
                    "E2EE freshness witness must be initialized from an existing trusted device",
                ));
            }
            self.publish_pending(pool, key, true).await?;
        }

        self.refresh(pool, key).await
    }

    pub(crate) async fn publish_and_refresh(
        &self,
        pool: &sqlx::SqlitePool,
        key: &hypr_e2ee::WorkspaceKey,
    ) -> io::Result<()> {
        self.publish_pending(pool, key, false).await?;
        self.refresh(pool, key).await
    }

    pub(crate) async fn refresh(
        &self,
        pool: &sqlx::SqlitePool,
        key: &hypr_e2ee::WorkspaceKey,
    ) -> io::Result<()> {
        let cursor = witness_cursor(pool, &self.workspace_id).await?;
        let mut page = self.read_page(cursor, None).await?;
        self.validate_page(&page, cursor, None)?;
        if !page.initialized {
            return Err(io::Error::other(
                "E2EE freshness witness is not initialized",
            ));
        }
        if page.head_sequence < cursor {
            return Err(rollback_error());
        }

        let through = page.through_sequence;
        loop {
            let events = page
                .events
                .into_iter()
                .map(|event| hypr_db_app::E2eeWitnessEvent {
                    sequence: event.sequence,
                    record_id: event.record_id,
                    workspace_id: self.workspace_id.clone(),
                    payload_hash: event.payload_hash,
                    payload: event.payload,
                })
                .collect::<Vec<_>>();
            hypr_db_app::merge_e2ee_witness_events(pool, key, &self.workspace_id, &events)
                .await
                .map_err(replica_error)?;
            let after = page.next_after_sequence;
            if after == through {
                break;
            }
            if after >= through {
                return Err(invalid_data("E2EE witness page cursor is invalid"));
            }
            page = self.read_page(after, Some(through)).await?;
            self.validate_page(&page, after, Some(through))?;
        }

        hypr_db_app::advance_e2ee_witness_cursor(pool, &self.workspace_id, through)
            .await
            .map_err(replica_error)
    }

    async fn publish_pending(
        &self,
        pool: &sqlx::SqlitePool,
        key: &hypr_e2ee::WorkspaceKey,
        initialize: bool,
    ) -> io::Result<()> {
        let uploads = hypr_db_app::pending_e2ee_witness_uploads(pool, &self.workspace_id, key)
            .await
            .map_err(replica_error)?;
        if initialize && uploads.is_empty() {
            return Err(io::Error::other(
                "E2EE freshness initialization requires established encrypted state",
            ));
        }
        let cursor = witness_cursor(pool, &self.workspace_id).await?;
        let mut start = 0;
        while start < uploads.len() {
            let mut end = start;
            let mut batch_bytes = 0usize;
            while end < uploads.len() && end - start < MAX_EVENTS_PER_BATCH {
                let event_bytes = uploads[end]
                    .payload
                    .len()
                    .saturating_add(uploads[end].record_id.len())
                    .saturating_add(uploads[end].payload_hash.len())
                    .saturating_add(256);
                if uploads[end].payload.len() > MAX_EVENT_BYTES {
                    return Err(invalid_data("E2EE witness event is too large"));
                }
                if end > start && batch_bytes.saturating_add(event_bytes) > MAX_BATCH_BYTES {
                    break;
                }
                batch_bytes = batch_bytes.saturating_add(event_bytes);
                end += 1;
            }
            let batch = &uploads[start..end];
            let response = self
                .client
                .post(self.endpoint.clone())
                .bearer_auth(&self.access_token)
                .json(&PublishRequest {
                    initialize: initialize && start == 0,
                    events: batch
                        .iter()
                        .map(|upload| PublishEvent {
                            record_id: &upload.record_id,
                            payload_hash: &upload.payload_hash,
                            payload: &upload.payload,
                        })
                        .collect(),
                })
                .send()
                .await
                .map_err(transport_error)?;
            let status = response.status();
            let bytes = read_bounded(response).await?;
            if !status.is_success() {
                return Err(io::Error::other(format!(
                    "E2EE witness publication was rejected with status {status}"
                )));
            }
            let response: PublishResponse = serde_json::from_slice(&bytes)
                .map_err(|_| invalid_data("E2EE witness publication response is invalid"))?;
            if response.initialized_at.is_empty() || response.head_sequence < cursor {
                return Err(rollback_error());
            }
            hypr_db_app::acknowledge_e2ee_witness_uploads(pool, key, batch)
                .await
                .map_err(replica_error)?;
            start = end;
        }
        Ok(())
    }

    async fn read_page(&self, after: u64, through: Option<u64>) -> io::Result<ReadPage> {
        let mut request = self
            .client
            .get(self.endpoint.clone())
            .bearer_auth(&self.access_token)
            .query(&[("afterSequence", after)]);
        if let Some(through) = through {
            request = request.query(&[("throughSequence", through)]);
        }
        let response = request.send().await.map_err(transport_error)?;
        let status = response.status();
        let bytes = read_bounded(response).await?;
        if !status.is_success() {
            return Err(io::Error::other(format!(
                "E2EE witness read was rejected with status {status}"
            )));
        }
        serde_json::from_slice(&bytes)
            .map_err(|_| invalid_data("E2EE witness read response is invalid"))
    }

    fn validate_page(
        &self,
        page: &ReadPage,
        requested_after: u64,
        requested_through: Option<u64>,
    ) -> io::Result<()> {
        if page.initialized != page.initialized_at.is_some()
            || page.through_sequence > page.head_sequence
            || requested_after > page.through_sequence
            || requested_through.is_some_and(|through| through != page.through_sequence)
            || page.next_after_sequence < requested_after
            || page.next_after_sequence > page.through_sequence
            || (page.events.is_empty() && page.next_after_sequence != requested_after)
            || (page.events.is_empty() && requested_after != page.through_sequence)
            || page
                .events
                .last()
                .is_some_and(|event| event.sequence != page.next_after_sequence)
        {
            return Err(invalid_data("E2EE witness page is invalid"));
        }
        let mut previous = requested_after;
        for event in &page.events {
            if event.sequence <= previous
                || event.sequence > page.through_sequence
                || event.payload.is_empty()
                || event.payload.len() > MAX_EVENT_BYTES
            {
                return Err(invalid_data("E2EE witness event is invalid"));
            }
            previous = event.sequence;
        }
        Ok(())
    }
}

async fn witness_cursor(pool: &sqlx::SqlitePool, workspace_id: &str) -> io::Result<u64> {
    hypr_db_app::e2ee_witness_cursor(pool, workspace_id)
        .await
        .map_err(replica_error)
}

async fn read_bounded(response: reqwest::Response) -> io::Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(invalid_data("E2EE witness response is too large"));
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(transport_error)?;
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(invalid_data("E2EE witness response is too large"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn replica_error(error: hypr_db_app::E2eeReplicaError) -> io::Error {
    io::Error::other(format!("E2EE witness state failed: {error}"))
}

fn transport_error(error: reqwest::Error) -> io::Error {
    io::Error::other(format!("E2EE witness request failed: {error}"))
}

fn invalid_data(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn rollback_error() -> io::Error {
    io::Error::other("E2EE freshness witness rollback was detected")
}
