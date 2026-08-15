use std::future::Future;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

const MAX_EVENTS_PER_BATCH: usize = 16;
const MAX_EVENT_BYTES: usize = 16 * 1024 * 1024;
const MAX_BATCH_BYTES: usize = 48 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
const MAX_RATE_LIMIT_RETRIES: usize = 3;
const DEFAULT_RETRY_AFTER: std::time::Duration = std::time::Duration::from_secs(30);
const MAX_RETRY_AFTER: std::time::Duration = std::time::Duration::from_secs(60);
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

#[derive(Clone, Default)]
pub(crate) struct E2eeWitnessCancellation {
    state: Arc<E2eeWitnessCancellationState>,
}

#[derive(Default)]
struct E2eeWitnessCancellationState {
    cancelled: AtomicBool,
    changed: tokio::sync::Notify,
}

impl E2eeWitnessCancellation {
    pub(crate) fn cancel(&self) {
        if !self.state.cancelled.swap(true, Ordering::AcqRel) {
            self.state.changed.notify_waiters();
        }
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.state.cancelled.load(Ordering::Acquire)
    }

    pub(crate) fn check(&self) -> io::Result<()> {
        if self.is_cancelled() {
            Err(cancelled_error())
        } else {
            Ok(())
        }
    }

    pub(crate) async fn cancelled(&self) {
        loop {
            let changed = self.state.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.is_cancelled() {
                return;
            }
            changed.await;
        }
    }

    async fn run_network<T>(&self, future: impl Future<Output = T>) -> io::Result<T> {
        self.check()?;
        tokio::pin!(future);
        tokio::select! {
            biased;
            _ = self.cancelled() => Err(cancelled_error()),
            result = &mut future => Ok(result),
        }
    }
}

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WaitResponse {
    initialized: bool,
    head_sequence: u64,
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

    pub(crate) fn for_workspace(&self, workspace_id: &str) -> io::Result<Self> {
        if workspace_id.is_empty()
            || workspace_id.contains('/')
            || workspace_id.contains('?')
            || workspace_id.contains('#')
        {
            return Err(invalid_data("E2EE witness workspace is invalid"));
        }
        let mut endpoint = self.endpoint.clone();
        let mut segments = endpoint
            .path_segments_mut()
            .map_err(|()| invalid_data("E2EE witness endpoint is invalid"))?;
        segments.pop_if_empty();
        segments.pop();
        segments.push(workspace_id);
        drop(segments);
        Ok(Self {
            client: self.client.clone(),
            endpoint,
            access_token: self.access_token.clone(),
            workspace_id: workspace_id.to_string(),
        })
    }

    pub(crate) fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    #[cfg(test)]
    pub(crate) async fn initialize(
        &self,
        pool: &sqlx::SqlitePool,
        key: &anlg_e2ee::WorkspaceKey,
    ) -> io::Result<()> {
        self.initialize_cancellable(pool, key, &E2eeWitnessCancellation::default())
            .await
    }

    pub(crate) async fn initialize_cancellable(
        &self,
        pool: &sqlx::SqlitePool,
        key: &anlg_e2ee::WorkspaceKey,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<()> {
        let keyring = anlg_e2ee::WorkspaceKeyring::new(key.clone());
        self.initialize_keyring_cancellable(pool, &keyring, cancellation)
            .await
    }

    pub(crate) async fn initialize_keyring_cancellable(
        &self,
        pool: &sqlx::SqlitePool,
        keyring: &anlg_e2ee::WorkspaceKeyring,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<()> {
        let cursor = witness_cursor_cancellable(pool, &self.workspace_id, cancellation).await?;
        let status = self
            .read_page_cancellable(cursor, None, cancellation)
            .await?;
        self.validate_page(&status, cursor, None)?;
        if status.head_sequence < cursor {
            return Err(rollback_error());
        }

        if status.initialized {
            self.refresh_keyring_cancellable(pool, keyring, cancellation)
                .await?;
            self.publish_pending(pool, keyring.active(), false, cancellation)
                .await?;
        } else {
            cancellation.check()?;
            let has_local_state = anlg_db_app::has_e2ee_local_state(pool, &self.workspace_id)
                .await
                .map_err(replica_error)?;
            cancellation.check()?;
            if !has_local_state {
                return Err(io::Error::other(
                    "E2EE freshness witness must be initialized from an existing trusted device",
                ));
            }
            self.publish_pending(pool, keyring.active(), true, cancellation)
                .await?;
        }

        self.refresh_keyring_cancellable(pool, keyring, cancellation)
            .await
            .map(|_| ())
    }

    #[cfg(test)]
    pub(crate) async fn publish_and_refresh(
        &self,
        pool: &sqlx::SqlitePool,
        key: &anlg_e2ee::WorkspaceKey,
    ) -> io::Result<usize> {
        self.publish_and_refresh_cancellable(pool, key, &E2eeWitnessCancellation::default())
            .await
    }

    pub(crate) async fn publish_and_refresh_cancellable(
        &self,
        pool: &sqlx::SqlitePool,
        key: &anlg_e2ee::WorkspaceKey,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<usize> {
        let keyring = anlg_e2ee::WorkspaceKeyring::new(key.clone());
        self.publish_and_refresh_keyring_cancellable(pool, &keyring, cancellation)
            .await
    }

    pub(crate) async fn publish_and_refresh_keyring_cancellable(
        &self,
        pool: &sqlx::SqlitePool,
        keyring: &anlg_e2ee::WorkspaceKeyring,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<usize> {
        self.publish_pending(pool, keyring.active(), false, cancellation)
            .await?;
        self.refresh_keyring_cancellable(pool, keyring, cancellation)
            .await
    }

    pub(crate) async fn publish_and_refresh_notifying_cancellable<F>(
        &self,
        pool: &sqlx::SqlitePool,
        key: &anlg_e2ee::WorkspaceKey,
        on_events: F,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<usize>
    where
        F: FnMut(),
    {
        let keyring = anlg_e2ee::WorkspaceKeyring::new(key.clone());
        self.publish_and_refresh_keyring_notifying_cancellable(
            pool,
            &keyring,
            on_events,
            cancellation,
        )
        .await
    }

    pub(crate) async fn publish_and_refresh_keyring_notifying_cancellable<F>(
        &self,
        pool: &sqlx::SqlitePool,
        keyring: &anlg_e2ee::WorkspaceKeyring,
        on_events: F,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<usize>
    where
        F: FnMut(),
    {
        self.publish_pending(pool, keyring.active(), false, cancellation)
            .await?;
        self.refresh_keyring_notifying_cancellable(pool, keyring, on_events, cancellation)
            .await
    }

    #[cfg(test)]
    pub(crate) async fn refresh(
        &self,
        pool: &sqlx::SqlitePool,
        key: &anlg_e2ee::WorkspaceKey,
    ) -> io::Result<usize> {
        self.refresh_cancellable(pool, key, &E2eeWitnessCancellation::default())
            .await
    }

    pub(crate) async fn refresh_cancellable(
        &self,
        pool: &sqlx::SqlitePool,
        key: &anlg_e2ee::WorkspaceKey,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<usize> {
        let keyring = anlg_e2ee::WorkspaceKeyring::new(key.clone());
        self.refresh_keyring_cancellable(pool, &keyring, cancellation)
            .await
    }

    pub(crate) async fn refresh_keyring_cancellable(
        &self,
        pool: &sqlx::SqlitePool,
        keyring: &anlg_e2ee::WorkspaceKeyring,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<usize> {
        self.refresh_keyring_notifying_cancellable(pool, keyring, || {}, cancellation)
            .await
    }

    #[cfg(test)]
    pub(crate) async fn refresh_notifying<F>(
        &self,
        pool: &sqlx::SqlitePool,
        key: &anlg_e2ee::WorkspaceKey,
        on_events: F,
    ) -> io::Result<usize>
    where
        F: FnMut(),
    {
        self.refresh_notifying_cancellable(
            pool,
            key,
            on_events,
            &E2eeWitnessCancellation::default(),
        )
        .await
    }

    pub(crate) async fn refresh_notifying_cancellable<F>(
        &self,
        pool: &sqlx::SqlitePool,
        key: &anlg_e2ee::WorkspaceKey,
        on_events: F,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<usize>
    where
        F: FnMut(),
    {
        let keyring = anlg_e2ee::WorkspaceKeyring::new(key.clone());
        self.refresh_keyring_notifying_cancellable(pool, &keyring, on_events, cancellation)
            .await
    }

    pub(crate) async fn refresh_keyring_notifying_cancellable<F>(
        &self,
        pool: &sqlx::SqlitePool,
        keyring: &anlg_e2ee::WorkspaceKeyring,
        mut on_events: F,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<usize>
    where
        F: FnMut(),
    {
        let mut cursor = witness_cursor_cancellable(pool, &self.workspace_id, cancellation).await?;
        let mut page = self
            .read_page_cancellable(cursor, None, cancellation)
            .await?;
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
        let mut received_events = 0_usize;
        loop {
            received_events = received_events.saturating_add(page.events.len());
            if !page.events.is_empty() {
                on_events();
            }
            let events = page
                .events
                .into_iter()
                .map(|event| anlg_db_app::E2eeWitnessEvent {
                    sequence: event.sequence,
                    record_id: event.record_id,
                    workspace_id: self.workspace_id.clone(),
                    payload_hash: event.payload_hash,
                    payload: event.payload,
                })
                .collect::<Vec<_>>();
            cancellation.check()?;
            anlg_db_app::merge_e2ee_witness_events_with_keyring_cancellable(
                pool,
                keyring,
                &self.workspace_id,
                &events,
                || cancellation.is_cancelled(),
            )
            .await
            .map_err(replica_error)?;
            cancellation.check()?;
            let after = page.next_after_sequence;
            if after != cursor {
                cancellation.check()?;
                anlg_db_app::advance_e2ee_witness_cursor(pool, &self.workspace_id, after)
                    .await
                    .map_err(replica_error)?;
                cancellation.check()?;
                cursor = after;
            }
            if after == through {
                break;
            }
            if after >= through {
                return Err(invalid_data("E2EE witness page cursor is invalid"));
            }
            page = self
                .read_page_cancellable(after, Some(through), cancellation)
                .await?;
            self.validate_page(&page, after, Some(through))?;
        }
        Ok(received_events)
    }

    async fn publish_pending(
        &self,
        pool: &sqlx::SqlitePool,
        key: &anlg_e2ee::WorkspaceKey,
        initialize: bool,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<()> {
        let cursor = witness_cursor_cancellable(pool, &self.workspace_id, cancellation).await?;
        let mut first_batch = true;
        loop {
            cancellation.check()?;
            let uploads = anlg_db_app::pending_e2ee_witness_uploads_cancellable(
                pool,
                &self.workspace_id,
                key,
                MAX_EVENTS_PER_BATCH,
                MAX_BATCH_BYTES,
                || cancellation.is_cancelled(),
            )
            .await
            .map_err(replica_error)?;
            cancellation.check()?;
            if uploads.is_empty() {
                if initialize && first_batch {
                    return Err(io::Error::other(
                        "E2EE freshness initialization requires established encrypted state",
                    ));
                }
                return Ok(());
            }

            let mut batch_bytes = 0usize;
            for upload in &uploads {
                cancellation.check()?;
                let event_bytes = upload
                    .payload
                    .len()
                    .saturating_add(upload.record_id.len())
                    .saturating_add(upload.payload_hash.len())
                    .saturating_add(256);
                if upload.payload.len() > MAX_EVENT_BYTES {
                    return Err(invalid_data("E2EE witness event is too large"));
                }
                if batch_bytes.saturating_add(event_bytes) > MAX_BATCH_BYTES {
                    return Err(invalid_data("E2EE witness batch is too large"));
                }
                batch_bytes = batch_bytes.saturating_add(event_bytes);
            }
            let response = self
                .send_with_rate_limit_retry(
                    || {
                        self.client
                            .post(self.endpoint.clone())
                            .bearer_auth(&self.access_token)
                            .json(&PublishRequest {
                                initialize: initialize && first_batch,
                                events: uploads
                                    .iter()
                                    .map(|upload| PublishEvent {
                                        record_id: &upload.record_id,
                                        payload_hash: &upload.payload_hash,
                                        payload: &upload.payload,
                                    })
                                    .collect(),
                            })
                    },
                    cancellation,
                )
                .await?;
            let status = response.status();
            let bytes = cancellation.run_network(read_bounded(response)).await??;
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
            cancellation.check()?;
            anlg_db_app::acknowledge_e2ee_witness_uploads_cancellable(pool, key, &uploads, || {
                cancellation.is_cancelled()
            })
            .await
            .map_err(replica_error)?;
            cancellation.check()?;
            first_batch = false;
        }
    }

    #[cfg(test)]
    async fn read_page(&self, after: u64, through: Option<u64>) -> io::Result<ReadPage> {
        self.read_page_cancellable(after, through, &E2eeWitnessCancellation::default())
            .await
    }

    /// Long-poll the witness service until its head advances past `after` or
    /// the server's hold expires. Returns the new head when it advanced.
    pub(crate) async fn wait_for_remote_head(
        &self,
        after: u64,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<Option<u64>> {
        let mut endpoint = self.endpoint.clone();
        endpoint
            .path_segments_mut()
            .map_err(|()| invalid_data("E2EE witness endpoint is invalid"))?
            .push("wait");
        let response = self
            .send_with_rate_limit_retry(
                || {
                    self.client
                        .get(endpoint.clone())
                        .bearer_auth(&self.access_token)
                        .query(&[("afterSequence", after)])
                },
                cancellation,
            )
            .await?;
        let status = response.status();
        let bytes = cancellation.run_network(read_bounded(response)).await??;
        if !status.is_success() {
            return Err(io::Error::other(format!(
                "E2EE witness wait was rejected with status {status}"
            )));
        }
        let response: WaitResponse = serde_json::from_slice(&bytes)
            .map_err(|_| invalid_data("E2EE witness wait response is invalid"))?;
        Ok((response.initialized && response.head_sequence > after)
            .then_some(response.head_sequence))
    }

    async fn read_page_cancellable(
        &self,
        after: u64,
        through: Option<u64>,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<ReadPage> {
        let response = self
            .send_with_rate_limit_retry(
                || {
                    let mut request = self
                        .client
                        .get(self.endpoint.clone())
                        .bearer_auth(&self.access_token)
                        .query(&[("afterSequence", after)]);
                    if let Some(through) = through {
                        request = request.query(&[("throughSequence", through)]);
                    }
                    request
                },
                cancellation,
            )
            .await?;
        let status = response.status();
        let bytes = cancellation.run_network(read_bounded(response)).await??;
        if !status.is_success() {
            return Err(io::Error::other(format!(
                "E2EE witness read was rejected with status {status}"
            )));
        }
        serde_json::from_slice(&bytes)
            .map_err(|_| invalid_data("E2EE witness read response is invalid"))
    }

    async fn send_with_rate_limit_retry(
        &self,
        request: impl Fn() -> reqwest::RequestBuilder,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            let response = cancellation
                .run_network(request().send())
                .await?
                .map_err(transport_error)?;
            if response.status() != reqwest::StatusCode::TOO_MANY_REQUESTS
                || retries == MAX_RATE_LIMIT_RETRIES
            {
                return Ok(response);
            }
            let delay = retry_after_delay(response.headers());
            cancellation.run_network(read_bounded(response)).await??;
            cancellation.run_network(tokio::time::sleep(delay)).await?;
            retries += 1;
        }
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
    anlg_db_app::e2ee_witness_cursor(pool, workspace_id)
        .await
        .map_err(replica_error)
}

async fn witness_cursor_cancellable(
    pool: &sqlx::SqlitePool,
    workspace_id: &str,
    cancellation: &E2eeWitnessCancellation,
) -> io::Result<u64> {
    cancellation.check()?;
    let cursor = witness_cursor(pool, workspace_id).await?;
    cancellation.check()?;
    Ok(cursor)
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

fn replica_error(error: anlg_db_app::E2eeReplicaError) -> io::Error {
    if matches!(&error, anlg_db_app::E2eeReplicaError::Cancelled) {
        cancelled_error()
    } else {
        io::Error::other(format!("E2EE witness state failed: {error}"))
    }
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

fn cancelled_error() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, "E2EE witness request cancelled")
}

fn retry_after_delay(headers: &reqwest::header::HeaderMap) -> std::time::Duration {
    let seconds = headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    match seconds {
        None => DEFAULT_RETRY_AFTER,
        Some(0) => std::time::Duration::ZERO,
        Some(seconds) => std::time::Duration::from_secs(seconds)
            .saturating_add(std::time::Duration::from_secs(1))
            .min(MAX_RETRY_AFTER),
    }
}

#[cfg(test)]
mod tests;
