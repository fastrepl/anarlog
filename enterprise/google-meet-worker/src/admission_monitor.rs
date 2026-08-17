use std::{future::Future, time::Duration};

use anlg_meeting_capture::{BotState, CaptureEvent, TransitionError};
use chrono::Utc;
use tokio::time::Instant;

use crate::{AdmissionSnapshot, CdpError, CdpPage, WorkerLifecycle};

pub const DEFAULT_ADMISSION_TIMEOUT: Duration = Duration::from_secs(10 * 60);
pub const DEFAULT_ADMISSION_POLL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmissionMonitorConfig {
    pub timeout: Duration,
    pub poll_interval: Duration,
}

impl Default for AdmissionMonitorConfig {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_ADMISSION_TIMEOUT,
            poll_interval: DEFAULT_ADMISSION_POLL_INTERVAL,
        }
    }
}

impl AdmissionMonitorConfig {
    pub fn validate(self) -> Result<Self, AdmissionMonitorError> {
        if self.timeout.is_zero()
            || self.poll_interval.is_zero()
            || self.poll_interval > self.timeout
        {
            return Err(AdmissionMonitorError::InvalidConfig {
                timeout: self.timeout,
                poll_interval: self.poll_interval,
            });
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AdmissionMonitor {
    config: AdmissionMonitorConfig,
}

impl AdmissionMonitor {
    pub fn new(config: AdmissionMonitorConfig) -> Result<Self, AdmissionMonitorError> {
        Ok(Self {
            config: config.validate()?,
        })
    }

    pub async fn wait(
        &self,
        page: &mut CdpPage,
        lifecycle: &mut WorkerLifecycle,
    ) -> Result<Vec<CaptureEvent>, AdmissionMonitorError> {
        self.wait_with_source(page, lifecycle).await
    }

    async fn wait_with_source<S: AdmissionProbeSource>(
        &self,
        source: &mut S,
        lifecycle: &mut WorkerLifecycle,
    ) -> Result<Vec<CaptureEvent>, AdmissionMonitorError> {
        let started_at = Instant::now();
        let deadline = started_at + self.config.timeout;
        let mut events = Vec::new();
        loop {
            let snapshot = source.probe().await?;
            let observed_at = Instant::now();
            if let Some(event) =
                lifecycle.observe_admission(&snapshot, observed_at.into_std(), Utc::now())?
            {
                events.push(event);
            }
            if matches!(lifecycle.state(), BotState::Joined | BotState::Failed) {
                return Ok(events);
            }
            if observed_at >= deadline {
                events.push(lifecycle.admission_timed_out(Utc::now())?);
                return Ok(events);
            }
            tokio::time::sleep(self.config.poll_interval.min(deadline - observed_at)).await;
        }
    }
}

trait AdmissionProbeSource {
    fn probe(&mut self) -> impl Future<Output = Result<AdmissionSnapshot, CdpError>> + Send;
}

impl AdmissionProbeSource for CdpPage {
    async fn probe(&mut self) -> Result<AdmissionSnapshot, CdpError> {
        self.probe_admission().await
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AdmissionMonitorError {
    #[error(
        "admission timeout and poll interval must be non-zero, and interval cannot exceed timeout (timeout {timeout:?}, interval {poll_interval:?})"
    )]
    InvalidConfig {
        timeout: Duration,
        poll_interval: Duration,
    },
    #[error(transparent)]
    Cdp(#[from] CdpError),
    #[error(transparent)]
    Transition(#[from] TransitionError),
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::VecDeque;

    struct Snapshots(VecDeque<AdmissionSnapshot>);

    impl AdmissionProbeSource for Snapshots {
        async fn probe(&mut self) -> Result<AdmissionSnapshot, CdpError> {
            Ok(self.0.pop_front().unwrap_or_default())
        }
    }

    fn monitor(timeout: Duration, poll_interval: Duration) -> AdmissionMonitor {
        AdmissionMonitor::new(AdmissionMonitorConfig {
            timeout,
            poll_interval,
        })
        .unwrap()
    }

    #[tokio::test]
    async fn emits_waiting_then_joined_without_resetting_the_deadline() {
        let mut source = Snapshots(VecDeque::from([
            AdmissionSnapshot {
                waiting_room_visible: true,
                ..Default::default()
            },
            AdmissionSnapshot {
                participant_tile_labels: vec!["Ada Lovelace".into()],
                ..Default::default()
            },
        ]));
        let mut lifecycle = WorkerLifecycle::new("bot-1");
        lifecycle.launch_started(Utc::now()).unwrap();

        let events = monitor(Duration::from_secs(1), Duration::from_millis(1))
            .wait_with_source(&mut source, &mut lifecycle)
            .await
            .unwrap();

        assert_eq!(events.len(), 2);
        assert_eq!(lifecycle.state(), BotState::Joined);
    }

    #[tokio::test]
    async fn performs_a_final_probe_then_emits_retryable_timeout() {
        let mut source = Snapshots(VecDeque::new());
        let mut lifecycle = WorkerLifecycle::new("bot-1");
        lifecycle.launch_started(Utc::now()).unwrap();

        let events = monitor(Duration::from_millis(2), Duration::from_millis(1))
            .wait_with_source(&mut source, &mut lifecycle)
            .await
            .unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(lifecycle.state(), BotState::Failed);
    }

    #[test]
    fn rejects_invalid_deadline_configuration() {
        for config in [
            AdmissionMonitorConfig {
                timeout: Duration::ZERO,
                poll_interval: Duration::from_secs(1),
            },
            AdmissionMonitorConfig {
                timeout: Duration::from_secs(1),
                poll_interval: Duration::from_secs(2),
            },
        ] {
            assert!(matches!(
                AdmissionMonitor::new(config),
                Err(AdmissionMonitorError::InvalidConfig { .. })
            ));
        }
    }
}
