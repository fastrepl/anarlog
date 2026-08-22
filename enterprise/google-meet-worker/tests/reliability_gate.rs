use std::time::{Duration, Instant};

use anarlog_enterprise_google_meet_worker::{
    AdmissionSnapshot, AudioFrame, AudioFrameSink, AudioFrameSinkOutput, ChunkedRecordingConfig,
    ChunkedRecordingSink, RecordingChunkStore, RuntimeSnapshot, StoredRecordingObject,
    WorkerLifecycle,
};
use anlg_meeting_capture::{BotState, CaptureEventPayload, TerminalReasonKind};
use chrono::{DateTime, Utc};

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-08-17T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

fn join_and_capture(lifecycle: &mut WorkerLifecycle) {
    lifecycle.launch_started(now()).unwrap();
    lifecycle
        .observe_admission(
            &AdmissionSnapshot {
                participant_tile_labels: vec!["Ada Lovelace".into()],
                self_name_nodes: 1,
                ..Default::default()
            },
            Instant::now(),
            now(),
        )
        .unwrap();
    lifecycle.capture_started(now()).unwrap();
}

fn assert_terminal(lifecycle: &WorkerLifecycle) {
    assert!(
        lifecycle.state().is_terminal(),
        "expected terminal state, got {:?}",
        lifecycle.state()
    );
}

#[test]
fn reliability_gate_covers_required_terminal_reasons() {
    let started = Instant::now();
    type Case<'a> = (
        &'a str,
        Box<dyn Fn(&mut WorkerLifecycle) -> TerminalReasonKind + 'a>,
    );
    let cases: [Case<'_>; 9] = [
        (
            "admitted-kicked",
            Box::new(|lifecycle| {
                join_and_capture(lifecycle);
                let event = lifecycle
                    .observe_runtime(
                        &RuntimeSnapshot {
                            removal_indicator: Some("you were removed".into()),
                            ..Default::default()
                        },
                        Instant::now(),
                        now(),
                    )
                    .unwrap()
                    .unwrap();
                reason_kind(event)
            }),
        ),
        (
            "host-ended",
            Box::new(|lifecycle| {
                join_and_capture(lifecycle);
                let event = lifecycle
                    .observe_runtime(
                        &RuntimeSnapshot {
                            meeting_ended_indicator: Some("the meeting has ended".into()),
                            ..Default::default()
                        },
                        Instant::now(),
                        now(),
                    )
                    .unwrap()
                    .unwrap();
                reason_kind(event)
            }),
        ),
        (
            "denied",
            Box::new(|lifecycle| {
                lifecycle.launch_started(now()).unwrap();
                let event = lifecycle
                    .observe_admission(
                        &AdmissionSnapshot {
                            explicit_denial_indicator: Some("denied your request".into()),
                            ..Default::default()
                        },
                        Instant::now(),
                        now(),
                    )
                    .unwrap()
                    .unwrap();
                reason_kind(event)
            }),
        ),
        (
            "lobby-timeout",
            Box::new(|lifecycle| {
                lifecycle.launch_started(now()).unwrap();
                reason_kind(lifecycle.admission_timed_out(now()).unwrap())
            }),
        ),
        (
            "worker-crash",
            Box::new(|lifecycle| {
                join_and_capture(lifecycle);
                reason_kind(lifecycle.worker_exited("worker crashed", now()).unwrap())
            }),
        ),
        (
            "stt-outage",
            Box::new(|lifecycle| {
                join_and_capture(lifecycle);
                reason_kind(
                    lifecycle
                        .stt_unavailable("speech-to-text endpoint returned 503", now())
                        .unwrap(),
                )
            }),
        ),
        (
            "network-lost",
            Box::new(|lifecycle| {
                join_and_capture(lifecycle);
                let snapshot = RuntimeSnapshot {
                    connection_problem_indicator: Some("reconnecting".into()),
                    ..Default::default()
                };
                assert!(
                    lifecycle
                        .observe_runtime(&snapshot, started, now())
                        .unwrap()
                        .is_none()
                );
                reason_kind(
                    lifecycle
                        .observe_runtime(&snapshot, started + Duration::from_secs(30), now())
                        .unwrap()
                        .unwrap(),
                )
            }),
        ),
        (
            "nobody-joined",
            Box::new(|lifecycle| {
                let started = Instant::now();
                let mut local = WorkerLifecycle::with_empty_meeting_grace(
                    "bot-empty",
                    Duration::from_secs(1),
                    Duration::from_secs(30),
                );
                local.launch_started(now()).unwrap();
                local
                    .observe_admission(
                        &AdmissionSnapshot {
                            self_name_nodes: 1,
                            ..Default::default()
                        },
                        started,
                        now(),
                    )
                    .unwrap();
                local.capture_started(now()).unwrap();
                let snapshot = RuntimeSnapshot {
                    self_name_nodes: 1,
                    visible_meeting_controls: 1,
                    ..Default::default()
                };
                assert!(
                    local
                        .observe_runtime(&snapshot, started, now())
                        .unwrap()
                        .is_none()
                );
                let kind = reason_kind(
                    local
                        .observe_runtime(&snapshot, started + Duration::from_secs(1), now())
                        .unwrap()
                        .unwrap(),
                );
                *lifecycle = local;
                kind
            }),
        ),
        (
            "everyone-left",
            Box::new(|lifecycle| {
                let started = Instant::now();
                let mut local = WorkerLifecycle::with_empty_meeting_grace(
                    "bot-empty-after",
                    Duration::from_secs(30),
                    Duration::from_secs(1),
                );
                join_and_capture(&mut local);
                local
                    .observe_runtime(
                        &RuntimeSnapshot {
                            participant_tile_labels: vec!["Ada Lovelace".into()],
                            self_name_nodes: 1,
                            visible_meeting_controls: 2,
                            ..Default::default()
                        },
                        started,
                        now(),
                    )
                    .unwrap();
                let empty = RuntimeSnapshot {
                    self_name_nodes: 1,
                    visible_meeting_controls: 1,
                    ..Default::default()
                };
                assert!(
                    local
                        .observe_runtime(&empty, started, now())
                        .unwrap()
                        .is_none()
                );
                let kind = reason_kind(
                    local
                        .observe_runtime(&empty, started + Duration::from_secs(1), now())
                        .unwrap()
                        .unwrap(),
                );
                *lifecycle = local;
                kind
            }),
        ),
    ];

    let expected = [
        TerminalReasonKind::RemovedFromMeeting,
        TerminalReasonKind::MeetingEnded,
        TerminalReasonKind::AdmissionDenied,
        TerminalReasonKind::AdmissionTimeout,
        TerminalReasonKind::WorkerExited,
        TerminalReasonKind::ProviderError,
        TerminalReasonKind::NetworkLost,
        TerminalReasonKind::NoOneJoined,
        TerminalReasonKind::EveryoneLeft,
    ];

    for ((name, run), expected) in cases.into_iter().zip(expected) {
        let mut lifecycle = WorkerLifecycle::new(name);
        let kind = run(&mut lifecycle);
        assert_eq!(kind, expected, "{name}");
        assert!(lifecycle.state().is_terminal(), "{name}");
    }
}

fn reason_kind(event: anlg_meeting_capture::CaptureEvent) -> TerminalReasonKind {
    let CaptureEventPayload::Lifecycle(transition) = event.payload else {
        panic!("expected lifecycle event");
    };
    transition.reason.unwrap().kind
}

#[test]
fn concurrent_lifecycles_do_not_share_sequence_or_orphan_state() {
    let mut first = WorkerLifecycle::new("bot-a");
    let mut second = WorkerLifecycle::new("bot-b");
    join_and_capture(&mut first);
    join_and_capture(&mut second);

    first.worker_exited("first crashed", now()).unwrap();
    second
        .observe_runtime(
            &RuntimeSnapshot {
                meeting_ended_indicator: Some("meeting ended".into()),
                ..Default::default()
            },
            Instant::now(),
            now(),
        )
        .unwrap();

    assert_eq!(first.state(), BotState::Failed);
    assert_eq!(second.state(), BotState::Completed);
    assert_terminal(&first);
    assert_terminal(&second);
}

#[test]
fn overlapping_and_unresolved_speakers_stay_non_terminal() {
    let mut lifecycle = WorkerLifecycle::new("bot-speakers");
    join_and_capture(&mut lifecycle);
    assert!(
        lifecycle
            .observe_runtime(
                &RuntimeSnapshot {
                    participant_tile_labels: vec!["Ada Lovelace".into(), "Grace Hopper".into()],
                    self_name_nodes: 1,
                    visible_meeting_controls: 2,
                    ..Default::default()
                },
                Instant::now(),
                now(),
            )
            .unwrap()
            .is_none()
    );
    assert!(
        lifecycle
            .observe_runtime(
                &RuntimeSnapshot {
                    participant_tile_labels: vec!["Ada L.".into()],
                    self_name_nodes: 1,
                    visible_meeting_controls: 2,
                    ..Default::default()
                },
                Instant::now(),
                now(),
            )
            .unwrap()
            .is_none()
    );
    assert_eq!(lifecycle.state(), BotState::Capturing);
}

#[test]
fn captcha_and_error_page_emit_distinct_admission_terminal_reasons() {
    let started = Instant::now();
    let mut captcha = WorkerLifecycle::new("bot-captcha");
    captcha.launch_started(now()).unwrap();
    assert!(
        captcha
            .observe_admission(
                &AdmissionSnapshot {
                    ambiguous_error_indicator: Some("Try again".into()),
                    visible_recaptcha_challenge: true,
                    ..Default::default()
                },
                started,
                now(),
            )
            .unwrap()
            .is_some()
    );
    let captcha_event = captcha
        .observe_admission(
            &AdmissionSnapshot {
                ambiguous_error_indicator: Some("Try again".into()),
                visible_recaptcha_challenge: true,
                ..Default::default()
            },
            started + Duration::from_secs(120),
            now(),
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        reason_kind(captcha_event),
        TerminalReasonKind::AuthenticationFailed
    );
    assert_eq!(captcha.state(), BotState::Failed);

    let mut error_page = WorkerLifecycle::new("bot-error-page");
    error_page.launch_started(now()).unwrap();
    let error_event = error_page
        .observe_admission(
            &AdmissionSnapshot {
                ambiguous_error_indicator: Some("can't join this video call".into()),
                ..Default::default()
            },
            Instant::now(),
            now(),
        )
        .unwrap()
        .unwrap();
    assert_eq!(reason_kind(error_event), TerminalReasonKind::ProviderError);
    assert_eq!(error_page.state(), BotState::Failed);
}

#[derive(Default)]
struct MemoryRecordingStore {
    objects: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("memory recording store failed")]
struct MemoryRecordingStoreError;

#[async_trait::async_trait]
impl RecordingChunkStore for MemoryRecordingStore {
    type Error = MemoryRecordingStoreError;

    async fn put(
        &mut self,
        key: &str,
        _content_type: &str,
        _body: Vec<u8>,
    ) -> Result<StoredRecordingObject, Self::Error> {
        self.objects.push(key.into());
        Ok(StoredRecordingObject { uri: key.into() })
    }
}

#[tokio::test]
async fn two_hour_capture_finalizes_recording_chunks_before_meeting_ended() {
    let mut lifecycle = WorkerLifecycle::new("bot-long-recording");
    join_and_capture(&mut lifecycle);

    let mut sink = ChunkedRecordingSink::new(
        ChunkedRecordingConfig {
            object_prefix: "recordings/job-reliability".into(),
            chunk_duration: Duration::from_secs(60),
            max_lateness: Duration::from_secs(60),
        },
        MemoryRecordingStore::default(),
    )
    .unwrap();

    let mut outputs = Vec::new();
    for minute in 0..120_u64 {
        outputs.extend(
            sink.write_frame(AudioFrame {
                sequence: minute + 1,
                track_index: 0,
                sample_rate: 16_000,
                start_ms: minute * 60_000,
                samples: vec![1],
                speaker: None,
            })
            .await
            .unwrap(),
        );
    }
    outputs.extend(sink.finish(Duration::from_secs(2 * 60 * 60)).await.unwrap());

    assert_eq!(outputs.len(), 120);
    assert!(
        outputs
            .iter()
            .all(|output| matches!(output, AudioFrameSinkOutput::RecordingChunkReady(_)))
    );

    let chunk_events: Vec<_> = outputs
        .into_iter()
        .map(|output| lifecycle.emit_payload(output.into(), now()))
        .collect();
    assert!(
        chunk_events
            .iter()
            .all(|event| matches!(event.payload, CaptureEventPayload::RecordingChunkReady(_)))
    );
    assert_eq!(lifecycle.state(), BotState::Capturing);

    let ended = lifecycle
        .observe_runtime(
            &RuntimeSnapshot {
                meeting_ended_indicator: Some("the meeting has ended".into()),
                ..Default::default()
            },
            Instant::now(),
            now(),
        )
        .unwrap()
        .unwrap();
    assert!(
        chunk_events
            .iter()
            .all(|event| event.sequence < ended.sequence)
    );
    assert_eq!(reason_kind(ended), TerminalReasonKind::MeetingEnded);
    assert_eq!(lifecycle.state(), BotState::Completed);
}
