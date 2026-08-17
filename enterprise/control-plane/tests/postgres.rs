use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anarlog_enterprise_control_plane::{api::router, config::Config, configured_state, serve};
use anarlog_enterprise_google_meet_worker::{
    CaptureJobRuntime, CaptureJobSupervisor, CaptureJobSupervisorConfig,
    CaptureJobSupervisorOutcome, ControlPlaneEventSink, ControlPlaneEventSinkConfig,
    WorkerCheckpoint, WorkerLifecycle,
};
use anlg_meeting_capture::{
    BotState, CaptureEvent, CaptureEventPayload, Participant, RecordingChunk, Speaker,
    TerminalReason, TerminalReasonKind, TranscriptSegment,
};
use async_trait::async_trait;
use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use serde_json::Value;
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot, watch},
};
use tower::ServiceExt;

const TEST_DATABASE_URL_ENV: &str = "ANARLOG_ENTERPRISE_TEST_DATABASE_URL";
const TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[derive(Debug, thiserror::Error)]
#[error("test capture runtime failed")]
struct TestRuntimeError;

struct CompletingRuntime {
    cleaned: Arc<AtomicBool>,
}

#[async_trait]
impl CaptureJobRuntime for CompletingRuntime {
    type Error = TestRuntimeError;

    async fn run(
        &mut self,
        checkpoint: &WorkerCheckpoint,
        lifecycle: &mut WorkerLifecycle,
        events: mpsc::Sender<CaptureEvent>,
    ) -> Result<(), Self::Error> {
        assert!(checkpoint.job_id.starts_with("job-supervisor-"));
        events
            .send(lifecycle.launch_started(chrono::Utc::now()).unwrap())
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;
        events
            .send(
                lifecycle
                    .transition(BotState::Joined, None, chrono::Utc::now())
                    .unwrap(),
            )
            .await
            .unwrap();
        events
            .send(
                lifecycle
                    .transition(
                        BotState::Completed,
                        Some(TerminalReason {
                            kind: TerminalReasonKind::MeetingEnded,
                            message: None,
                            retryable: false,
                        }),
                        chrono::Utc::now(),
                    )
                    .unwrap(),
            )
            .await
            .unwrap();
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.cleaned.store(true, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn migrates_postgres_and_enforces_workspace_delivery() {
    let Ok(database_url) = std::env::var(TEST_DATABASE_URL_ENV) else {
        return;
    };
    let suffix = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let workspace_id = format!("workspace-{suffix}");
    let job_id = format!("job-{suffix}");
    let config = Config::from_values(
        database_url.clone(),
        Some("127.0.0.1:0".into()),
        Some("2".into()),
        format!(r#"{{"{workspace_id}":"{TOKEN}"}}"#),
    )
    .unwrap();
    let state = configured_state(&config).await.unwrap();
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let envelope = serde_json::json!({
        "schema_version": 1,
        "source_id": job_id,
        "revision": 1,
        "finalized": true,
        "workspace_id": workspace_id,
        "owner_user_id": "owner-a",
        "session": {
            "id": format!("session-{suffix}"),
            "title": "Smoke test",
            "status": "completed",
            "created_at": "2026-08-17T00:00:00Z",
            "updated_at": "2026-08-17T00:00:00Z"
        }
    });
    sqlx::query(
        r#"
        INSERT INTO session_envelopes (
            workspace_id,
            job_id,
            revision,
            finalized,
            content_hash,
            envelope
        ) VALUES ($1, $2, 1, TRUE, $3, $4)
        "#,
    )
    .bind(&workspace_id)
    .bind(&job_id)
    .bind("0000000000000000000000000000000000000000000000000000000000000000")
    .bind(envelope)
    .execute(&pool)
    .await
    .unwrap();

    let app = router(state);
    let list_path =
        format!("/v1/workspaces/{workspace_id}/session-envelopes?consumerId=device-a&after=0");
    let listed = app
        .clone()
        .oneshot(authorized_request(Method::GET, &list_path, Body::empty()))
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let listed = response_json(listed).await;
    assert_eq!(listed["items"][0]["jobId"], job_id);
    assert_eq!(listed["items"][0]["acknowledged"], false);

    let acknowledge_path = format!("/v1/workspaces/{workspace_id}/session-envelopes/{job_id}/ack");
    let acknowledged = app
        .clone()
        .oneshot(authorized_request(
            Method::POST,
            &acknowledge_path,
            Body::from(
                serde_json::json!({
                    "consumerId": "device-a",
                    "revision": 1,
                    "contentHash": "0000000000000000000000000000000000000000000000000000000000000000"
                })
                .to_string(),
            ),
        ))
        .await
        .unwrap();
    assert_eq!(acknowledged.status(), StatusCode::OK);

    let wrong_workspace = app
        .oneshot(authorized_request(
            Method::GET,
            "/v1/workspaces/other-workspace/session-envelopes?consumerId=device-a&after=0",
            Body::empty(),
        ))
        .await
        .unwrap();
    assert_eq!(wrong_workspace.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn concurrent_capture_claims_have_one_fenced_winner() {
    let Ok(database_url) = std::env::var(TEST_DATABASE_URL_ENV) else {
        return;
    };
    let suffix = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let workspace_id = format!("workspace-claim-{suffix}");
    let job_id = format!("job-claim-{suffix}");
    let config = Config::from_values(
        database_url,
        Some("127.0.0.1:0".into()),
        Some("2".into()),
        format!(r#"{{"{workspace_id}":"{TOKEN}"}}"#),
    )
    .unwrap();
    let app = router(configured_state(&config).await.unwrap());
    let create_path = format!("/v1/workspaces/{workspace_id}/capture-jobs/{job_id}");
    let created = app
        .clone()
        .oneshot(authorized_request(
            Method::POST,
            &create_path,
            Body::from(
                serde_json::json!({
                    "botId": format!("bot-claim-{suffix}"),
                    "ownerUserId": "owner-a",
                    "requestingActorId": "actor-a",
                    "sessionId": format!("session-claim-{suffix}"),
                    "sessionTitle": "Concurrent claim",
                    "provider": "anarlog",
                    "meeting": {
                        "platform": "google_meet",
                        "url": "https://meet.google.com/abc-defg-hij"
                    },
                    "createdAt": "2026-08-17T00:00:00Z"
                })
                .to_string(),
            ),
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);

    let claim_path = format!("{create_path}/claim");
    let first = app.clone().oneshot(authorized_request(
        Method::POST,
        &claim_path,
        Body::from(r#"{"workerId":"worker-a","leaseId":"lease-a"}"#),
    ));
    let second = app.oneshot(authorized_request(
        Method::POST,
        &claim_path,
        Body::from(r#"{"workerId":"worker-b","leaseId":"lease-b"}"#),
    ));
    let (first, second) = tokio::join!(first, second);
    let first = first.unwrap();
    let second = second.unwrap();

    assert_eq!(
        [first.status(), second.status()]
            .into_iter()
            .filter(|status| *status == StatusCode::OK)
            .count(),
        1
    );
    assert_eq!(
        [first.status(), second.status()]
            .into_iter()
            .filter(|status| *status == StatusCode::CONFLICT)
            .count(),
        1
    );
    let winner = if first.status() == StatusCode::OK {
        response_json(first).await
    } else {
        response_json(second).await
    };
    assert_eq!(winner["epoch"], 1);
}

#[tokio::test]
async fn supervises_a_capture_job_over_real_http_and_postgres() {
    let Ok(database_url) = std::env::var(TEST_DATABASE_URL_ENV) else {
        return;
    };
    let suffix = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let workspace_id = format!("workspace-supervisor-{suffix}");
    let job_id = format!("job-supervisor-{suffix}");
    let config = Config::from_values(
        database_url,
        Some("127.0.0.1:0".into()),
        Some("2".into()),
        format!(r#"{{"{workspace_id}":"{TOKEN}"}}"#),
    )
    .unwrap();
    let state = configured_state(&config).await.unwrap();
    let create_path = format!("/v1/workspaces/{workspace_id}/capture-jobs/{job_id}");
    let created = router(state.clone())
        .oneshot(authorized_request(
            Method::POST,
            &create_path,
            Body::from(
                serde_json::json!({
                    "botId": format!("bot-supervisor-{suffix}"),
                    "ownerUserId": "owner-a",
                    "requestingActorId": "actor-a",
                    "sessionId": format!("session-supervisor-{suffix}"),
                    "sessionTitle": "Supervisor contract",
                    "provider": "anarlog",
                    "meeting": {
                        "platform": "google_meet",
                        "url": "https://meet.google.com/abc-defg-hij"
                    },
                    "createdAt": "2026-08-17T00:00:00Z"
                })
                .to_string(),
            ),
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (server_shutdown_tx, server_shutdown_rx) = oneshot::channel();
    let server = tokio::spawn(serve(listener, state, async move {
        server_shutdown_rx.await.ok();
    }));
    let sink_config = || {
        ControlPlaneEventSinkConfig::new(
            reqwest::Url::parse(&format!("http://{address}")).unwrap(),
            &workspace_id,
            &job_id,
            TOKEN,
        )
    };
    let cleaned = Arc::new(AtomicBool::new(false));
    let supervisor = CaptureJobSupervisor::new(
        ControlPlaneEventSink::new(sink_config()).unwrap(),
        CompletingRuntime {
            cleaned: cleaned.clone(),
        },
        "worker-supervisor-a",
        "lease-supervisor-a",
        CaptureJobSupervisorConfig {
            lease_renew_interval: Duration::from_millis(1),
        },
    )
    .unwrap();
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);

    let outcome = supervisor.run(shutdown_rx).await.unwrap();

    assert_eq!(
        outcome,
        CaptureJobSupervisorOutcome::Terminal(BotState::Completed)
    );
    assert!(cleaned.load(Ordering::SeqCst));
    let checkpoint = ControlPlaneEventSink::new(sink_config())
        .unwrap()
        .read_checkpoint()
        .await
        .unwrap();
    assert_eq!(checkpoint.state, BotState::Completed);
    assert_eq!(checkpoint.next_sequence, 3);
    server_shutdown_tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .expect("server did not stop")
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn persists_projects_and_replays_capture_events_idempotently() {
    let Ok(database_url) = std::env::var(TEST_DATABASE_URL_ENV) else {
        return;
    };
    let suffix = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let workspace_id = format!("workspace-projection-{suffix}");
    let job_id = format!("job-projection-{suffix}");
    let bot_id = format!("bot-projection-{suffix}");
    let config = Config::from_values(
        database_url.clone(),
        Some("127.0.0.1:0".into()),
        Some("2".into()),
        format!(r#"{{"{workspace_id}":"{TOKEN}"}}"#),
    )
    .unwrap();
    let app = router(configured_state(&config).await.unwrap());
    let create_path = format!("/v1/workspaces/{workspace_id}/capture-jobs/{job_id}");
    let created = app
        .clone()
        .oneshot(authorized_request(
            Method::POST,
            &create_path,
            Body::from(
                serde_json::json!({
                    "botId": bot_id,
                    "ownerUserId": "owner-a",
                    "requestingActorId": "actor-a",
                    "sessionId": format!("session-{suffix}"),
                    "sessionTitle": "Deterministic capture",
                    "provider": "anarlog",
                    "meeting": {
                        "platform": "google_meet",
                        "url": "https://meet.google.com/abc-defg-hij",
                        "external_id": "abc-defg-hij",
                        "calendar_event_id": "calendar-event-a"
                    },
                    "createdAt": "2026-08-17T00:00:00Z"
                })
                .to_string(),
            ),
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);

    let queued_checkpoint = app
        .clone()
        .oneshot(authorized_request(Method::GET, &create_path, Body::empty()))
        .await
        .unwrap();
    assert_eq!(queued_checkpoint.status(), StatusCode::OK);
    let queued_checkpoint = response_json(queued_checkpoint).await;
    assert_eq!(queued_checkpoint["state"], "queued");
    assert_eq!(queued_checkpoint["nextSequence"], 0);

    let claim_path = format!("{create_path}/claim");
    let first_lease = app
        .clone()
        .oneshot(authorized_request(
            Method::POST,
            &claim_path,
            Body::from(
                serde_json::json!({
                    "workerId": "worker-a",
                    "leaseId": "lease-a"
                })
                .to_string(),
            ),
        ))
        .await
        .unwrap();
    assert_eq!(first_lease.status(), StatusCode::OK);
    let first_lease = response_json(first_lease).await;
    assert_eq!(first_lease["epoch"], 1);

    let replayed_claim = app
        .clone()
        .oneshot(authorized_request(
            Method::POST,
            &claim_path,
            Body::from(
                serde_json::json!({
                    "workerId": "worker-a",
                    "leaseId": "lease-a"
                })
                .to_string(),
            ),
        ))
        .await
        .unwrap();
    assert_eq!(replayed_claim.status(), StatusCode::OK);
    assert_eq!(response_json(replayed_claim).await["epoch"], 1);

    let competing_claim = app
        .clone()
        .oneshot(authorized_request(
            Method::POST,
            &claim_path,
            Body::from(
                serde_json::json!({
                    "workerId": "worker-b",
                    "leaseId": "lease-b"
                })
                .to_string(),
            ),
        ))
        .await
        .unwrap();
    assert_eq!(competing_claim.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(competing_claim).await["error"]["code"],
        "capture_lease_unavailable"
    );

    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    sqlx::query(
        "UPDATE capture_jobs SET lease_expires_at = now() - INTERVAL '1 second' WHERE workspace_id = $1 AND job_id = $2",
    )
    .bind(&workspace_id)
    .bind(&job_id)
    .execute(&pool)
    .await
    .unwrap();
    let second_lease = app
        .clone()
        .oneshot(authorized_request(
            Method::POST,
            &claim_path,
            Body::from(
                serde_json::json!({
                    "workerId": "worker-b",
                    "leaseId": "lease-b"
                })
                .to_string(),
            ),
        ))
        .await
        .unwrap();
    assert_eq!(second_lease.status(), StatusCode::OK);
    let second_lease = response_json(second_lease).await;
    assert_eq!(second_lease["epoch"], 2);
    let first_lease_identity = serde_json::json!({
        "workerId": first_lease["workerId"],
        "leaseId": first_lease["leaseId"],
        "epoch": first_lease["epoch"]
    });
    let second_lease_identity = serde_json::json!({
        "workerId": second_lease["workerId"],
        "leaseId": second_lease["leaseId"],
        "epoch": second_lease["epoch"]
    });

    let conflicting_job_path =
        format!("/v1/workspaces/{workspace_id}/capture-jobs/other-job-{suffix}");
    let bot_conflict = app
        .clone()
        .oneshot(authorized_request(
            Method::POST,
            &conflicting_job_path,
            Body::from(
                serde_json::json!({
                    "botId": bot_id,
                    "ownerUserId": "owner-a",
                    "requestingActorId": "actor-a",
                    "sessionId": format!("other-session-{suffix}"),
                    "sessionTitle": "Conflicting capture",
                    "provider": "anarlog",
                    "meeting": {
                        "platform": "google_meet",
                        "url": "https://meet.google.com/abc-defg-hij"
                    },
                    "createdAt": "2026-08-17T00:00:00Z"
                })
                .to_string(),
            ),
        ))
        .await
        .unwrap();
    assert_eq!(bot_conflict.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(bot_conflict).await["error"]["code"],
        "capture_bot_conflict"
    );

    let events = capture_events(&bot_id);
    let event_path = format!("{create_path}/events");
    let fenced = app
        .clone()
        .oneshot(authorized_request(
            Method::POST,
            &event_path,
            Body::from(
                serde_json::json!({
                    "lease": first_lease_identity,
                    "event": &events[0]
                })
                .to_string(),
            ),
        ))
        .await
        .unwrap();
    assert_eq!(fenced.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(fenced).await["error"]["code"],
        "capture_lease_lost"
    );
    let mut final_publication = Value::Null;
    for event in &events {
        let response = app
            .clone()
            .oneshot(authorized_request(
                Method::POST,
                &event_path,
                Body::from(
                    serde_json::json!({
                        "lease": second_lease_identity,
                        "event": event
                    })
                    .to_string(),
                ),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        final_publication = response_json(response).await;
    }
    assert_eq!(final_publication["revision"], 8);
    assert_eq!(final_publication["finalized"], true);
    assert_eq!(
        final_publication["envelope"]["transcripts"][0]["words"][0]["speaker"],
        "Ada"
    );
    assert_eq!(
        final_publication["envelope"]["attachments"][0]["object_key"],
        "recordings/job-a/0.webm"
    );

    let retried = app
        .clone()
        .oneshot(authorized_request(
            Method::POST,
            &event_path,
            Body::from(
                serde_json::json!({
                    "lease": first_lease_identity,
                    "event": events.last().unwrap()
                })
                .to_string(),
            ),
        ))
        .await
        .unwrap();
    assert_eq!(retried.status(), StatusCode::OK);
    assert_eq!(response_json(retried).await, final_publication);

    let mut conflicting = events.last().unwrap().clone();
    conflicting.id = "different-event-id".into();
    let conflict = app
        .clone()
        .oneshot(authorized_request(
            Method::POST,
            &event_path,
            Body::from(
                serde_json::json!({
                    "lease": second_lease_identity,
                    "event": conflicting
                })
                .to_string(),
            ),
        ))
        .await
        .unwrap();
    assert_eq!(conflict.status(), StatusCode::CONFLICT);

    let restarted = router(configured_state(&config).await.unwrap());
    let resumed_checkpoint = restarted
        .clone()
        .oneshot(authorized_request(Method::GET, &create_path, Body::empty()))
        .await
        .unwrap();
    assert_eq!(resumed_checkpoint.status(), StatusCode::OK);
    let resumed_checkpoint = response_json(resumed_checkpoint).await;
    assert_eq!(resumed_checkpoint["state"], "completed");
    assert_eq!(resumed_checkpoint["nextSequence"], 8);

    let session_path = format!("/v1/workspaces/{workspace_id}/sessions/{job_id}");
    let session = restarted
        .oneshot(authorized_request(
            Method::GET,
            &session_path,
            Body::empty(),
        ))
        .await
        .unwrap();
    assert_eq!(session.status(), StatusCode::OK);
    let session = response_json(session).await;
    assert_eq!(session["revision"], final_publication["revision"]);
    assert_eq!(session["contentHash"], final_publication["contentHash"]);
    assert_eq!(session["envelope"], final_publication["envelope"]);

    let event_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM capture_events WHERE workspace_id = $1 AND job_id = $2",
    )
    .bind(&workspace_id)
    .bind(&job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let envelope_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM session_envelopes WHERE workspace_id = $1 AND job_id = $2",
    )
    .bind(&workspace_id)
    .bind(&job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event_count, 8);
    assert_eq!(envelope_count, 8);
}

fn authorized_request(method: Method, path: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(body)
        .unwrap()
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

fn capture_events(bot_id: &str) -> Vec<CaptureEvent> {
    vec![
        lifecycle(bot_id, 0, BotState::Queued, BotState::Launching, None),
        lifecycle(bot_id, 1, BotState::Launching, BotState::Joined, None),
        lifecycle(bot_id, 2, BotState::Joined, BotState::Capturing, None),
        capture_event(
            bot_id,
            3,
            CaptureEventPayload::ParticipantUpserted(Participant {
                id: "participant-a".into(),
                display_name: Some("Ada".into()),
                email: Some("ada@example.com".into()),
            }),
        ),
        capture_event(
            bot_id,
            4,
            CaptureEventPayload::SpeakerUpserted(Speaker {
                id: "speaker-a".into(),
                display_name: Some("Ada".into()),
                participant_id: Some("participant-a".into()),
            }),
        ),
        capture_event(
            bot_id,
            5,
            CaptureEventPayload::Transcript(TranscriptSegment {
                id: "segment-a".into(),
                sequence: 0,
                start_ms: 100,
                end_ms: Some(450),
                text: "Ship the deterministic projector.".into(),
                speaker: Some(Speaker {
                    id: "speaker-a".into(),
                    display_name: None,
                    participant_id: Some("participant-a".into()),
                }),
                is_final: true,
            }),
        ),
        capture_event(
            bot_id,
            6,
            CaptureEventPayload::RecordingChunkReady(RecordingChunk {
                id: "chunk-a".into(),
                sequence: 0,
                start_ms: 0,
                duration_ms: 1_000,
                content_type: "audio/webm".into(),
                uri: "recordings/job-a/0.webm".into(),
                size_bytes: Some(4_096),
                sha256: Some("a".repeat(64)),
            }),
        ),
        lifecycle(
            bot_id,
            7,
            BotState::Capturing,
            BotState::Completed,
            Some(TerminalReason {
                kind: TerminalReasonKind::MeetingEnded,
                message: None,
                retryable: false,
            }),
        ),
    ]
}

fn lifecycle(
    bot_id: &str,
    sequence: u64,
    from: BotState,
    to: BotState,
    reason: Option<TerminalReason>,
) -> CaptureEvent {
    capture_event(
        bot_id,
        sequence,
        CaptureEventPayload::Lifecycle(from.transition_to(to, reason).unwrap()),
    )
}

fn capture_event(bot_id: &str, sequence: u64, payload: CaptureEventPayload) -> CaptureEvent {
    CaptureEvent {
        id: format!("event-{sequence}"),
        bot_id: bot_id.into(),
        sequence,
        occurred_at: chrono::DateTime::parse_from_rfc3339("2026-08-17T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc)
            + chrono::TimeDelta::seconds(sequence as i64),
        payload,
        metadata: Default::default(),
    }
}
