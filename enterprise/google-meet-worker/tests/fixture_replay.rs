use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use anarlog_enterprise_google_meet_worker::{
    AdmissionClassifier, AdmissionOutcome, AdmissionRejectionReason, AdmissionSnapshot,
    RuntimeClassifier, RuntimeOutcome, RuntimeSnapshot, WorkerLifecycle,
};
use anlg_meeting_capture::{BotState, CaptureEventPayload, TerminalReasonKind};
use chrono::{DateTime, Utc};
use serde::Deserialize;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/vexa-v0.12.18")
}

fn load_json<T: for<'de> Deserialize<'de>>(relative: &str) -> T {
    let path = fixture_root().join(relative);
    serde_json::from_str(
        &fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

#[derive(Debug, Deserialize)]
struct ClassifierFixture {
    kind: String,
    elapsed_ms: u64,
    snapshot: serde_json::Value,
    expected: ExpectedOutcome,
}

#[derive(Debug, Deserialize)]
struct ExpectedOutcome {
    outcome: String,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    indicator: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Scenario {
    id: String,
    expected_state: String,
    expected_terminal: String,
    retryable: bool,
    steps: Vec<ScenarioStep>,
}

#[derive(Debug, Deserialize)]
struct ScenarioStep {
    action: String,
    #[serde(default)]
    fixture: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-08-17T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

#[test]
fn replays_every_vexa_classifier_fixture() {
    let mut cases = Vec::new();
    for kind in ["admission", "runtime"] {
        let dir = fixture_root().join(kind);
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                cases.push((kind, path));
            }
        }
    }
    cases.sort_by(|left, right| left.1.file_name().cmp(&right.1.file_name()));
    assert!(
        cases.len() >= 12,
        "expected the committed Vexa behavior matrix fixtures"
    );

    for (kind, path) in cases {
        let relative = format!("{kind}/{}", path.file_name().unwrap().to_string_lossy());
        let fixture: ClassifierFixture = load_json(&relative);
        let started = Instant::now();
        match fixture.kind.as_str() {
            "admission" => {
                let snapshot: AdmissionSnapshot =
                    serde_json::from_value(fixture.snapshot.clone()).unwrap();
                let mut classifier = AdmissionClassifier::default();
                classifier.classify(&snapshot, started);
                let outcome = classifier.classify(
                    &snapshot,
                    started + Duration::from_millis(fixture.elapsed_ms),
                );
                assert_admission(&relative, &outcome, &fixture.expected);
            }
            "runtime" => {
                let snapshot: RuntimeSnapshot =
                    serde_json::from_value(fixture.snapshot.clone()).unwrap();
                let mut classifier = RuntimeClassifier::default();
                classifier.classify(&snapshot, started);
                let outcome = classifier.classify(
                    &snapshot,
                    started + Duration::from_millis(fixture.elapsed_ms),
                );
                assert_runtime(&relative, &outcome, &fixture.expected);
            }
            other => panic!("{relative}: unknown fixture kind {other}"),
        }
    }
}

fn assert_admission(id: &str, outcome: &AdmissionOutcome, expected: &ExpectedOutcome) {
    match expected.outcome.as_str() {
        "admitted" => assert_eq!(outcome, &AdmissionOutcome::Admitted, "{id}"),
        "waiting" => assert_eq!(outcome, &AdmissionOutcome::WaitingForAdmission, "{id}"),
        "consent" => assert_eq!(outcome, &AdmissionOutcome::ConsentRequired, "{id}"),
        "rejected" => match outcome {
            AdmissionOutcome::Rejected(rejection) => {
                let reason = match expected.reason.as_deref() {
                    Some("host_denied") => AdmissionRejectionReason::HostDenied,
                    Some("captcha_unsolved") => AdmissionRejectionReason::CaptchaUnsolved,
                    Some("error_page") => AdmissionRejectionReason::ErrorPage,
                    other => panic!("{id}: unknown rejection {other:?}"),
                };
                assert_eq!(rejection.reason, reason, "{id}");
                if let Some(indicator) = &expected.indicator {
                    assert_eq!(&rejection.indicator, indicator, "{id}");
                }
            }
            other => panic!("{id}: expected rejection, got {other:?}"),
        },
        other => panic!("{id}: unknown expected outcome {other}"),
    }
}

fn assert_runtime(id: &str, outcome: &RuntimeOutcome, expected: &ExpectedOutcome) {
    match expected.outcome.as_str() {
        "active" => assert_eq!(outcome, &RuntimeOutcome::Active, "{id}"),
        "removed" => match outcome {
            RuntimeOutcome::Removed(indicator) => {
                assert_eq!(indicator, expected.indicator.as_ref().unwrap(), "{id}");
            }
            other => panic!("{id}: expected removed, got {other:?}"),
        },
        "meeting_ended" => match outcome {
            RuntimeOutcome::MeetingEnded(indicator) => {
                assert_eq!(indicator, expected.indicator.as_ref().unwrap(), "{id}");
            }
            other => panic!("{id}: expected meeting ended, got {other:?}"),
        },
        "network_lost" => match outcome {
            RuntimeOutcome::NetworkLost(indicator) => {
                assert_eq!(indicator, expected.indicator.as_ref().unwrap(), "{id}");
            }
            other => panic!("{id}: expected network lost, got {other:?}"),
        },
        other => panic!("{id}: unknown expected runtime outcome {other}"),
    }
}

#[test]
fn replays_lifecycle_scenarios_to_provider_neutral_terminal_reasons() {
    let scenarios: Vec<Scenario> = load_json("scenarios.json");
    assert!(!scenarios.is_empty());

    for scenario in scenarios {
        let mut lifecycle = WorkerLifecycle::new(&scenario.id);
        let started = Instant::now();
        let mut last_terminal = None;
        for step in &scenario.steps {
            if step.action == "stopped_by_request" {
                for event in lifecycle.stopped_by_request(now()).unwrap() {
                    if let CaptureEventPayload::Lifecycle(transition) = event.payload {
                        last_terminal = transition.reason;
                    }
                }
                continue;
            }
            let event = match step.action.as_str() {
                "launch" => Some(lifecycle.launch_started(now()).unwrap()),
                "admission" => {
                    let fixture: ClassifierFixture =
                        load_json(step.fixture.as_ref().expect("admission fixture"));
                    let snapshot: AdmissionSnapshot =
                        serde_json::from_value(fixture.snapshot).unwrap();
                    let observed_at = started + Duration::from_millis(fixture.elapsed_ms);
                    if fixture.elapsed_ms > 0 {
                        let _ = lifecycle
                            .observe_admission(&snapshot, started, now())
                            .unwrap();
                    }
                    lifecycle
                        .observe_admission(&snapshot, observed_at, now())
                        .unwrap()
                }
                "capture_started" => Some(lifecycle.capture_started(now()).unwrap()),
                "runtime" => {
                    let fixture: ClassifierFixture =
                        load_json(step.fixture.as_ref().expect("runtime fixture"));
                    let snapshot: RuntimeSnapshot =
                        serde_json::from_value(fixture.snapshot).unwrap();
                    let observed_at = started + Duration::from_millis(fixture.elapsed_ms);
                    if fixture.elapsed_ms > 0 {
                        let _ = lifecycle
                            .observe_runtime(&snapshot, started, now())
                            .unwrap();
                    }
                    lifecycle
                        .observe_runtime(&snapshot, observed_at, now())
                        .unwrap()
                }
                "admission_timeout" => Some(lifecycle.admission_timed_out(now()).unwrap()),
                "worker_exited" => Some(
                    lifecycle
                        .worker_exited(
                            step.message.clone().unwrap_or_else(|| "crash".into()),
                            now(),
                        )
                        .unwrap(),
                ),
                "stt_unavailable" => Some(
                    lifecycle
                        .stt_unavailable(
                            step.message
                                .clone()
                                .unwrap_or_else(|| "speech-to-text unavailable".into()),
                            now(),
                        )
                        .unwrap(),
                ),
                other => panic!("{}: unknown action {other}", scenario.id),
            };
            if let Some(event) = event
                && let CaptureEventPayload::Lifecycle(transition) = event.payload
            {
                last_terminal = transition.reason;
            }
        }

        let expected_state = match scenario.expected_state.as_str() {
            "failed" => BotState::Failed,
            "completed" => BotState::Completed,
            "canceled" => BotState::Canceled,
            other => panic!("{}: unknown state {other}", scenario.id),
        };
        assert_eq!(lifecycle.state(), expected_state, "{}", scenario.id);
        let reason = last_terminal.expect(&scenario.id);
        let expected_kind = serde_json::from_value(serde_json::Value::String(
            scenario.expected_terminal.clone(),
        ))
        .unwrap_or_else(|error| {
            panic!(
                "{}: invalid terminal {}: {error}",
                scenario.id, scenario.expected_terminal
            )
        });
        let expected_kind: TerminalReasonKind = expected_kind;
        assert_eq!(reason.kind, expected_kind, "{}", scenario.id);
        assert_eq!(reason.retryable, scenario.retryable, "{}", scenario.id);
    }
}
