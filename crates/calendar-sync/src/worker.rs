use std::sync::{Arc, Mutex};

use apalis::prelude::Data;
use apalis_cron::Tick;
use chrono::Utc;
use tokio::time::timeout;

use crate::runtime::{CalendarSyncRuntime, CalendarSyncWorkerEvent, SyncReason, SyncStatus};
use crate::source::{BoxError, CalendarSyncSource};

#[derive(Clone)]
pub struct WorkerState {
    pub source: Arc<dyn CalendarSyncSource>,
    pub runtime: Arc<dyn CalendarSyncRuntime>,
    pub sync_timeout: std::time::Duration,
    inner: Arc<Mutex<InnerState>>,
}

#[derive(Debug, Default)]
struct InnerState {
    status: Option<SyncStatus>,
    pending_reasons: Vec<SyncReason>,
}

impl WorkerState {
    pub fn new(
        source: impl CalendarSyncSource,
        runtime: impl CalendarSyncRuntime,
        sync_timeout: std::time::Duration,
    ) -> Self {
        Self {
            source: Arc::new(source),
            runtime: Arc::new(runtime),
            sync_timeout,
            inner: Arc::new(Mutex::new(InnerState {
                status: Some(SyncStatus::Idle),
                pending_reasons: Vec::new(),
            })),
        }
    }

    pub fn status(&self) -> SyncStatus {
        self.inner
            .lock()
            .unwrap()
            .status
            .unwrap_or(SyncStatus::Idle)
    }

    pub fn request_sync(&self, reason: SyncReason) -> SyncStatus {
        tracing::info!(?reason, "calendar sync requested");
        let mut inner = self.inner.lock().unwrap();
        push_unique(&mut inner.pending_reasons, reason);

        let previous = inner.status.unwrap_or(SyncStatus::Idle);
        if previous == SyncStatus::Idle {
            inner.status = Some(SyncStatus::Scheduled);
        }

        drop(inner);
        let state = self.clone();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to create calendar sync runtime");
            runtime.block_on(async move {
                drive(state).await;
            });
        });

        match previous {
            SyncStatus::Idle => SyncStatus::Scheduled,
            other => other,
        }
    }

    fn take_pending_reasons(&self) -> Option<Vec<SyncReason>> {
        let mut inner = self.inner.lock().unwrap();
        if inner.pending_reasons.is_empty() {
            return None;
        }

        inner.status = Some(SyncStatus::Running);
        let reasons = std::mem::take(&mut inner.pending_reasons);
        Some(reasons)
    }

    fn finish_run(&self) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let next_status = if inner.pending_reasons.is_empty() {
            SyncStatus::Idle
        } else {
            SyncStatus::Scheduled
        };
        inner.status = Some(next_status);
        next_status == SyncStatus::Scheduled
    }
}

pub async fn enqueue_interval_tick(
    _tick: Tick<Utc>,
    ctx: Data<WorkerState>,
) -> Result<(), BoxError> {
    ctx.request_sync(SyncReason::Interval);
    Ok(())
}

pub async fn drive(state: WorkerState) {
    loop {
        let Some(reasons) = state.take_pending_reasons() else {
            break;
        };

        tracing::info!(?reasons, "calendar sync started");
        state.runtime.emit(CalendarSyncWorkerEvent::SyncStarted {
            reasons: reasons.clone(),
        });

        match timeout(state.sync_timeout, state.source.sync(reasons.clone())).await {
            Ok(Ok(outcome)) => {
                tracing::info!(
                    ?reasons,
                    data_changed = outcome.data_changed,
                    "calendar sync finished"
                );
                state.runtime.emit(CalendarSyncWorkerEvent::SyncFinished {
                    reasons,
                    data_changed: outcome.data_changed,
                });
            }
            Ok(Err(error)) => {
                tracing::error!(?reasons, error = %error, "calendar sync failed");
                state.runtime.emit(CalendarSyncWorkerEvent::SyncFailed {
                    reasons,
                    error: error.to_string(),
                });
            }
            Err(_) => {
                tracing::error!(
                    ?reasons,
                    timeout_secs = state.sync_timeout.as_secs(),
                    "calendar sync timed out"
                );
                state.runtime.emit(CalendarSyncWorkerEvent::SyncFailed {
                    reasons,
                    error: format!(
                        "calendar sync timed out after {}s",
                        state.sync_timeout.as_secs()
                    ),
                });
            }
        }

        if !state.finish_run() {
            break;
        }
    }
}

fn push_unique(reasons: &mut Vec<SyncReason>, reason: SyncReason) {
    if !reasons.contains(&reason) {
        reasons.push(reason);
    }
}
