use std::{collections::HashMap, sync::Arc};

use anarlog_enterprise_control_plane::{
    api::{AppState, router},
    auth::StaticTokenAuthenticator,
    capture::{
        CaptureDispatch, CaptureJob, CaptureJobCheckpoint, CaptureJobLease,
        CaptureJobLeaseIdentity, CaptureJobStatus, ProjectionPublication,
    },
    config::{Config, ZoomConfigValues},
    license::{License, LicenseClaims},
    schedule::{
        CalendarEventInput, CapturePolicy, ScheduleDecision, ScheduledCapture,
        ScheduledCaptureStatus, decide_schedule, scheduled_job_id,
    },
    serve,
    store::{ControlPlaneStore, StoreError},
    zoom::{
        ZoomDispatchError, ZoomDispatchOutcome, ZoomStopReason, ZoomWebhookDispatcher,
        ZoomWebhookService,
    },
};
use anarlog_enterprise_google_meet_worker::{ControlPlaneEventSink, ControlPlaneEventSinkConfig};
use anarlog_enterprise_zoom_rtms_worker::{ZoomRtmsStarted, ZoomRtmsTerminal};
use anlg_meeting_capture::{BotState, CaptureEvent, CaptureProviderKind};
use anlg_session_ingest::{AcknowledgeRequest, DeliveryPage, SessionRead};
use async_trait::async_trait;
use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use hmac::{Hmac, Mac};
use serde_json::Value;
use sha2_legacy::Sha256;
use std::sync::Mutex;
use tokio::{net::TcpListener, sync::oneshot, time::Duration};
use tower::ServiceExt;

const TOKEN_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const TOKEN_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

struct MemoryStore {
    ready: bool,
    policies: Mutex<HashMap<String, CapturePolicy>>,
    scheduled: Mutex<HashMap<(String, String), ScheduledCapture>>,
}

#[derive(Default)]
struct RecordingZoomDispatcher {
    starts: Mutex<Vec<(String, ZoomRtmsStarted)>>,
    stops: Mutex<Vec<(ZoomRtmsTerminal, ZoomStopReason)>>,
}

#[async_trait]
impl ZoomWebhookDispatcher for RecordingZoomDispatcher {
    async fn start(
        &self,
        workspace_id: &str,
        started: ZoomRtmsStarted,
    ) -> Result<ZoomDispatchOutcome, ZoomDispatchError> {
        let mut starts = self.starts.lock().unwrap();
        let first_delivery = starts.is_empty();
        starts.push((workspace_id.into(), started));
        Ok(ZoomDispatchOutcome {
            job_id: "job-zoom-a".into(),
            started: first_delivery,
        })
    }

    async fn stop(
        &self,
        terminal: &ZoomRtmsTerminal,
        reason: ZoomStopReason,
    ) -> Result<(), ZoomDispatchError> {
        self.stops.lock().unwrap().push((terminal.clone(), reason));
        Ok(())
    }
}

#[async_trait]
impl ControlPlaneStore for MemoryStore {
    async fn readiness(&self) -> Result<(), StoreError> {
        if self.ready {
            Ok(())
        } else {
            Err(StoreError::CorruptEnvelope)
        }
    }

    async fn create_capture_job(&self, job: &CaptureJob) -> Result<CaptureJobStatus, StoreError> {
        Ok(CaptureJobStatus {
            job_id: job.job_id.clone(),
            created: true,
            state: BotState::Queued,
        })
    }

    async fn read_capture_checkpoint(
        &self,
        workspace_id: &str,
        job_id: &str,
    ) -> Result<CaptureJobCheckpoint, StoreError> {
        if workspace_id != "workspace-a" || job_id != "job-a" {
            return Err(StoreError::NotFound);
        }
        Ok(CaptureJobCheckpoint {
            job: capture_job(),
            state: BotState::Capturing,
            next_sequence: 7,
        })
    }

    async fn find_dispatchable_capture_checkpoint(
        &self,
        _workspace_id: &str,
        _provider: CaptureProviderKind,
        _external_ids: &[String],
    ) -> Result<CaptureJobCheckpoint, StoreError> {
        Err(StoreError::NotFound)
    }

    async fn save_capture_dispatch(&self, _dispatch: &CaptureDispatch) -> Result<(), StoreError> {
        Ok(())
    }

    async fn list_capture_dispatches(
        &self,
        _provider: CaptureProviderKind,
    ) -> Result<Vec<CaptureDispatch>, StoreError> {
        Ok(Vec::new())
    }

    async fn find_capture_dispatch(
        &self,
        _provider: CaptureProviderKind,
        _dispatch_id: &str,
    ) -> Result<CaptureDispatch, StoreError> {
        Err(StoreError::NotFound)
    }

    async fn next_capture_transcript_sequence(
        &self,
        _workspace_id: &str,
        _job_id: &str,
    ) -> Result<u64, StoreError> {
        Ok(0)
    }

    async fn claim_capture_job(
        &self,
        _workspace_id: &str,
        _job_id: &str,
        worker_id: &str,
        lease_id: &str,
        lease_duration: std::time::Duration,
    ) -> Result<CaptureJobLease, StoreError> {
        Ok(CaptureJobLease {
            worker_id: worker_id.into(),
            lease_id: lease_id.into(),
            epoch: 1,
            expires_at: chrono::Utc::now() + chrono::Duration::from_std(lease_duration).unwrap(),
        })
    }

    async fn renew_capture_job_lease(
        &self,
        _workspace_id: &str,
        _job_id: &str,
        lease: &CaptureJobLeaseIdentity,
        lease_duration: std::time::Duration,
    ) -> Result<CaptureJobLease, StoreError> {
        Ok(CaptureJobLease {
            worker_id: lease.worker_id.clone(),
            lease_id: lease.lease_id.clone(),
            epoch: lease.epoch,
            expires_at: chrono::Utc::now() + chrono::Duration::from_std(lease_duration).unwrap(),
        })
    }

    async fn append_capture_event(
        &self,
        _workspace_id: &str,
        _job_id: &str,
        _lease: &CaptureJobLeaseIdentity,
        _event: &CaptureEvent,
    ) -> Result<ProjectionPublication, StoreError> {
        Err(StoreError::NotFound)
    }

    async fn list_deliveries(
        &self,
        _workspace_id: &str,
        _consumer_id: &str,
        after: u64,
        _limit: u16,
    ) -> Result<DeliveryPage, StoreError> {
        Ok(DeliveryPage {
            items: vec![],
            next_cursor: after,
            has_more: false,
        })
    }

    async fn acknowledge(
        &self,
        _workspace_id: &str,
        _job_id: &str,
        _request: &AcknowledgeRequest,
    ) -> Result<(), StoreError> {
        Ok(())
    }

    async fn read_session(
        &self,
        _workspace_id: &str,
        _job_id: &str,
    ) -> Result<SessionRead, StoreError> {
        Err(StoreError::NotFound)
    }

    async fn get_capture_policy(&self, workspace_id: &str) -> Result<CapturePolicy, StoreError> {
        Ok(self
            .policies
            .lock()
            .unwrap()
            .get(workspace_id)
            .cloned()
            .unwrap_or_else(|| CapturePolicy::default_off(workspace_id)))
    }

    async fn upsert_capture_policy(
        &self,
        policy: &CapturePolicy,
    ) -> Result<CapturePolicy, StoreError> {
        self.policies
            .lock()
            .unwrap()
            .insert(policy.workspace_id.clone(), policy.clone());
        Ok(policy.clone())
    }

    async fn upsert_calendar_events(
        &self,
        workspace_id: &str,
        events: &[CalendarEventInput],
    ) -> Result<Vec<ScheduledCapture>, StoreError> {
        let policy = self.get_capture_policy(workspace_id).await?;
        let mut scheduled = self.scheduled.lock().unwrap();
        let mut out = Vec::new();
        for event in events {
            let key = (workspace_id.to_string(), event.calendar_event_id.clone());
            if let Some(existing) = scheduled.get(&key) {
                if existing.status == ScheduledCaptureStatus::Dispatched {
                    out.push(existing.clone());
                    continue;
                }
            }
            let decision = decide_schedule(&policy, event);
            let row = match decision {
                ScheduleDecision::Pending => ScheduledCapture {
                    workspace_id: workspace_id.into(),
                    calendar_event_id: event.calendar_event_id.clone(),
                    job_id: Some(scheduled_job_id(&event.calendar_event_id)),
                    title: event.title.clone(),
                    starts_at: event.starts_at,
                    ends_at: event.ends_at,
                    meeting: event.meeting.clone(),
                    provider: event.provider,
                    owner_user_id: event.owner_user_id.clone(),
                    status: ScheduledCaptureStatus::Pending,
                    skip_reason: None,
                    bot_name: policy.bot_name.clone(),
                    disclosure_text: policy.disclosure_text.clone(),
                },
                ScheduleDecision::Skipped(reason) => ScheduledCapture {
                    workspace_id: workspace_id.into(),
                    calendar_event_id: event.calendar_event_id.clone(),
                    job_id: None,
                    title: event.title.clone(),
                    starts_at: event.starts_at,
                    ends_at: event.ends_at,
                    meeting: event.meeting.clone(),
                    provider: event.provider,
                    owner_user_id: event.owner_user_id.clone(),
                    status: ScheduledCaptureStatus::Skipped,
                    skip_reason: Some(reason.into()),
                    bot_name: policy.bot_name.clone(),
                    disclosure_text: policy.disclosure_text.clone(),
                },
                ScheduleDecision::Canceled(reason) => ScheduledCapture {
                    workspace_id: workspace_id.into(),
                    calendar_event_id: event.calendar_event_id.clone(),
                    job_id: None,
                    title: event.title.clone(),
                    starts_at: event.starts_at,
                    ends_at: event.ends_at,
                    meeting: event.meeting.clone(),
                    provider: event.provider,
                    owner_user_id: event.owner_user_id.clone(),
                    status: ScheduledCaptureStatus::Canceled,
                    skip_reason: Some(reason.into()),
                    bot_name: policy.bot_name.clone(),
                    disclosure_text: policy.disclosure_text.clone(),
                },
            };
            scheduled.insert(key, row.clone());
            out.push(row);
        }
        Ok(out)
    }

    async fn list_scheduled_captures(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<ScheduledCapture>, StoreError> {
        Ok(self
            .scheduled
            .lock()
            .unwrap()
            .values()
            .filter(|row| row.workspace_id == workspace_id)
            .cloned()
            .collect())
    }

    async fn cancel_scheduled_capture(
        &self,
        workspace_id: &str,
        calendar_event_id: &str,
    ) -> Result<ScheduledCapture, StoreError> {
        let mut scheduled = self.scheduled.lock().unwrap();
        let row = scheduled
            .get_mut(&(workspace_id.into(), calendar_event_id.into()))
            .ok_or(StoreError::NotFound)?;
        if row.status != ScheduledCaptureStatus::Dispatched {
            row.status = ScheduledCaptureStatus::Canceled;
            row.skip_reason = Some("canceled_by_user".into());
        }
        Ok(row.clone())
    }

    async fn dispatch_due_scheduled_captures(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<CaptureJobStatus>, StoreError> {
        let mut scheduled = self.scheduled.lock().unwrap();
        let mut dispatched = Vec::new();
        for row in scheduled.values_mut() {
            if row.status != ScheduledCaptureStatus::Pending || row.starts_at > now {
                continue;
            }
            let job_id = row
                .job_id
                .clone()
                .unwrap_or_else(|| scheduled_job_id(&row.calendar_event_id));
            row.job_id = Some(job_id.clone());
            row.status = ScheduledCaptureStatus::Dispatched;
            dispatched.push(CaptureJobStatus {
                job_id,
                created: true,
                state: BotState::Queued,
            });
        }
        Ok(dispatched)
    }
}

fn state(ready: bool) -> AppState {
    let authenticator = StaticTokenAuthenticator::new([
        ("workspace-a".into(), TOKEN_A.into()),
        ("workspace-b".into(), TOKEN_B.into()),
    ])
    .unwrap();
    AppState::new(
        Arc::new(MemoryStore {
            ready,
            policies: Mutex::new(HashMap::new()),
            scheduled: Mutex::new(HashMap::new()),
        }),
        Arc::new(authenticator),
    )
}

fn state_with_zoom(dispatcher: Arc<RecordingZoomDispatcher>) -> AppState {
    let config = Config::from_values_with_zoom(
        "postgres://localhost/anarlog".into(),
        None,
        None,
        format!(r#"{{"workspace-a":"{TOKEN_A}","workspace-b":"{TOKEN_B}"}}"#),
        ZoomConfigValues {
            client_id: Some("zoom-client".into()),
            client_secret: Some("zoom-client-secret".into()),
            webhook_secret: Some("zoom-webhook-secret".into()),
            account_workspaces: Some(r#"{"account-a":"workspace-a"}"#.into()),
        },
    )
    .unwrap();
    state(true).with_zoom(Arc::new(ZoomWebhookService::new(
        config.zoom.unwrap(),
        dispatcher,
    )))
}

#[tokio::test]
async fn exposes_database_independent_liveness_and_fail_closed_readiness() {
    let app = router(state(false));

    let live = app
        .clone()
        .oneshot(Request::get("/health/live").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let ready = app
        .oneshot(Request::get("/health/ready").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(live.status(), StatusCode::OK);
    assert_eq!(ready.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn mounts_zoom_webhooks_only_when_complete_configuration_is_present() {
    let body = zoom_started_body("account-a");
    let disabled = router(state(true))
        .oneshot(zoom_request(&body, true))
        .await
        .unwrap();
    assert_eq!(disabled.status(), StatusCode::NOT_FOUND);

    let dispatcher = Arc::new(RecordingZoomDispatcher::default());
    let enabled = router(state_with_zoom(dispatcher.clone()));
    let missing_signature = enabled
        .clone()
        .oneshot(
            Request::post("/webhooks/zoom")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let invalid_signature = enabled
        .clone()
        .oneshot(zoom_request(&body, false))
        .await
        .unwrap();
    let accepted = enabled
        .clone()
        .oneshot(zoom_request(&body, true))
        .await
        .unwrap();
    let retried = enabled.oneshot(zoom_request(&body, true)).await.unwrap();

    assert_eq!(missing_signature.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(invalid_signature.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(accepted.status(), StatusCode::OK);
    assert_eq!(retried.status(), StatusCode::OK);
    assert_eq!(
        response_json(accepted).await,
        serde_json::json!({
            "status": "started",
            "jobId": "job-zoom-a"
        })
    );
    assert_eq!(
        response_json(retried).await,
        serde_json::json!({
            "status": "already_running",
            "jobId": "job-zoom-a"
        })
    );
    let starts = dispatcher.starts.lock().unwrap();
    assert_eq!(starts.len(), 2);
    assert_eq!(starts[0].0, "workspace-a");
    assert_eq!(starts[0].1.account_id, "account-a");
    assert_eq!(starts[0].1.meeting_id, "123456789");
}

#[tokio::test]
async fn validates_zoom_challenges_and_ignores_unregistered_accounts() {
    let dispatcher = Arc::new(RecordingZoomDispatcher::default());
    let app = router(state_with_zoom(dispatcher.clone()));
    let validation_body = serde_json::json!({
        "event": "endpoint.url_validation",
        "event_ts": 1,
        "payload": { "plainToken": "plain-token" }
    })
    .to_string();
    let validation = app
        .clone()
        .oneshot(zoom_request(&validation_body, true))
        .await
        .unwrap();
    assert_eq!(validation.status(), StatusCode::OK);
    let validation = response_json(validation).await;
    assert_eq!(validation["plainToken"], "plain-token");
    assert_eq!(validation["encryptedToken"].as_str().unwrap().len(), 64);

    let ignored = app
        .oneshot(zoom_request(&zoom_started_body("account-b"), true))
        .await
        .unwrap();
    assert_eq!(ignored.status(), StatusCode::NO_CONTENT);
    assert!(dispatcher.starts.lock().unwrap().is_empty());
}

#[tokio::test]
async fn hands_signed_zoom_terminal_events_to_the_running_worker_registry() {
    let dispatcher = Arc::new(RecordingZoomDispatcher::default());
    let app = router(state_with_zoom(dispatcher.clone()));
    let body = serde_json::json!({
        "event": "meeting.rtms_interrupted",
        "event_ts": 1,
        "payload": {
            "meeting_uuid": "meeting-uuid",
            "rtms_stream_id": "stream-id"
        }
    })
    .to_string();

    let response = app.oneshot(zoom_request(&body, true)).await.unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let stops = dispatcher.stops.lock().unwrap();
    assert_eq!(stops.len(), 1);
    assert_eq!(stops[0].0.stream_id, "stream-id");
    assert_eq!(stops[0].1, ZoomStopReason::Interrupted);
}

#[tokio::test]
async fn requires_workspace_scoped_credentials() {
    let app = router(state(true));
    let path = "/v1/workspaces/workspace-a/session-envelopes?consumerId=device-a&after=0";

    let missing = app
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let wrong_workspace = app
        .clone()
        .oneshot(authorized_request(path, TOKEN_B))
        .await
        .unwrap();
    let authorized = app
        .oneshot(authorized_request(path, TOKEN_A))
        .await
        .unwrap();

    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(missing.headers()[header::WWW_AUTHENTICATE], "Bearer");
    assert_eq!(wrong_workspace.status(), StatusCode::FORBIDDEN);
    assert_eq!(authorized.status(), StatusCode::OK);
    assert_eq!(
        response_json(authorized).await,
        serde_json::json!({
            "items": [],
            "nextCursor": 0,
            "hasMore": false,
        })
    );
}

#[tokio::test]
async fn authenticates_capture_job_creation_before_writing() {
    let app = router(state(true));
    let path = "/v1/workspaces/workspace-a/capture-jobs/job-a";
    let body = serde_json::json!({
        "botId": "bot-a",
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
    });

    let missing = app
        .clone()
        .oneshot(json_request(Method::POST, path, None, &body))
        .await
        .unwrap();
    let wrong_workspace = app
        .clone()
        .oneshot(json_request(Method::POST, path, Some(TOKEN_B), &body))
        .await
        .unwrap();
    let created = app
        .oneshot(json_request(Method::POST, path, Some(TOKEN_A), &body))
        .await
        .unwrap();

    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(wrong_workspace.status(), StatusCode::FORBIDDEN);
    assert_eq!(created.status(), StatusCode::CREATED);
    assert_eq!(
        response_json(created).await,
        serde_json::json!({
            "jobId": "job-a",
            "created": true,
            "state": "queued"
        })
    );
}

#[tokio::test]
async fn reads_only_the_authorized_workspace_capture_checkpoint() {
    let app = router(state(true));
    let path = "/v1/workspaces/workspace-a/capture-jobs/job-a";

    let missing = app
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let wrong_workspace = app
        .clone()
        .oneshot(authorized_request(path, TOKEN_B))
        .await
        .unwrap();
    let checkpoint = app
        .oneshot(authorized_request(path, TOKEN_A))
        .await
        .unwrap();

    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(wrong_workspace.status(), StatusCode::FORBIDDEN);
    assert_eq!(checkpoint.status(), StatusCode::OK);
    assert_eq!(
        response_json(checkpoint).await,
        serde_json::json!({
            "job": {
                "workspaceId": "workspace-a",
                "jobId": "job-a",
                "botId": "bot-a",
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
    );
}

#[tokio::test]
async fn claims_and_renews_only_an_authorized_worker_lease() {
    let app = router(state(true));
    let claim_path = "/v1/workspaces/workspace-a/capture-jobs/job-a/claim";
    let claim_body = serde_json::json!({
        "workerId": "worker-a",
        "leaseId": "lease-a"
    });

    let wrong_workspace = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            claim_path,
            Some(TOKEN_B),
            &claim_body,
        ))
        .await
        .unwrap();
    let claimed = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            claim_path,
            Some(TOKEN_A),
            &claim_body,
        ))
        .await
        .unwrap();

    assert_eq!(wrong_workspace.status(), StatusCode::FORBIDDEN);
    assert_eq!(claimed.status(), StatusCode::OK);
    let claimed = response_json(claimed).await;
    assert_eq!(claimed["workerId"], "worker-a");
    assert_eq!(claimed["leaseId"], "lease-a");
    assert_eq!(claimed["epoch"], 1);
    assert!(claimed["expiresAt"].is_string());

    let renewed = app
        .oneshot(json_request(
            Method::POST,
            "/v1/workspaces/workspace-a/capture-jobs/job-a/lease",
            Some(TOKEN_A),
            &serde_json::json!({
                "lease": {
                    "workerId": "worker-a",
                    "leaseId": "lease-a",
                    "epoch": 1
                }
            }),
        ))
        .await
        .unwrap();
    assert_eq!(renewed.status(), StatusCode::OK);
    assert_eq!(response_json(renewed).await["epoch"], 1);
}

#[tokio::test]
async fn boots_on_a_real_listener_and_shuts_down_gracefully() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server = tokio::spawn(serve(listener, state(true), async move {
        shutdown_rx.await.ok();
    }));

    let response = reqwest::get(format!("http://{address}/health/live"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    shutdown_tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .expect("server did not stop")
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn worker_reads_the_checkpoint_and_manages_its_lease_over_real_http() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server = tokio::spawn(serve(listener, state(true), async move {
        shutdown_rx.await.ok();
    }));
    let sink = ControlPlaneEventSink::new(ControlPlaneEventSinkConfig::new(
        reqwest::Url::parse(&format!("http://{address}")).unwrap(),
        "workspace-a",
        "job-a",
        TOKEN_A,
    ))
    .unwrap();

    let checkpoint = sink.read_checkpoint().await.unwrap();
    let lease = sink.claim("worker-a", "lease-a").await.unwrap();
    let renewed = sink.renew_lease().await.unwrap();

    assert_eq!(checkpoint.job_id, "job-a");
    assert_eq!(checkpoint.bot_id, "bot-a");
    assert_eq!(checkpoint.state, BotState::Capturing);
    assert_eq!(checkpoint.next_sequence, 7);
    assert_eq!(lease.worker_id, "worker-a");
    assert_eq!(lease.lease_id, "lease-a");
    assert_eq!(lease.epoch, 1);
    assert_eq!(renewed.epoch, lease.epoch);
    shutdown_tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .expect("server did not stop")
        .unwrap()
        .unwrap();
}

fn authorized_request(path: &str, token: &str) -> Request<Body> {
    Request::get(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

fn json_request(method: Method, path: &str, token: Option<&str>, body: &Value) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    request.body(Body::from(body.to_string())).unwrap()
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

fn zoom_started_body(account_id: &str) -> String {
    serde_json::json!({
        "event": "meeting.rtms_started",
        "event_ts": 1,
        "payload": {
            "account_id": account_id,
            "meeting_uuid": "meeting-uuid",
            "meeting_id": 123456789,
            "rtms_stream_id": "stream-id",
            "server_urls": "wss://rtms.zoom.us/signaling"
        }
    })
    .to_string()
}

fn zoom_request(body: &str, valid_signature: bool) -> Request<Body> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();
    let mut mac = Hmac::<Sha256>::new_from_slice(if valid_signature {
        b"zoom-webhook-secret"
    } else {
        b"wrong-webhook-secret"
    })
    .unwrap();
    mac.update(format!("v0:{timestamp}:").as_bytes());
    mac.update(body.as_bytes());
    let signature = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Request::post("/webhooks/zoom")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-zm-request-timestamp", timestamp)
        .header("x-zm-signature", format!("v0={signature}"))
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn capture_job() -> CaptureJob {
    CaptureJob {
        workspace_id: "workspace-a".into(),
        job_id: "job-a".into(),
        bot_id: "bot-a".into(),
        owner_user_id: "owner-a".into(),
        requesting_actor_id: "actor-a".into(),
        session_id: "session-a".into(),
        session_title: "Architecture review".into(),
        provider: anlg_meeting_capture::CaptureProviderKind::Anarlog,
        meeting: anlg_meeting_capture::MeetingReference {
            platform: anlg_meeting_capture::MeetingPlatform::GoogleMeet,
            url: "https://meet.google.com/abc-defg-hij".into(),
            external_id: None,
            calendar_event_id: None,
        },
        created_at: chrono::DateTime::parse_from_rfc3339("2026-08-17T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    }
}

#[tokio::test]
async fn schedules_exactly_one_capture_job_and_allows_cancel() {
    let app = router(state(true));
    let policy = serde_json::json!({
        "workspaceId": "workspace-a",
        "captureEnabled": true,
        "allowedProviders": ["anarlog"],
        "botName": "Anarlog Notetaker",
        "skipIfDesktopCapture": true
    });
    let saved = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/v1/workspaces/workspace-a/capture-policy",
            Some(TOKEN_A),
            &policy,
        ))
        .await
        .unwrap();
    assert_eq!(saved.status(), StatusCode::OK);

    let events = serde_json::json!([{
        "calendarEventId": "evt-1",
        "title": "Standup",
        "startsAt": "2026-08-21T15:00:00Z",
        "meeting": {
            "platform": "google_meet",
            "url": "https://meet.google.com/aaa-bbbb-ccc"
        },
        "provider": "anarlog",
        "ownerUserId": "owner-a"
    }]);
    let first = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/v1/workspaces/workspace-a/calendar-events",
            Some(TOKEN_A),
            &events,
        ))
        .await
        .unwrap();
    let second = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/v1/workspaces/workspace-a/calendar-events",
            Some(TOKEN_A),
            &events,
        ))
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(second.status(), StatusCode::OK);
    let listed = response_json(first).await;
    assert_eq!(listed.as_array().map(Vec::len), Some(1));
    assert_eq!(listed[0]["jobId"], "cal-evt-1");
    assert_eq!(listed[0]["status"], "pending");
    assert_eq!(response_json(second).await[0]["jobId"], "cal-evt-1");

    let canceled = app
        .oneshot(
            Request::delete("/v1/workspaces/workspace-a/scheduled-captures/evt-1")
                .header(header::AUTHORIZATION, format!("Bearer {TOKEN_A}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(canceled.status(), StatusCode::OK);
    assert_eq!(response_json(canceled).await["status"], "canceled");
}

#[tokio::test]
async fn offline_license_forbids_unlicensed_workspaces() {
    let claims = LicenseClaims {
        customer_id: "acme".into(),
        workspace_ids: vec!["workspace-b".into()],
        not_before: chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
        expires_at: None,
        features: Default::default(),
    };
    let key = "0123456789abcdef0123456789abcdef";
    let token = License::issue(&claims, key).unwrap();
    let license = License::parse(
        &token,
        key,
        chrono::DateTime::parse_from_rfc3339("2026-08-21T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    )
    .unwrap();
    let app = router(state(true).with_license(license));
    let denied = app
        .oneshot(
            Request::get("/v1/workspaces/workspace-a/scheduled-captures")
                .header(header::AUTHORIZATION, format!("Bearer {TOKEN_A}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
}
