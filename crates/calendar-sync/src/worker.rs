use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::FutureExt;
use tokio::sync::mpsc::{UnboundedReceiver, error::TryRecvError};
use tokio::time::{Instant, sleep_until, timeout};

use crate::runtime::{CalendarSyncRuntime, CalendarSyncWorkerEvent, SyncReason, SyncStatus};
use crate::source::CalendarSyncSource;

pub struct SyncWorker {
    source: Arc<dyn CalendarSyncSource>,
    runtime: Arc<dyn CalendarSyncRuntime>,
    status: Arc<Mutex<SyncStatus>>,
    rx: UnboundedReceiver<SyncReason>,
    interval: Duration,
    sync_timeout: Duration,
}

impl SyncWorker {
    pub fn new(
        source: Arc<dyn CalendarSyncSource>,
        runtime: Arc<dyn CalendarSyncRuntime>,
        status: Arc<Mutex<SyncStatus>>,
        rx: UnboundedReceiver<SyncReason>,
        interval: Duration,
        sync_timeout: Duration,
    ) -> Self {
        Self {
            source,
            runtime,
            status,
            rx,
            interval,
            sync_timeout,
        }
    }

    pub async fn run(mut self) {
        tracing::info!("calendar sync worker started");
        let mut last_attempt = Instant::now();

        loop {
            let next_interval = last_attempt + self.interval;

            tokio::select! {
                biased;
                maybe_reason = self.rx.recv() => {
                    let Some(first_reason) = maybe_reason else {
                        break;
                    };

                    self.set_status(SyncStatus::Scheduled);
                    let reasons = self.drain_pending(first_reason);
                    self.run_once(reasons).await;
                    last_attempt = Instant::now();
                }
                _ = sleep_until(next_interval) => {
                    self.run_once(vec![SyncReason::Interval]).await;
                    last_attempt = Instant::now();
                }
            }
        }

        tracing::warn!("calendar sync worker stopped");
    }

    fn drain_pending(&mut self, first_reason: SyncReason) -> Vec<SyncReason> {
        let mut reasons = vec![first_reason];

        loop {
            match self.rx.try_recv() {
                Ok(reason) => push_unique(&mut reasons, reason),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }

        reasons
    }

    async fn run_once(&self, reasons: Vec<SyncReason>) {
        let panic_reasons = reasons.clone();
        let outcome = AssertUnwindSafe(async {
            self.set_status(SyncStatus::Running);
            tracing::info!(?reasons, "calendar sync started");
            self.safe_emit(CalendarSyncWorkerEvent::SyncStarted {
                reasons: reasons.clone(),
            });

            match timeout(self.sync_timeout, self.source.sync(reasons.clone())).await {
                Ok(Ok(outcome)) => {
                    tracing::info!(
                        ?reasons,
                        data_changed = outcome.data_changed,
                        "calendar sync finished"
                    );
                    self.safe_emit(CalendarSyncWorkerEvent::SyncFinished {
                        reasons,
                        data_changed: outcome.data_changed,
                    });
                }
                Ok(Err(error)) => {
                    tracing::error!(?reasons, error = %error, "calendar sync failed");
                    self.safe_emit(CalendarSyncWorkerEvent::SyncFailed {
                        reasons,
                        error: error.to_string(),
                    });
                }
                Err(_) => {
                    tracing::error!(
                        ?reasons,
                        timeout_secs = self.sync_timeout.as_secs(),
                        "calendar sync timed out"
                    );
                    self.safe_emit(CalendarSyncWorkerEvent::SyncFailed {
                        reasons,
                        error: format!(
                            "calendar sync timed out after {}s",
                            self.sync_timeout.as_secs()
                        ),
                    });
                }
            }
        })
        .catch_unwind()
        .await;

        if let Err(panic_payload) = outcome {
            let panic_message = panic_message(&panic_payload);
            tracing::error!(?panic_reasons, panic = %panic_message, "calendar sync panicked");
            self.safe_emit(CalendarSyncWorkerEvent::SyncFailed {
                reasons: panic_reasons,
                error: format!("calendar sync panicked: {panic_message}"),
            });
        }

        self.set_status(SyncStatus::Idle);
    }

    fn set_status(&self, next: SyncStatus) {
        let should_emit = {
            let mut status = self.status.lock().unwrap();
            if *status == next {
                false
            } else {
                *status = next;
                true
            }
        };

        if should_emit {
            self.safe_emit(CalendarSyncWorkerEvent::StatusChanged { status: next });
        }
    }

    fn safe_emit(&self, event: CalendarSyncWorkerEvent) {
        if let Err(panic_payload) = catch_unwind(AssertUnwindSafe(|| self.runtime.emit(event))) {
            tracing::error!(
                panic = %panic_message(&panic_payload),
                "calendar sync runtime emit panicked"
            );
        }
    }
}

fn push_unique(reasons: &mut Vec<SyncReason>, reason: SyncReason) {
    if !reasons.contains(&reason) {
        reasons.push(reason);
    }
}

fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic".to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use tokio::sync::Notify;

    use super::*;
    use crate::source::{BoxError, SyncOutcome};

    #[derive(Clone, Default)]
    struct MockRuntime;

    impl CalendarSyncRuntime for MockRuntime {
        fn emit(&self, _event: CalendarSyncWorkerEvent) {}
    }

    #[derive(Clone)]
    struct MockSource {
        calls: Arc<Mutex<Vec<Vec<SyncReason>>>>,
        calls_changed: Arc<Notify>,
        block_first_call: Arc<AtomicBool>,
        release_first_call: Arc<Notify>,
    }

    impl MockSource {
        fn new(block_first_call: bool) -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                calls_changed: Arc::new(Notify::new()),
                block_first_call: Arc::new(AtomicBool::new(block_first_call)),
                release_first_call: Arc::new(Notify::new()),
            }
        }

        fn recorded_calls(&self) -> Vec<Vec<SyncReason>> {
            self.calls.lock().unwrap().clone()
        }

        async fn wait_for_calls(&self, count: usize) {
            loop {
                if self.recorded_calls().len() >= count {
                    return;
                }

                self.calls_changed.notified().await;
            }
        }

        fn release_blocked_call(&self) {
            self.release_first_call.notify_waiters();
        }
    }

    impl CalendarSyncSource for MockSource {
        fn sync(
            &self,
            reasons: Vec<SyncReason>,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<SyncOutcome, BoxError>> + Send + '_>,
        > {
            let calls = self.calls.clone();
            let calls_changed = self.calls_changed.clone();
            let block_first_call = self.block_first_call.clone();
            let release_first_call = self.release_first_call.clone();

            Box::pin(async move {
                calls.lock().unwrap().push(reasons);
                calls_changed.notify_waiters();

                if block_first_call.swap(false, Ordering::SeqCst) {
                    release_first_call.notified().await;
                }

                Ok(SyncOutcome::default())
            })
        }
    }

    #[tokio::test(start_paused = true)]
    async fn manual_sync_resets_interval_timer() {
        let source = MockSource::new(false);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let worker = SyncWorker::new(
            Arc::new(source.clone()),
            Arc::new(MockRuntime),
            Arc::new(Mutex::new(SyncStatus::Idle)),
            rx,
            Duration::from_secs(60),
            Duration::from_secs(5),
        );

        let task = tokio::spawn(worker.run());

        tokio::time::advance(Duration::from_secs(30)).await;
        tokio::task::yield_now().await;
        assert!(source.recorded_calls().is_empty());

        tx.send(SyncReason::Manual).unwrap();
        source.wait_for_calls(1).await;
        assert_eq!(source.recorded_calls(), vec![vec![SyncReason::Manual]]);

        tokio::time::advance(Duration::from_secs(59)).await;
        tokio::task::yield_now().await;
        assert_eq!(source.recorded_calls().len(), 1);

        tokio::time::advance(Duration::from_secs(1)).await;
        source.wait_for_calls(2).await;
        assert_eq!(source.recorded_calls()[1], vec![SyncReason::Interval]);

        drop(tx);
        task.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn queued_requests_are_coalesced_between_runs() {
        let source = MockSource::new(true);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let worker = SyncWorker::new(
            Arc::new(source.clone()),
            Arc::new(MockRuntime),
            Arc::new(Mutex::new(SyncStatus::Idle)),
            rx,
            Duration::from_secs(60 * 60),
            Duration::from_secs(5),
        );

        let task = tokio::spawn(worker.run());

        tx.send(SyncReason::Manual).unwrap();
        source.wait_for_calls(1).await;

        tx.send(SyncReason::Deeplink).unwrap();
        tx.send(SyncReason::AppleCalendarChanged).unwrap();
        tx.send(SyncReason::Deeplink).unwrap();
        source.release_blocked_call();
        source.wait_for_calls(2).await;

        let calls = source.recorded_calls();
        assert_eq!(calls[0], vec![SyncReason::Manual]);
        assert_eq!(
            calls[1],
            vec![SyncReason::Deeplink, SyncReason::AppleCalendarChanged]
        );

        drop(tx);
        task.abort();
    }
}
