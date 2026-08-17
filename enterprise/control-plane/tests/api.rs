use std::sync::Arc;

use anarlog_enterprise_control_plane::{
    api::{AppState, router},
    auth::StaticTokenAuthenticator,
    capture::{
        CaptureJob, CaptureJobCheckpoint, CaptureJobLease, CaptureJobLeaseIdentity,
        CaptureJobStatus, ProjectionPublication,
    },
    serve,
    store::{ControlPlaneStore, StoreError},
};
use anarlog_enterprise_google_meet_worker::{ControlPlaneEventSink, ControlPlaneEventSinkConfig};
use anlg_meeting_capture::{BotState, CaptureEvent};
use anlg_session_ingest::{AcknowledgeRequest, DeliveryPage, SessionRead};
use async_trait::async_trait;
use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use serde_json::Value;
use tokio::{net::TcpListener, sync::oneshot, time::Duration};
use tower::ServiceExt;

const TOKEN_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const TOKEN_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

struct MemoryStore {
    ready: bool,
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
}

fn state(ready: bool) -> AppState {
    let authenticator = StaticTokenAuthenticator::new([
        ("workspace-a".into(), TOKEN_A.into()),
        ("workspace-b".into(), TOKEN_B.into()),
    ])
    .unwrap();
    AppState::new(Arc::new(MemoryStore { ready }), Arc::new(authenticator))
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
