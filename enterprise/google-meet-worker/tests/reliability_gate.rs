use std::time::{Duration, Instant};

use anarlog_enterprise_google_meet_worker::{AdmissionSnapshot, RuntimeSnapshot, WorkerLifecycle};
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
    let cases: [(
        &str,
        Box<dyn Fn(&mut WorkerLifecycle) -> TerminalReasonKind>,
    ); 9] = [
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
