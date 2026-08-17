use std::{collections::VecDeque, time::Duration};

use anlg_meeting_capture::{
    BotState, CaptureEvent, CaptureProviderKind, CaptureWorkerCheckpoint, MeetingReference,
};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use url::Url;

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_MAX_ATTEMPTS: u8 = 4;
const DEFAULT_INITIAL_RETRY_DELAY: Duration = Duration::from_millis(200);
const DEFAULT_MAX_RETRY_DELAY: Duration = Duration::from_secs(2);
const MAX_CONTROL_PLANE_RESPONSE_BYTES: usize = 64 * 1024;

#[async_trait]
pub trait CaptureEventSink: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn append(&self, event: &CaptureEvent) -> Result<(), Self::Error>;
}

pub struct ControlPlaneEventSinkConfig {
    pub base_url: Url,
    pub workspace_id: String,
    pub job_id: String,
    pub bearer_token: String,
    pub request_timeout: Duration,
    pub retry: CaptureEventRetryConfig,
}

impl std::fmt::Debug for ControlPlaneEventSinkConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ControlPlaneEventSinkConfig")
            .field("base_url", &self.base_url)
            .field("workspace_id", &self.workspace_id)
            .field("job_id", &self.job_id)
            .field("bearer_token", &"[REDACTED]")
            .field("request_timeout", &self.request_timeout)
            .field("retry", &self.retry)
            .finish()
    }
}

impl ControlPlaneEventSinkConfig {
    pub fn new(
        base_url: Url,
        workspace_id: impl Into<String>,
        job_id: impl Into<String>,
        bearer_token: impl Into<String>,
    ) -> Self {
        Self {
            base_url,
            workspace_id: workspace_id.into(),
            job_id: job_id.into(),
            bearer_token: bearer_token.into(),
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            retry: CaptureEventRetryConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureEventRetryConfig {
    pub max_attempts: u8,
    pub initial_delay: Duration,
    pub max_delay: Duration,
}

impl Default for CaptureEventRetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            initial_delay: DEFAULT_INITIAL_RETRY_DELAY,
            max_delay: DEFAULT_MAX_RETRY_DELAY,
        }
    }
}

pub struct ControlPlaneEventSink {
    client: reqwest::Client,
    job_endpoint: Url,
    event_endpoint: Url,
    claim_endpoint: Url,
    lease_endpoint: Url,
    workspace_id: String,
    job_id: String,
    bearer_token: String,
    retry: CaptureEventRetryConfig,
    lease: tokio::sync::RwLock<Option<WorkerLease>>,
}

impl ControlPlaneEventSink {
    pub fn new(config: ControlPlaneEventSinkConfig) -> Result<Self, CaptureEventSinkConfigError> {
        validate_identifier(&config.workspace_id, "workspace ID")?;
        validate_identifier(&config.job_id, "job ID")?;
        if config.bearer_token.is_empty() || config.bearer_token.chars().any(char::is_control) {
            return Err(CaptureEventSinkConfigError::InvalidBearerToken);
        }
        if config.request_timeout.is_zero() {
            return Err(CaptureEventSinkConfigError::InvalidRequestTimeout);
        }
        validate_retry(config.retry)?;

        let mut job_endpoint = config.base_url;
        if !matches!(job_endpoint.scheme(), "http" | "https")
            || job_endpoint.host_str().is_none()
            || !job_endpoint.username().is_empty()
            || job_endpoint.password().is_some()
            || job_endpoint.query().is_some()
            || job_endpoint.fragment().is_some()
        {
            return Err(CaptureEventSinkConfigError::InvalidBaseUrl);
        }
        job_endpoint
            .path_segments_mut()
            .map_err(|_| CaptureEventSinkConfigError::InvalidBaseUrl)?
            .pop_if_empty()
            .extend([
                "v1",
                "workspaces",
                &config.workspace_id,
                "capture-jobs",
                &config.job_id,
            ]);
        let mut event_endpoint = job_endpoint.clone();
        event_endpoint
            .path_segments_mut()
            .map_err(|_| CaptureEventSinkConfigError::InvalidBaseUrl)?
            .push("events");
        let mut claim_endpoint = job_endpoint.clone();
        claim_endpoint
            .path_segments_mut()
            .map_err(|_| CaptureEventSinkConfigError::InvalidBaseUrl)?
            .push("claim");
        let mut lease_endpoint = job_endpoint.clone();
        lease_endpoint
            .path_segments_mut()
            .map_err(|_| CaptureEventSinkConfigError::InvalidBaseUrl)?
            .push("lease");
        let client = reqwest::Client::builder()
            .timeout(config.request_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(CaptureEventSinkConfigError::Client)?;
        Ok(Self {
            client,
            job_endpoint,
            event_endpoint,
            claim_endpoint,
            lease_endpoint,
            workspace_id: config.workspace_id,
            job_id: config.job_id,
            bearer_token: config.bearer_token,
            retry: config.retry,
            lease: tokio::sync::RwLock::new(None),
        })
    }

    pub async fn read_checkpoint(&self) -> Result<WorkerCheckpoint, CaptureEventSinkError> {
        let mut retry_delay = self.retry.initial_delay;
        for attempt in 1..=self.retry.max_attempts {
            match self
                .client
                .get(self.job_endpoint.clone())
                .bearer_auth(&self.bearer_token)
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    return decode_checkpoint(response, &self.workspace_id, &self.job_id).await;
                }
                Ok(response) => {
                    let status = response.status();
                    if attempt == self.retry.max_attempts || !retryable_status(status) {
                        return Err(CaptureEventSinkError::ControlPlaneStatus(status));
                    }
                }
                Err(error) => {
                    if attempt == self.retry.max_attempts {
                        return Err(CaptureEventSinkError::Request(error));
                    }
                }
            }
            tokio::time::sleep(retry_delay).await;
            retry_delay = retry_delay.saturating_mul(2).min(self.retry.max_delay);
        }
        unreachable!("validated retry configuration always performs at least one attempt")
    }

    pub async fn claim(
        &self,
        worker_id: impl Into<String>,
        lease_id: impl Into<String>,
    ) -> Result<WorkerLease, CaptureEventSinkError> {
        let worker_id = worker_id.into();
        let lease_id = lease_id.into();
        validate_identifier(&worker_id, "worker ID")
            .map_err(|_| CaptureEventSinkError::InvalidLeaseIdentity("worker ID"))?;
        validate_identifier(&lease_id, "lease ID")
            .map_err(|_| CaptureEventSinkError::InvalidLeaseIdentity("lease ID"))?;
        let request = ClaimCaptureJobRequest {
            worker_id: &worker_id,
            lease_id: &lease_id,
        };
        let lease = self
            .send_lease_request(self.claim_endpoint.clone(), &request)
            .await?;
        validate_lease(&lease, &worker_id, &lease_id, None)?;
        *self.lease.write().await = Some(lease.clone());
        Ok(lease)
    }

    pub async fn renew_lease(&self) -> Result<WorkerLease, CaptureEventSinkError> {
        let current = self
            .lease
            .read()
            .await
            .clone()
            .ok_or(CaptureEventSinkError::LeaseRequired)?;
        let identity = WorkerLeaseIdentity::from(&current);
        let request = RenewCaptureJobLeaseRequest { lease: &identity };
        let renewed = match self
            .send_lease_request(self.lease_endpoint.clone(), &request)
            .await
        {
            Ok(renewed) => renewed,
            Err(error) => {
                *self.lease.write().await = None;
                return Err(error);
            }
        };
        if let Err(error) = validate_lease(
            &renewed,
            &current.worker_id,
            &current.lease_id,
            Some(current.epoch),
        ) {
            *self.lease.write().await = None;
            return Err(error);
        }
        *self.lease.write().await = Some(renewed.clone());
        Ok(renewed)
    }

    async fn send_lease_request<T: Serialize + Sync>(
        &self,
        endpoint: Url,
        request: &T,
    ) -> Result<WorkerLease, CaptureEventSinkError> {
        let mut retry_delay = self.retry.initial_delay;
        for attempt in 1..=self.retry.max_attempts {
            match self
                .client
                .post(endpoint.clone())
                .bearer_auth(&self.bearer_token)
                .json(request)
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    return decode_bounded_json(response).await;
                }
                Ok(response) => {
                    let status = response.status();
                    if attempt == self.retry.max_attempts || !retryable_status(status) {
                        return Err(CaptureEventSinkError::ControlPlaneStatus(status));
                    }
                }
                Err(error) => {
                    if attempt == self.retry.max_attempts {
                        return Err(CaptureEventSinkError::Request(error));
                    }
                }
            }
            tokio::time::sleep(retry_delay).await;
            retry_delay = retry_delay.saturating_mul(2).min(self.retry.max_delay);
        }
        unreachable!("validated retry configuration always performs at least one attempt")
    }
}

#[async_trait]
impl CaptureEventSink for ControlPlaneEventSink {
    type Error = CaptureEventSinkError;

    async fn append(&self, event: &CaptureEvent) -> Result<(), CaptureEventSinkError> {
        let lease = self
            .lease
            .read()
            .await
            .clone()
            .ok_or(CaptureEventSinkError::LeaseRequired)?;
        let identity = WorkerLeaseIdentity::from(&lease);
        let request = AppendCaptureEventRequest {
            lease: &identity,
            event,
        };
        let mut retry_delay = self.retry.initial_delay;
        for attempt in 1..=self.retry.max_attempts {
            match self
                .client
                .post(self.event_endpoint.clone())
                .bearer_auth(&self.bearer_token)
                .json(&request)
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => return Ok(()),
                Ok(response) => {
                    let status = response.status();
                    if attempt == self.retry.max_attempts || !retryable_status(status) {
                        return Err(CaptureEventSinkError::ControlPlaneStatus(status));
                    }
                }
                Err(error) => {
                    if attempt == self.retry.max_attempts {
                        return Err(CaptureEventSinkError::Request(error));
                    }
                }
            }
            tokio::time::sleep(retry_delay).await;
            retry_delay = retry_delay.saturating_mul(2).min(self.retry.max_delay);
        }
        unreachable!("validated retry configuration always performs at least one attempt")
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppendCaptureEventRequest<'a> {
    lease: &'a WorkerLeaseIdentity,
    event: &'a CaptureEvent,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ClaimCaptureJobRequest<'a> {
    worker_id: &'a str,
    lease_id: &'a str,
}

#[derive(Serialize)]
struct RenewCaptureJobLeaseRequest<'a> {
    lease: &'a WorkerLeaseIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerLease {
    pub worker_id: String,
    pub lease_id: String,
    pub epoch: u64,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkerLeaseIdentity {
    worker_id: String,
    lease_id: String,
    epoch: u64,
}

impl From<&WorkerLease> for WorkerLeaseIdentity {
    fn from(lease: &WorkerLease) -> Self {
        Self {
            worker_id: lease.worker_id.clone(),
            lease_id: lease.lease_id.clone(),
            epoch: lease.epoch,
        }
    }
}

pub type WorkerCheckpoint = CaptureWorkerCheckpoint;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireCaptureJobCheckpoint {
    job: WireCaptureJob,
    state: BotState,
    next_sequence: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireCaptureJob {
    workspace_id: String,
    job_id: String,
    bot_id: String,
    provider: CaptureProviderKind,
    meeting: MeetingReference,
}

async fn decode_checkpoint(
    response: reqwest::Response,
    expected_workspace_id: &str,
    expected_job_id: &str,
) -> Result<WorkerCheckpoint, CaptureEventSinkError> {
    let checkpoint: WireCaptureJobCheckpoint = decode_bounded_json(response).await?;
    if checkpoint.job.workspace_id != expected_workspace_id {
        return Err(CaptureEventSinkError::InvalidCheckpoint("workspace ID"));
    }
    if checkpoint.job.job_id != expected_job_id {
        return Err(CaptureEventSinkError::InvalidCheckpoint("job ID"));
    }
    validate_identifier(&checkpoint.job.bot_id, "checkpoint bot ID")
        .map_err(|_| CaptureEventSinkError::InvalidCheckpoint("bot ID"))?;
    let checkpoint = WorkerCheckpoint {
        job_id: checkpoint.job.job_id,
        bot_id: checkpoint.job.bot_id,
        provider: checkpoint.job.provider,
        meeting: checkpoint.job.meeting,
        state: checkpoint.state,
        next_sequence: checkpoint.next_sequence,
    };
    checkpoint
        .validate()
        .map_err(|_| CaptureEventSinkError::InvalidCheckpoint("worker checkpoint"))?;
    Ok(checkpoint)
}

async fn decode_bounded_json<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, CaptureEventSinkError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_CONTROL_PLANE_RESPONSE_BYTES as u64)
    {
        return Err(CaptureEventSinkError::ControlPlaneResponseTooLarge);
    }
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(CaptureEventSinkError::ResponseBody)?;
        if body.len().saturating_add(chunk.len()) > MAX_CONTROL_PLANE_RESPONSE_BYTES {
            return Err(CaptureEventSinkError::ControlPlaneResponseTooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(CaptureEventSinkError::InvalidControlPlaneResponse)
}

fn validate_lease(
    lease: &WorkerLease,
    expected_worker_id: &str,
    expected_lease_id: &str,
    expected_epoch: Option<u64>,
) -> Result<(), CaptureEventSinkError> {
    if lease.worker_id != expected_worker_id {
        return Err(CaptureEventSinkError::InvalidLease("worker ID"));
    }
    if lease.lease_id != expected_lease_id {
        return Err(CaptureEventSinkError::InvalidLease("lease ID"));
    }
    if lease.epoch == 0 || expected_epoch.is_some_and(|epoch| epoch != lease.epoch) {
        return Err(CaptureEventSinkError::InvalidLease("epoch"));
    }
    Ok(())
}

pub struct CaptureEventPublisher<S> {
    sink: S,
    pending: VecDeque<CaptureEvent>,
}

impl<S> CaptureEventPublisher<S>
where
    S: CaptureEventSink,
{
    pub fn new(sink: S) -> Self {
        Self {
            sink,
            pending: VecDeque::new(),
        }
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub async fn publish(&mut self, event: CaptureEvent) -> Result<(), S::Error> {
        self.pending.push_back(event);
        self.flush().await
    }

    pub async fn retry_pending(&mut self) -> Result<(), S::Error> {
        self.flush().await
    }

    async fn flush(&mut self) -> Result<(), S::Error> {
        while let Some(event) = self.pending.front() {
            self.sink.append(event).await?;
            self.pending.pop_front();
        }
        Ok(())
    }
}

fn validate_identifier(
    value: &str,
    field: &'static str,
) -> Result<(), CaptureEventSinkConfigError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
    {
        return Err(CaptureEventSinkConfigError::InvalidIdentifier { field });
    }
    Ok(())
}

fn validate_retry(retry: CaptureEventRetryConfig) -> Result<(), CaptureEventSinkConfigError> {
    if retry.max_attempts == 0
        || retry.initial_delay.is_zero()
        || retry.max_delay.is_zero()
        || retry.initial_delay > retry.max_delay
    {
        return Err(CaptureEventSinkConfigError::InvalidRetryConfig);
    }
    Ok(())
}

fn retryable_status(status: StatusCode) -> bool {
    matches!(status.as_u16(), 408 | 425 | 429) || status.is_server_error()
}

#[derive(Debug, thiserror::Error)]
pub enum CaptureEventSinkConfigError {
    #[error(
        "control-plane base URL must be an HTTP(S) URL without credentials, query, or fragment"
    )]
    InvalidBaseUrl,
    #[error("invalid control-plane {field}")]
    InvalidIdentifier { field: &'static str },
    #[error("control-plane bearer token must be non-empty and contain no control characters")]
    InvalidBearerToken,
    #[error("control-plane request timeout must be greater than zero")]
    InvalidRequestTimeout,
    #[error("capture event retry attempts and delays must be non-zero, with initial <= maximum")]
    InvalidRetryConfig,
    #[error("failed to build control-plane HTTP client")]
    Client(#[source] reqwest::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum CaptureEventSinkError {
    #[error("control-plane request failed")]
    Request(#[source] reqwest::Error),
    #[error("control plane rejected the request with HTTP {0}")]
    ControlPlaneStatus(StatusCode),
    #[error("failed to read control-plane response")]
    ResponseBody(#[source] reqwest::Error),
    #[error("control-plane response exceeded 64 KiB")]
    ControlPlaneResponseTooLarge,
    #[error("control-plane response is invalid")]
    InvalidControlPlaneResponse(#[source] serde_json::Error),
    #[error("control-plane checkpoint contains an invalid {0}")]
    InvalidCheckpoint(&'static str),
    #[error("capture worker must hold a lease before publishing events")]
    LeaseRequired,
    #[error("capture lease contains an invalid {0}")]
    InvalidLease(&'static str),
    #[error("capture lease request contains an invalid {0}")]
    InvalidLeaseIdentity(&'static str),
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use anlg_meeting_capture::{CaptureEventPayload, ProviderMetadata, TranscriptSegment};
    use chrono::{DateTime, Utc};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use super::*;

    fn event(sequence: u64) -> CaptureEvent {
        CaptureEvent {
            id: format!("capture-event-{sequence}"),
            bot_id: "bot-1".into(),
            sequence,
            occurred_at: DateTime::parse_from_rfc3339("2026-08-17T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            payload: CaptureEventPayload::Transcript(TranscriptSegment {
                id: format!("segment-{sequence}"),
                sequence,
                start_ms: 0,
                end_ms: Some(100),
                text: "hello".into(),
                speaker: None,
                is_final: true,
            }),
            metadata: ProviderMetadata::default(),
        }
    }

    async fn server(statuses: Vec<StatusCode>) -> (Url, tokio::task::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let mut requests = Vec::new();
            for status in statuses {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut chunk = [0; 4096];
                    let count = stream.read(&mut chunk).await.unwrap();
                    if count == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&chunk[..count]);
                    if complete_request_length(&bytes).is_some_and(|length| bytes.len() >= length) {
                        break;
                    }
                }
                requests.push(String::from_utf8(bytes).unwrap());
                let reason = status.canonical_reason().unwrap_or("Test");
                stream
                    .write_all(
                        format!(
                            "HTTP/1.1 {} {reason}\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}",
                            status.as_u16()
                        )
                        .as_bytes(),
                    )
                    .await
                    .unwrap();
            }
            requests
        });
        (
            Url::parse(&format!("http://{address}/root/")).unwrap(),
            task,
        )
    }

    async fn checkpoint_server(body: String) -> (Url, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut chunk = [0; 4096];
                let count = stream.read(&mut chunk).await.unwrap();
                if count == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..count]);
                if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            String::from_utf8(bytes).unwrap()
        });
        (
            Url::parse(&format!("http://{address}/root/")).unwrap(),
            task,
        )
    }

    async fn json_server(bodies: Vec<String>) -> (Url, tokio::task::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let mut requests = Vec::new();
            for body in bodies {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut chunk = [0; 4096];
                    let count = stream.read(&mut chunk).await.unwrap();
                    if count == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&chunk[..count]);
                    if complete_request_length(&bytes).is_some_and(|length| bytes.len() >= length) {
                        break;
                    }
                }
                requests.push(String::from_utf8(bytes).unwrap());
                stream
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .await
                    .unwrap();
            }
            requests
        });
        (
            Url::parse(&format!("http://{address}/root/")).unwrap(),
            task,
        )
    }

    fn complete_request_length(bytes: &[u8]) -> Option<usize> {
        let headers_end = bytes.windows(4).position(|window| window == b"\r\n\r\n")? + 4;
        let headers = std::str::from_utf8(&bytes[..headers_end]).ok()?;
        let content_length = headers.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })?;
        Some(headers_end + content_length)
    }

    fn sink_config(base_url: Url) -> ControlPlaneEventSinkConfig {
        let mut config = ControlPlaneEventSinkConfig::new(
            base_url,
            "workspace-a",
            "job-1",
            "super-secret-token",
        );
        config.request_timeout = Duration::from_secs(1);
        config.retry = CaptureEventRetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(2),
        };
        config
    }

    async fn install_test_lease(sink: &ControlPlaneEventSink) {
        *sink.lease.write().await = Some(WorkerLease {
            worker_id: "worker-a".into(),
            lease_id: "lease-a".into(),
            epoch: 3,
            expires_at: Utc::now() + chrono::Duration::minutes(1),
        });
    }

    #[tokio::test]
    async fn posts_the_normalized_event_with_workspace_credentials() {
        let (base_url, server) = server(vec![StatusCode::OK]).await;
        let sink = ControlPlaneEventSink::new(sink_config(base_url)).unwrap();
        install_test_lease(&sink).await;

        sink.append(&event(1)).await.unwrap();

        let requests = server.await.unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].starts_with(
            "POST /root/v1/workspaces/workspace-a/capture-jobs/job-1/events HTTP/1.1"
        ));
        assert!(
            requests[0]
                .to_ascii_lowercase()
                .contains("authorization: bearer super-secret-token")
        );
        assert!(
            requests[0].contains(
                "\"lease\":{\"workerId\":\"worker-a\",\"leaseId\":\"lease-a\",\"epoch\":3}"
            )
        );
        assert!(requests[0].contains("\"event\":{\"id\":\"capture-event-1\""));
    }

    #[tokio::test]
    async fn requires_a_worker_lease_before_event_delivery() {
        let sink = ControlPlaneEventSink::new(sink_config(
            Url::parse("http://127.0.0.1:1/root/").unwrap(),
        ))
        .unwrap();

        assert!(matches!(
            sink.append(&event(1)).await,
            Err(CaptureEventSinkError::LeaseRequired)
        ));
    }

    #[tokio::test]
    async fn claims_and_renews_the_fenced_worker_lease() {
        let lease = serde_json::json!({
            "workerId": "worker-a",
            "leaseId": "lease-a",
            "epoch": 3,
            "expiresAt": "2026-08-17T00:01:00Z"
        })
        .to_string();
        let renewed = serde_json::json!({
            "workerId": "worker-a",
            "leaseId": "lease-a",
            "epoch": 3,
            "expiresAt": "2026-08-17T00:02:00Z"
        })
        .to_string();
        let (base_url, server) = json_server(vec![lease, renewed]).await;
        let sink = ControlPlaneEventSink::new(sink_config(base_url)).unwrap();

        let claimed = sink.claim("worker-a", "lease-a").await.unwrap();
        let renewed = sink.renew_lease().await.unwrap();

        assert_eq!(claimed.epoch, 3);
        assert!(renewed.expires_at > claimed.expires_at);
        let requests = server.await.unwrap();
        assert!(
            requests[0].starts_with(
                "POST /root/v1/workspaces/workspace-a/capture-jobs/job-1/claim HTTP/1.1"
            )
        );
        assert!(requests[0].contains("\"workerId\":\"worker-a\",\"leaseId\":\"lease-a\""));
        assert!(
            requests[1].starts_with(
                "POST /root/v1/workspaces/workspace-a/capture-jobs/job-1/lease HTTP/1.1"
            )
        );
        assert!(
            requests[1].contains(
                "\"lease\":{\"workerId\":\"worker-a\",\"leaseId\":\"lease-a\",\"epoch\":3}"
            )
        );
    }

    #[tokio::test]
    async fn stops_event_delivery_after_lease_renewal_is_rejected() {
        let (base_url, server) = server(vec![StatusCode::CONFLICT]).await;
        let sink = ControlPlaneEventSink::new(sink_config(base_url)).unwrap();
        install_test_lease(&sink).await;

        assert!(matches!(
            sink.renew_lease().await,
            Err(CaptureEventSinkError::ControlPlaneStatus(
                StatusCode::CONFLICT
            ))
        ));
        assert!(matches!(
            sink.append(&event(1)).await,
            Err(CaptureEventSinkError::LeaseRequired)
        ));
        assert_eq!(server.await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn reads_and_validates_the_durable_worker_checkpoint() {
        let body = serde_json::json!({
            "job": {
                "workspaceId": "workspace-a",
                "jobId": "job-1",
                "botId": "bot-1",
                "ownerUserId": "owner-a",
                "requestingActorId": "actor-a",
                "sessionId": "session-a",
                "sessionTitle": "Architecture review",
                "provider": "anarlog",
                "meeting": {
                    "platform": "google_meet",
                    "url": "https://meet.google.com/abc-defg-hij"
                },
                "createdAt": "2026-08-17T00:00:00Z"
            },
            "state": "capturing",
            "nextSequence": 7
        })
        .to_string();
        let (base_url, server) = checkpoint_server(body).await;
        let sink = ControlPlaneEventSink::new(sink_config(base_url)).unwrap();

        let checkpoint = sink.read_checkpoint().await.unwrap();

        assert_eq!(checkpoint.job_id, "job-1");
        assert_eq!(checkpoint.bot_id, "bot-1");
        assert_eq!(
            checkpoint.meeting.url,
            "https://meet.google.com/abc-defg-hij"
        );
        assert_eq!(checkpoint.state, BotState::Capturing);
        assert_eq!(checkpoint.next_sequence, 7);
        let request = server.await.unwrap();
        assert!(
            request.starts_with("GET /root/v1/workspaces/workspace-a/capture-jobs/job-1 HTTP/1.1")
        );
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer super-secret-token")
        );
    }

    #[tokio::test]
    async fn rejects_an_oversized_checkpoint_before_deserializing_it() {
        let (base_url, server) =
            checkpoint_server("x".repeat(MAX_CONTROL_PLANE_RESPONSE_BYTES + 1)).await;
        let sink = ControlPlaneEventSink::new(sink_config(base_url)).unwrap();

        assert!(matches!(
            sink.read_checkpoint().await,
            Err(CaptureEventSinkError::ControlPlaneResponseTooLarge)
        ));

        server.await.unwrap();
    }

    #[tokio::test]
    async fn retries_a_transient_response_with_the_identical_event() {
        let (base_url, server) =
            server(vec![StatusCode::SERVICE_UNAVAILABLE, StatusCode::OK]).await;
        let sink = ControlPlaneEventSink::new(sink_config(base_url)).unwrap();
        install_test_lease(&sink).await;

        sink.append(&event(1)).await.unwrap();

        let requests = server.await.unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests[0].split("\r\n\r\n").nth(1),
            requests[1].split("\r\n\r\n").nth(1)
        );
    }

    #[tokio::test]
    async fn does_not_retry_a_sequence_conflict() {
        let (base_url, server) = server(vec![StatusCode::CONFLICT]).await;
        let sink = ControlPlaneEventSink::new(sink_config(base_url)).unwrap();
        install_test_lease(&sink).await;

        assert!(matches!(
            sink.append(&event(1)).await,
            Err(CaptureEventSinkError::ControlPlaneStatus(
                StatusCode::CONFLICT
            ))
        ));

        assert_eq!(server.await.unwrap().len(), 1);
    }

    #[derive(Default)]
    struct RecoveringSink {
        attempts: Arc<Mutex<Vec<CaptureEvent>>>,
    }

    #[derive(Debug, thiserror::Error)]
    #[error("offline")]
    struct RecoveringSinkError;

    #[async_trait]
    impl CaptureEventSink for RecoveringSink {
        type Error = RecoveringSinkError;

        async fn append(&self, event: &CaptureEvent) -> Result<(), Self::Error> {
            let mut attempts = self.attempts.lock().unwrap();
            attempts.push(event.clone());
            if attempts.len() == 1 {
                Err(RecoveringSinkError)
            } else {
                Ok(())
            }
        }
    }

    #[tokio::test]
    async fn retains_an_unacknowledged_event_for_an_identical_retry() {
        let sink = RecoveringSink::default();
        let attempts = sink.attempts.clone();
        let mut publisher = CaptureEventPublisher::new(sink);

        assert!(publisher.publish(event(1)).await.is_err());
        assert_eq!(publisher.pending_count(), 1);
        publisher.retry_pending().await.unwrap();
        assert_eq!(publisher.pending_count(), 0);

        let attempts = attempts.lock().unwrap();
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0], attempts[1]);
    }

    #[test]
    fn validates_configuration_without_exposing_the_token() {
        let config = sink_config(Url::parse("http://127.0.0.1:8000").unwrap());
        let debug = format!("{config:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("super-secret-token"));

        let mut invalid = sink_config(Url::parse("http://127.0.0.1:8000").unwrap());
        invalid.workspace_id = "../../other".into();
        assert!(matches!(
            ControlPlaneEventSink::new(invalid),
            Err(CaptureEventSinkConfigError::InvalidIdentifier { .. })
        ));
    }
}
