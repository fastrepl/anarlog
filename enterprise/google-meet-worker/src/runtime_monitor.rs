use std::{future::Future, time::Duration};

use anlg_meeting_capture::{CaptureEvent, TransitionError};
use chrono::Utc;
use tokio::sync::watch;

use crate::{CdpError, CdpPage, RuntimeOutcome, RuntimeSnapshot, WorkerLifecycle};

pub const DEFAULT_RUNTIME_POLL_INTERVAL: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, Copy)]
pub struct RuntimeMonitor {
    poll_interval: Duration,
}

impl RuntimeMonitor {
    pub fn new(poll_interval: Duration) -> Result<Self, RuntimeMonitorError> {
        if poll_interval.is_zero() {
            return Err(RuntimeMonitorError::InvalidPollInterval);
        }
        Ok(Self { poll_interval })
    }

    pub async fn run(
        &self,
        page: &mut CdpPage,
        lifecycle: &mut WorkerLifecycle,
        stop: watch::Receiver<bool>,
    ) -> Result<Vec<CaptureEvent>, RuntimeMonitorError> {
        self.run_with_source(page, lifecycle, stop).await
    }

    pub(crate) fn poll_interval(&self) -> Duration {
        self.poll_interval
    }

    pub(crate) async fn probe_outcome(
        &self,
        page: &mut CdpPage,
        lifecycle: &mut WorkerLifecycle,
    ) -> Result<Option<RuntimeOutcome>, RuntimeMonitorError> {
        let snapshot = page.probe_runtime().await?;
        Ok(lifecycle.classify_runtime(&snapshot, std::time::Instant::now()))
    }

    async fn run_with_source<S: RuntimeProbeSource>(
        &self,
        source: &mut S,
        lifecycle: &mut WorkerLifecycle,
        mut stop: watch::Receiver<bool>,
    ) -> Result<Vec<CaptureEvent>, RuntimeMonitorError> {
        let mut events = Vec::new();
        loop {
            if *stop.borrow() {
                events.extend(lifecycle.stopped_by_request(Utc::now())?);
                return Ok(events);
            }
            let snapshot = source.probe().await?;
            if let Some(event) =
                lifecycle.observe_runtime(&snapshot, std::time::Instant::now(), Utc::now())?
            {
                events.push(event);
            }
            if lifecycle.state().is_terminal() {
                return Ok(events);
            }

            tokio::select! {
                _ = tokio::time::sleep(self.poll_interval) => {}
                result = stop.changed() => {
                    if result.is_err() || *stop.borrow() {
                        events.extend(lifecycle.stopped_by_request(Utc::now())?);
                        return Ok(events);
                    }
                }
            }
        }
    }
}

impl Default for RuntimeMonitor {
    fn default() -> Self {
        Self {
            poll_interval: DEFAULT_RUNTIME_POLL_INTERVAL,
        }
    }
}

trait RuntimeProbeSource {
    fn probe(&mut self) -> impl Future<Output = Result<RuntimeSnapshot, CdpError>> + Send;
}

impl RuntimeProbeSource for CdpPage {
    async fn probe(&mut self) -> Result<RuntimeSnapshot, CdpError> {
        self.probe_runtime().await
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeMonitorError {
    #[error("runtime poll interval must be greater than zero")]
    InvalidPollInterval,
    #[error(transparent)]
    Cdp(#[from] CdpError),
    #[error(transparent)]
    Transition(#[from] TransitionError),
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::VecDeque;

    use anlg_meeting_capture::BotState;

    use crate::AdmissionSnapshot;

    struct Snapshots(VecDeque<RuntimeSnapshot>);

    impl RuntimeProbeSource for Snapshots {
        async fn probe(&mut self) -> Result<RuntimeSnapshot, CdpError> {
            Ok(self.0.pop_front().unwrap_or_default())
        }
    }

    fn capturing_lifecycle() -> WorkerLifecycle {
        let mut lifecycle = WorkerLifecycle::new("bot-1");
        lifecycle.launch_started(Utc::now()).unwrap();
        lifecycle
            .observe_admission(
                &AdmissionSnapshot {
                    self_name_nodes: 1,
                    ..Default::default()
                },
                std::time::Instant::now(),
                Utc::now(),
            )
            .unwrap();
        lifecycle.capture_started(Utc::now()).unwrap();
        lifecycle
    }

    #[tokio::test]
    async fn meeting_end_finishes_the_monitor() {
        let mut source = Snapshots(VecDeque::from([RuntimeSnapshot {
            meeting_ended_indicator: Some("meeting ended".into()),
            ..Default::default()
        }]));
        let mut lifecycle = capturing_lifecycle();
        let (_stop_tx, stop_rx) = watch::channel(false);

        let events = RuntimeMonitor::default()
            .run_with_source(&mut source, &mut lifecycle, stop_rx)
            .await
            .unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(lifecycle.state(), BotState::Completed);
    }

    #[tokio::test]
    async fn explicit_stop_completes_without_an_extra_probe() {
        let mut source = Snapshots(VecDeque::new());
        let mut lifecycle = capturing_lifecycle();
        let (_stop_tx, stop_rx) = watch::channel(true);

        let events = RuntimeMonitor::default()
            .run_with_source(&mut source, &mut lifecycle, stop_rx)
            .await
            .unwrap();

        assert_eq!(events.len(), 2);
        assert_eq!(lifecycle.state(), BotState::Completed);
    }

    #[test]
    fn rejects_zero_poll_interval() {
        assert!(matches!(
            RuntimeMonitor::new(Duration::ZERO),
            Err(RuntimeMonitorError::InvalidPollInterval)
        ));
    }
}
