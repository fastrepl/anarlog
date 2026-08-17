use std::{error::Error, time::Duration};

use anlg_meeting_capture::{BotState, CaptureEvent, CaptureEventPayload, TransitionError};
use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::{mpsc, watch};

use crate::{
    CaptureEventSink, CaptureEventSinkError, ControlPlaneEventSink, WorkerCheckpoint, WorkerLease,
    WorkerLifecycle, WorkerLifecycleResumeError,
};

pub const DEFAULT_LEASE_RENEW_INTERVAL: Duration = Duration::from_secs(20);
pub const MAX_LEASE_RENEW_INTERVAL: Duration = Duration::from_secs(30);
const EVENT_CHANNEL_CAPACITY: usize = 32;

#[async_trait]
pub trait CaptureJobControlPlane: Send + Sync {
    type Error: Error + Send + Sync + 'static;

    async fn read_checkpoint(&self) -> Result<WorkerCheckpoint, Self::Error>;
    async fn claim(&self, worker_id: &str, lease_id: &str) -> Result<WorkerLease, Self::Error>;
    async fn renew_lease(&self) -> Result<WorkerLease, Self::Error>;
    async fn append(&self, event: &CaptureEvent) -> Result<(), Self::Error>;
}

#[async_trait]
impl CaptureJobControlPlane for ControlPlaneEventSink {
    type Error = CaptureEventSinkError;

    async fn read_checkpoint(&self) -> Result<WorkerCheckpoint, Self::Error> {
        self.read_checkpoint().await
    }

    async fn claim(&self, worker_id: &str, lease_id: &str) -> Result<WorkerLease, Self::Error> {
        self.claim(worker_id, lease_id).await
    }

    async fn renew_lease(&self) -> Result<WorkerLease, Self::Error> {
        self.renew_lease().await
    }

    async fn append(&self, event: &CaptureEvent) -> Result<(), Self::Error> {
        CaptureEventSink::append(self, event).await
    }
}

#[async_trait]
pub trait CaptureJobRuntime: Send {
    type Error: Error + Send + Sync + 'static;

    fn validate_checkpoint(&self, _checkpoint: &WorkerCheckpoint) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn run(
        &mut self,
        checkpoint: &WorkerCheckpoint,
        lifecycle: &mut WorkerLifecycle,
        events: mpsc::Sender<CaptureEvent>,
    ) -> Result<(), Self::Error>;

    async fn cleanup(&mut self) -> Result<Vec<CaptureEventPayload>, Self::Error>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureJobSupervisorConfig {
    pub lease_renew_interval: Duration,
}

impl Default for CaptureJobSupervisorConfig {
    fn default() -> Self {
        Self {
            lease_renew_interval: DEFAULT_LEASE_RENEW_INTERVAL,
        }
    }
}

impl CaptureJobSupervisorConfig {
    pub fn validate(self) -> Result<Self, CaptureJobSupervisorConfigError> {
        if self.lease_renew_interval.is_zero()
            || self.lease_renew_interval > MAX_LEASE_RENEW_INTERVAL
        {
            return Err(CaptureJobSupervisorConfigError::InvalidLeaseRenewInterval);
        }
        Ok(self)
    }
}

pub struct CaptureJobSupervisor<C, R> {
    control_plane: C,
    runtime: R,
    worker_id: String,
    lease_id: String,
    config: CaptureJobSupervisorConfig,
}

impl<C, R> CaptureJobSupervisor<C, R>
where
    C: CaptureJobControlPlane,
    R: CaptureJobRuntime,
{
    pub fn new(
        control_plane: C,
        runtime: R,
        worker_id: impl Into<String>,
        lease_id: impl Into<String>,
        config: CaptureJobSupervisorConfig,
    ) -> Result<Self, CaptureJobSupervisorConfigError> {
        let worker_id = worker_id.into();
        let lease_id = lease_id.into();
        if worker_id.is_empty() || lease_id.is_empty() {
            return Err(CaptureJobSupervisorConfigError::MissingLeaseIdentity);
        }
        if !valid_identifier(&worker_id) || !valid_identifier(&lease_id) {
            return Err(CaptureJobSupervisorConfigError::InvalidLeaseIdentity);
        }
        Ok(Self {
            control_plane,
            runtime,
            worker_id,
            lease_id,
            config: config.validate()?,
        })
    }

    pub async fn run(
        mut self,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<CaptureJobSupervisorOutcome, CaptureJobSupervisorError<C::Error, R::Error>> {
        let checkpoint = self
            .control_plane
            .read_checkpoint()
            .await
            .map_err(CaptureJobSupervisorError::ControlPlane)?;
        if checkpoint.state.is_terminal() {
            return Ok(CaptureJobSupervisorOutcome::AlreadyTerminal(
                checkpoint.state,
            ));
        }
        if *shutdown.borrow() {
            return Ok(CaptureJobSupervisorOutcome::ShutdownBeforeClaim);
        }
        self.runtime
            .validate_checkpoint(&checkpoint)
            .map_err(CaptureJobSupervisorError::RuntimeValidation)?;

        self.control_plane
            .claim(&self.worker_id, &self.lease_id)
            .await
            .map_err(CaptureJobSupervisorError::ControlPlane)?;
        let checkpoint = self
            .control_plane
            .read_checkpoint()
            .await
            .map_err(CaptureJobSupervisorError::ControlPlane)?;
        if checkpoint.state.is_terminal() {
            return Ok(CaptureJobSupervisorOutcome::AlreadyTerminal(
                checkpoint.state,
            ));
        }
        let mut lifecycle = WorkerLifecycle::resume(
            checkpoint.bot_id.clone(),
            checkpoint.state,
            checkpoint.next_sequence,
        )?;

        if lifecycle.state() != BotState::Queued {
            let event = lifecycle.worker_exited(
                "capture worker restarted after losing browser ownership",
                Utc::now(),
            )?;
            self.control_plane
                .append(&event)
                .await
                .map_err(CaptureJobSupervisorError::ControlPlane)?;
            return Ok(CaptureJobSupervisorOutcome::Terminal(lifecycle.state()));
        }

        let (events_tx, mut events_rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        let mut renewals = tokio::time::interval_at(
            tokio::time::Instant::now() + self.config.lease_renew_interval,
            self.config.lease_renew_interval,
        );
        let mut runtime = Box::pin(self.runtime.run(&checkpoint, &mut lifecycle, events_tx));
        let mut events_closed = false;
        let mut deferred_terminal = None;
        let exit = loop {
            tokio::select! {
                event = events_rx.recv(), if !events_closed => {
                    match event {
                        Some(event) if is_terminal_event(&event) => deferred_terminal = Some(event),
                        Some(event) => {
                            if let Err(error) = self.control_plane.append(&event).await {
                                break RuntimeExit::ControlPlane(error);
                            }
                        }
                        None => events_closed = true,
                    }
                }
                result = &mut runtime => break RuntimeExit::Runtime(result),
                _ = renewals.tick() => {
                    if let Err(error) = self.control_plane.renew_lease().await {
                        break RuntimeExit::LeaseLost(error);
                    }
                }
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        break RuntimeExit::Shutdown;
                    }
                }
            }
        };
        drop(runtime);

        if matches!(exit, RuntimeExit::Runtime(_) | RuntimeExit::Shutdown) {
            while let Ok(event) = events_rx.try_recv() {
                if is_terminal_event(&event) {
                    deferred_terminal = Some(event);
                } else if let Err(error) = self.control_plane.append(&event).await {
                    self.runtime
                        .cleanup()
                        .await
                        .map_err(CaptureJobSupervisorError::Cleanup)?;
                    return Err(CaptureJobSupervisorError::ControlPlane(error));
                }
            }
        }
        let cleanup_payloads = self
            .runtime
            .cleanup()
            .await
            .map_err(CaptureJobSupervisorError::Cleanup)?;

        if matches!(exit, RuntimeExit::Runtime(_) | RuntimeExit::Shutdown) {
            if lifecycle.state().is_terminal() && !cleanup_payloads.is_empty() {
                return Err(CaptureJobSupervisorError::CleanupOutputAfterTerminal);
            }
            for payload in cleanup_payloads {
                if matches!(payload, CaptureEventPayload::Lifecycle(_)) {
                    return Err(CaptureJobSupervisorError::LifecycleOutputFromCleanup);
                }
                let event = lifecycle.emit_payload(payload, Utc::now());
                self.control_plane
                    .append(&event)
                    .await
                    .map_err(CaptureJobSupervisorError::ControlPlane)?;
            }
        }

        if matches!(exit, RuntimeExit::Runtime(_) | RuntimeExit::Shutdown)
            && let Some(event) = deferred_terminal
        {
            self.control_plane
                .append(&event)
                .await
                .map_err(CaptureJobSupervisorError::ControlPlane)?;
        }

        match exit {
            RuntimeExit::ControlPlane(error) | RuntimeExit::LeaseLost(error) => {
                Err(CaptureJobSupervisorError::ControlPlane(error))
            }
            RuntimeExit::Shutdown => {
                for event in lifecycle.stopped_by_request(Utc::now())? {
                    self.control_plane
                        .append(&event)
                        .await
                        .map_err(CaptureJobSupervisorError::ControlPlane)?;
                }
                Ok(CaptureJobSupervisorOutcome::Terminal(lifecycle.state()))
            }
            RuntimeExit::Runtime(result) => {
                if lifecycle.state().is_terminal() {
                    return Ok(CaptureJobSupervisorOutcome::Terminal(lifecycle.state()));
                }
                if let Err(error) = result {
                    let event = lifecycle
                        .worker_exited(format!("capture runtime exited: {error}"), Utc::now())?;
                    self.control_plane
                        .append(&event)
                        .await
                        .map_err(CaptureJobSupervisorError::ControlPlane)?;
                } else if !lifecycle.state().is_terminal() {
                    let event = lifecycle.worker_exited(
                        "capture runtime exited without a terminal state",
                        Utc::now(),
                    )?;
                    self.control_plane
                        .append(&event)
                        .await
                        .map_err(CaptureJobSupervisorError::ControlPlane)?;
                }
                Ok(CaptureJobSupervisorOutcome::Terminal(lifecycle.state()))
            }
        }
    }
}

enum RuntimeExit<C, R> {
    ControlPlane(C),
    LeaseLost(C),
    Runtime(Result<(), R>),
    Shutdown,
}

fn is_terminal_event(event: &CaptureEvent) -> bool {
    matches!(
        &event.payload,
        CaptureEventPayload::Lifecycle(transition) if transition.to.is_terminal()
    )
}

fn valid_identifier(value: &str) -> bool {
    value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureJobSupervisorOutcome {
    AlreadyTerminal(BotState),
    ShutdownBeforeClaim,
    Terminal(BotState),
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CaptureJobSupervisorConfigError {
    #[error("capture worker and lease IDs must be non-empty")]
    MissingLeaseIdentity,
    #[error("capture worker and lease IDs contain unsupported characters")]
    InvalidLeaseIdentity,
    #[error("capture lease renewal interval must be between one nanosecond and 30 seconds")]
    InvalidLeaseRenewInterval,
}

#[derive(Debug, thiserror::Error)]
pub enum CaptureJobSupervisorError<C, R>
where
    C: Error + 'static,
    R: Error + 'static,
{
    #[error("capture control-plane operation failed")]
    ControlPlane(#[source] C),
    #[error("capture runtime cleanup failed")]
    Cleanup(#[source] R),
    #[error("capture runtime rejected the worker checkpoint")]
    RuntimeValidation(#[source] R),
    #[error("capture runtime produced cleanup output after entering a terminal state")]
    CleanupOutputAfterTerminal,
    #[error("capture runtime cleanup cannot produce lifecycle events")]
    LifecycleOutputFromCleanup,
    #[error(transparent)]
    Resume(#[from] WorkerLifecycleResumeError),
    #[error(transparent)]
    Transition(#[from] TransitionError),
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use anlg_meeting_capture::{
        CaptureEventPayload, TerminalReason, TerminalReasonKind, TranscriptSegment,
    };
    use chrono::{DateTime, Utc};

    use super::*;

    #[derive(Debug, Clone, thiserror::Error)]
    #[error("{0}")]
    struct TestError(&'static str);

    #[derive(Clone)]
    struct TestControlPlane {
        checkpoint: WorkerCheckpoint,
        events: Arc<Mutex<Vec<CaptureEvent>>>,
        renewals: Arc<Mutex<usize>>,
        cleaned: Arc<Mutex<bool>>,
        append_cleanup_states: Arc<Mutex<Vec<bool>>>,
        reject_renewal: bool,
    }

    #[async_trait]
    impl CaptureJobControlPlane for TestControlPlane {
        type Error = TestError;

        async fn read_checkpoint(&self) -> Result<WorkerCheckpoint, Self::Error> {
            Ok(self.checkpoint.clone())
        }

        async fn claim(&self, worker_id: &str, lease_id: &str) -> Result<WorkerLease, Self::Error> {
            Ok(WorkerLease {
                worker_id: worker_id.into(),
                lease_id: lease_id.into(),
                epoch: 1,
                expires_at: now() + chrono::Duration::minutes(1),
            })
        }

        async fn renew_lease(&self) -> Result<WorkerLease, Self::Error> {
            *self.renewals.lock().unwrap() += 1;
            if self.reject_renewal {
                return Err(TestError("lease lost"));
            }
            Ok(WorkerLease {
                worker_id: "worker-a".into(),
                lease_id: "lease-a".into(),
                epoch: 1,
                expires_at: now() + chrono::Duration::minutes(1),
            })
        }

        async fn append(&self, event: &CaptureEvent) -> Result<(), Self::Error> {
            self.events.lock().unwrap().push(event.clone());
            self.append_cleanup_states
                .lock()
                .unwrap()
                .push(*self.cleaned.lock().unwrap());
            Ok(())
        }
    }

    struct TestRuntime {
        mode: RuntimeMode,
        cleaned: Arc<Mutex<bool>>,
    }

    enum RuntimeMode {
        Complete,
        Fail,
        FailWithCleanupOutput,
        TerminalThenWait,
        Wait,
    }

    struct TestHarness {
        supervisor: CaptureJobSupervisor<TestControlPlane, TestRuntime>,
        events: Arc<Mutex<Vec<CaptureEvent>>>,
        renewals: Arc<Mutex<usize>>,
        cleaned: Arc<Mutex<bool>>,
        append_cleanup_states: Arc<Mutex<Vec<bool>>>,
    }

    #[async_trait]
    impl CaptureJobRuntime for TestRuntime {
        type Error = TestError;

        async fn run(
            &mut self,
            checkpoint: &WorkerCheckpoint,
            lifecycle: &mut WorkerLifecycle,
            events: mpsc::Sender<CaptureEvent>,
        ) -> Result<(), Self::Error> {
            assert_eq!(checkpoint.job_id, "job-a");
            assert_eq!(
                checkpoint.meeting.url,
                "https://meet.google.com/abc-defg-hij"
            );
            events
                .send(lifecycle.launch_started(now()).unwrap())
                .await
                .unwrap();
            match self.mode {
                RuntimeMode::Complete => {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                    let joined = lifecycle.transition(BotState::Joined, None, now()).unwrap();
                    events.send(joined).await.unwrap();
                    let completed = lifecycle
                        .transition(
                            BotState::Completed,
                            Some(TerminalReason {
                                kind: TerminalReasonKind::MeetingEnded,
                                message: None,
                                retryable: false,
                            }),
                            now(),
                        )
                        .unwrap();
                    events.send(completed).await.unwrap();
                    Ok(())
                }
                RuntimeMode::Fail | RuntimeMode::FailWithCleanupOutput => {
                    Err(TestError("browser exited"))
                }
                RuntimeMode::TerminalThenWait => {
                    let joined = lifecycle.transition(BotState::Joined, None, now()).unwrap();
                    events.send(joined).await.unwrap();
                    let completed = lifecycle
                        .transition(
                            BotState::Completed,
                            Some(TerminalReason {
                                kind: TerminalReasonKind::MeetingEnded,
                                message: None,
                                retryable: false,
                            }),
                            now(),
                        )
                        .unwrap();
                    events.send(completed).await.unwrap();
                    std::future::pending().await
                }
                RuntimeMode::Wait => std::future::pending().await,
            }
        }

        async fn cleanup(&mut self) -> Result<Vec<CaptureEventPayload>, Self::Error> {
            *self.cleaned.lock().unwrap() = true;
            if matches!(self.mode, RuntimeMode::FailWithCleanupOutput) {
                Ok(vec![CaptureEventPayload::Transcript(TranscriptSegment {
                    id: "segment-cleanup".into(),
                    sequence: 0,
                    start_ms: 0,
                    end_ms: Some(500),
                    text: "final words".into(),
                    speaker: None,
                    is_final: true,
                })])
            } else {
                Ok(Vec::new())
            }
        }
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-08-17T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn checkpoint(state: BotState, next_sequence: u64) -> WorkerCheckpoint {
        WorkerCheckpoint {
            job_id: "job-a".into(),
            bot_id: "bot-a".into(),
            provider: anlg_meeting_capture::CaptureProviderKind::Anarlog,
            meeting: anlg_meeting_capture::MeetingReference {
                platform: anlg_meeting_capture::MeetingPlatform::GoogleMeet,
                url: "https://meet.google.com/abc-defg-hij".into(),
                external_id: None,
                calendar_event_id: None,
            },
            state,
            next_sequence,
        }
    }

    fn harness(
        state: BotState,
        next_sequence: u64,
        mode: RuntimeMode,
        reject_renewal: bool,
    ) -> TestHarness {
        let events = Arc::new(Mutex::new(Vec::new()));
        let renewals = Arc::new(Mutex::new(0));
        let cleaned = Arc::new(Mutex::new(false));
        let append_cleanup_states = Arc::new(Mutex::new(Vec::new()));
        let supervisor = CaptureJobSupervisor::new(
            TestControlPlane {
                checkpoint: checkpoint(state, next_sequence),
                events: events.clone(),
                renewals: renewals.clone(),
                cleaned: cleaned.clone(),
                append_cleanup_states: append_cleanup_states.clone(),
                reject_renewal,
            },
            TestRuntime {
                mode,
                cleaned: cleaned.clone(),
            },
            "worker-a",
            "lease-a",
            CaptureJobSupervisorConfig {
                lease_renew_interval: Duration::from_millis(1),
            },
        )
        .unwrap();
        TestHarness {
            supervisor,
            events,
            renewals,
            cleaned,
            append_cleanup_states,
        }
    }

    #[tokio::test]
    async fn renews_while_running_and_cleans_up_before_terminal_success() {
        let harness = harness(BotState::Queued, 0, RuntimeMode::Complete, false);
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);

        let outcome = harness.supervisor.run(shutdown_rx).await.unwrap();

        assert_eq!(
            outcome,
            CaptureJobSupervisorOutcome::Terminal(BotState::Completed)
        );
        assert!(*harness.renewals.lock().unwrap() > 0);
        assert!(*harness.cleaned.lock().unwrap());
        assert_eq!(
            harness
                .events
                .lock()
                .unwrap()
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert_eq!(
            *harness.append_cleanup_states.lock().unwrap(),
            vec![false, false, true]
        );
    }

    #[tokio::test]
    async fn terminalizes_an_interrupted_nonqueued_checkpoint_without_relaunching() {
        let harness = harness(BotState::Capturing, 7, RuntimeMode::Wait, false);
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);

        let outcome = harness.supervisor.run(shutdown_rx).await.unwrap();

        assert_eq!(
            outcome,
            CaptureJobSupervisorOutcome::Terminal(BotState::Failed)
        );
        assert!(!*harness.cleaned.lock().unwrap());
        let events = harness.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sequence, 7);
        let CaptureEventPayload::Lifecycle(transition) = &events[0].payload else {
            panic!("expected lifecycle event")
        };
        assert_eq!(
            transition.reason.as_ref().unwrap().kind,
            TerminalReasonKind::WorkerExited
        );
    }

    #[tokio::test]
    async fn lease_loss_cancels_and_cleans_up_without_emitting_a_stale_terminal_event() {
        let harness = harness(BotState::Queued, 0, RuntimeMode::Wait, true);
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);

        assert!(matches!(
            harness.supervisor.run(shutdown_rx).await,
            Err(CaptureJobSupervisorError::ControlPlane(TestError(
                "lease lost"
            )))
        ));
        assert_eq!(*harness.renewals.lock().unwrap(), 1);
        assert!(*harness.cleaned.lock().unwrap());
        assert!(harness.events.lock().unwrap().iter().all(|event| {
            !matches!(
                &event.payload,
                CaptureEventPayload::Lifecycle(transition) if transition.to.is_terminal()
            )
        }));
    }

    #[tokio::test]
    async fn lease_loss_discards_a_terminal_event_that_was_not_yet_committed() {
        let mut harness = harness(BotState::Queued, 0, RuntimeMode::TerminalThenWait, true);
        harness.supervisor.config.lease_renew_interval = Duration::from_millis(10);
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);

        assert!(matches!(
            harness.supervisor.run(shutdown_rx).await,
            Err(CaptureJobSupervisorError::ControlPlane(TestError(
                "lease lost"
            )))
        ));
        assert!(*harness.cleaned.lock().unwrap());
        assert_eq!(
            harness
                .events
                .lock()
                .unwrap()
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[tokio::test]
    async fn runtime_failure_cleans_up_then_emits_an_actionable_terminal_event() {
        let harness = harness(BotState::Queued, 0, RuntimeMode::Fail, false);
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);

        let outcome = harness.supervisor.run(shutdown_rx).await.unwrap();

        assert_eq!(
            outcome,
            CaptureJobSupervisorOutcome::Terminal(BotState::Failed)
        );
        assert!(*harness.cleaned.lock().unwrap());
        let events = harness.events.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].sequence, 0);
        assert_eq!(events[1].sequence, 1);
        let CaptureEventPayload::Lifecycle(transition) = &events[1].payload else {
            panic!("expected lifecycle event")
        };
        assert_eq!(
            transition.reason.as_ref().unwrap().kind,
            TerminalReasonKind::WorkerExited
        );
        assert_eq!(
            transition.reason.as_ref().unwrap().message.as_deref(),
            Some("capture runtime exited: browser exited")
        );
        assert_eq!(
            *harness.append_cleanup_states.lock().unwrap(),
            vec![false, true]
        );
    }

    #[tokio::test]
    async fn persists_final_media_output_before_runtime_failure_terminalizes() {
        let harness = harness(
            BotState::Queued,
            0,
            RuntimeMode::FailWithCleanupOutput,
            false,
        );
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);

        let outcome = harness.supervisor.run(shutdown_rx).await.unwrap();

        assert_eq!(
            outcome,
            CaptureJobSupervisorOutcome::Terminal(BotState::Failed)
        );
        let events = harness.events.lock().unwrap();
        assert_eq!(events.len(), 3);
        assert!(matches!(
            events[1].payload,
            CaptureEventPayload::Transcript(_)
        ));
        assert!(matches!(
            events[2].payload,
            CaptureEventPayload::Lifecycle(ref transition)
                if transition.to == BotState::Failed
        ));
        assert_eq!(
            events
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert_eq!(
            *harness.append_cleanup_states.lock().unwrap(),
            vec![false, true, true]
        );
    }

    #[tokio::test]
    async fn shutdown_cleans_up_and_publishes_phase_aware_terminal_events() {
        let harness = harness(BotState::Queued, 0, RuntimeMode::Wait, false);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let run = tokio::spawn(harness.supervisor.run(shutdown_rx));
        tokio::time::timeout(Duration::from_secs(1), async {
            while harness.events.lock().unwrap().is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        shutdown_tx.send(true).unwrap();

        let outcome = run.await.unwrap().unwrap();

        assert_eq!(
            outcome,
            CaptureJobSupervisorOutcome::Terminal(BotState::Completed)
        );
        assert!(*harness.cleaned.lock().unwrap());
        let events = harness.events.lock().unwrap();
        assert_eq!(events.last().unwrap().sequence, 2);
        assert!(matches!(
            &events.last().unwrap().payload,
            CaptureEventPayload::Lifecycle(transition)
                if transition.to == BotState::Completed
        ));
    }

    #[test]
    fn rejects_an_unsafe_renewal_configuration() {
        assert!(valid_identifier("worker-a.1"));
        assert!(!valid_identifier("../worker"));
        assert_eq!(
            CaptureJobSupervisorConfig::default()
                .validate()
                .unwrap()
                .lease_renew_interval,
            DEFAULT_LEASE_RENEW_INTERVAL
        );
        assert_eq!(
            CaptureJobSupervisorConfig {
                lease_renew_interval: Duration::ZERO,
            }
            .validate(),
            Err(CaptureJobSupervisorConfigError::InvalidLeaseRenewInterval)
        );
        assert_eq!(
            CaptureJobSupervisorConfig {
                lease_renew_interval: MAX_LEASE_RENEW_INTERVAL + Duration::from_nanos(1),
            }
            .validate(),
            Err(CaptureJobSupervisorConfigError::InvalidLeaseRenewInterval)
        );
    }
}
