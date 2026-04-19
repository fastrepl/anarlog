use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use futures_util::FutureExt;
use tokio::sync::mpsc::{UnboundedReceiver, error::TryRecvError};
use tokio::time::{Instant, sleep_until, timeout};

use crate::panic_utils::panic_message;
use crate::plan::{plan_calendars, plan_events};
use crate::runtime::{CalendarSyncRuntime, CalendarSyncWorkerEvent, SyncStatus};
use crate::source::{CalendarSyncSource, SyncOutcome};
use crate::store::CalendarSyncStore;
use crate::types::SyncRange;

pub(crate) struct SyncWorker<S, T, R> {
    source: Arc<S>,
    store: Arc<T>,
    runtime: Arc<R>,
    status: Arc<Mutex<SyncStatus>>,
    rx: UnboundedReceiver<()>,
    interval: Duration,
    sync_timeout: Duration,
}

impl<S, T, R> SyncWorker<S, T, R>
where
    S: CalendarSyncSource,
    T: CalendarSyncStore,
    R: CalendarSyncRuntime,
{
    pub(crate) fn new(
        source: Arc<S>,
        store: Arc<T>,
        runtime: Arc<R>,
        status: Arc<Mutex<SyncStatus>>,
        rx: UnboundedReceiver<()>,
        interval: Duration,
        sync_timeout: Duration,
    ) -> Self {
        Self {
            source,
            store,
            runtime,
            status,
            rx,
            interval,
            sync_timeout,
        }
    }

    pub(crate) async fn run(mut self) {
        tracing::info!("calendar sync worker started");
        let mut last_attempt = Instant::now();

        loop {
            let next_interval = last_attempt + self.interval;

            tokio::select! {
                biased;
                maybe_request = self.rx.recv() => {
                    let Some(()) = maybe_request else {
                        break;
                    };

                    self.prepare_run();
                    self.run_once().await;
                    last_attempt = Instant::now();
                }
                _ = sleep_until(next_interval) => {
                    self.prepare_run();
                    self.run_once().await;
                    last_attempt = Instant::now();
                }
            }
        }

        tracing::warn!("calendar sync worker stopped");
    }

    fn drain_pending(&mut self) {
        loop {
            match self.rx.try_recv() {
                Ok(()) => {}
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
    }

    fn prepare_run(&mut self) {
        self.set_status(SyncStatus::Scheduled);
        self.drain_pending();
    }

    async fn run_once(&self) {
        let outcome = AssertUnwindSafe(async {
            self.set_status(SyncStatus::Running);
            tracing::info!("calendar sync started");
            self.safe_emit(CalendarSyncWorkerEvent::SyncStarted);

            match timeout(self.sync_timeout, self.sync_once()).await {
                Ok(Ok(outcome)) => {
                    tracing::info!(
                        data_changed = outcome.data_changed,
                        "calendar sync finished"
                    );
                    self.safe_emit(CalendarSyncWorkerEvent::SyncFinished {
                        data_changed: outcome.data_changed,
                    });
                }
                Ok(Err(error)) => {
                    tracing::error!(error = %error, "calendar sync failed");
                    self.safe_emit(CalendarSyncWorkerEvent::SyncFailed {
                        error: error.to_string(),
                    });
                }
                Err(_) => {
                    tracing::error!(
                        timeout_secs = self.sync_timeout.as_secs(),
                        "calendar sync timed out"
                    );
                    self.safe_emit(CalendarSyncWorkerEvent::SyncFailed {
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
            let panic_message = panic_message(panic_payload.as_ref());
            tracing::error!(panic = %panic_message, "calendar sync panicked");
            self.safe_emit(CalendarSyncWorkerEvent::SyncFailed {
                error: format!("calendar sync panicked: {panic_message}"),
            });
        }

        self.set_status(SyncStatus::Idle);
    }

    async fn sync_once(&self) -> Result<SyncOutcome, crate::source::BoxError> {
        let range = sync_range();
        let snapshot = self.source.fetch(range).await?;
        let (calendars, events) = self.store.read().await?;
        let calendar_plan = plan_calendars(
            &calendars,
            &snapshot.calendars,
            &snapshot.requested_connections,
            &snapshot.successful_calendar_connections,
        );
        let event_plan = plan_events(
            &calendars,
            &events,
            &snapshot.events,
            &snapshot.successful_event_connections,
            &calendar_plan,
            range,
        );
        let data_changed = self.store.apply(calendar_plan, event_plan).await?;
        Ok(SyncOutcome { data_changed })
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
                panic = %panic_message(panic_payload.as_ref()),
                "calendar sync runtime emit panicked"
            );
        }
    }
}

fn sync_range() -> SyncRange {
    let now = Utc::now();
    SyncRange {
        from: now - ChronoDuration::days(7),
        to: now + ChronoDuration::days(30),
    }
}

#[cfg(test)]
mod tests {
    use std::pin::Pin;
    use std::sync::atomic::{AtomicBool, Ordering};

    use tokio::sync::Notify;

    use super::*;
    use crate::plan::{CalendarPlan, EventPlan};
    use crate::source::{BoxError, IncomingSnapshot};
    use crate::types::{CalendarKey, PersistedCalendar, PersistedEvent};

    #[derive(Clone, Default)]
    struct MockRuntime;

    impl CalendarSyncRuntime for MockRuntime {
        fn emit(&self, _event: CalendarSyncWorkerEvent) {}
    }

    #[derive(Clone, Default)]
    struct RecordingRuntime {
        events: Arc<Mutex<Vec<CalendarSyncWorkerEvent>>>,
    }

    impl RecordingRuntime {
        fn recorded_events(&self) -> Vec<CalendarSyncWorkerEvent> {
            self.events.lock().unwrap().clone()
        }
    }

    impl CalendarSyncRuntime for RecordingRuntime {
        fn emit(&self, event: CalendarSyncWorkerEvent) {
            self.events.lock().unwrap().push(event);
        }
    }

    #[derive(Clone)]
    struct MockSource {
        calls: Arc<Mutex<usize>>,
        calls_changed: Arc<Notify>,
        block_first_call: Arc<AtomicBool>,
        release_first_call: Arc<Notify>,
    }

    impl MockSource {
        fn new(block_first_call: bool) -> Self {
            Self {
                calls: Arc::new(Mutex::new(0)),
                calls_changed: Arc::new(Notify::new()),
                block_first_call: Arc::new(AtomicBool::new(block_first_call)),
                release_first_call: Arc::new(Notify::new()),
            }
        }

        fn recorded_calls(&self) -> usize {
            *self.calls.lock().unwrap()
        }

        async fn wait_for_calls(&self, count: usize) {
            loop {
                if self.recorded_calls() >= count {
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
        fn fetch(
            &self,
            _range: SyncRange,
        ) -> Pin<
            Box<dyn std::future::Future<Output = Result<IncomingSnapshot, BoxError>> + Send + '_>,
        > {
            let calls = self.calls.clone();
            let calls_changed = self.calls_changed.clone();
            let block_first_call = self.block_first_call.clone();
            let release_first_call = self.release_first_call.clone();

            Box::pin(async move {
                *calls.lock().unwrap() += 1;
                calls_changed.notify_waiters();

                if block_first_call.swap(false, Ordering::SeqCst) {
                    release_first_call.notified().await;
                }

                Ok(IncomingSnapshot::default())
            })
        }
    }

    #[derive(Clone, Default)]
    struct MockStore;

    #[derive(Clone)]
    struct TestCalendar {
        id: String,
        key: CalendarKey,
        enabled: bool,
    }

    impl PersistedCalendar for TestCalendar {
        fn id(&self) -> &str {
            &self.id
        }

        fn key(&self) -> CalendarKey {
            self.key.clone()
        }

        fn enabled(&self) -> bool {
            self.enabled
        }
    }

    #[derive(Clone)]
    struct TestEvent {
        id: String,
        tracking_id_event: Option<String>,
        calendar_id: String,
        started_at: String,
        ended_at: Option<String>,
    }

    impl PersistedEvent for TestEvent {
        fn id(&self) -> &str {
            &self.id
        }

        fn tracking_id_event(&self) -> Option<&str> {
            self.tracking_id_event.as_deref()
        }

        fn calendar_id(&self) -> &str {
            &self.calendar_id
        }

        fn started_at(&self) -> &str {
            &self.started_at
        }

        fn ended_at(&self) -> Option<&str> {
            self.ended_at.as_deref()
        }
    }

    impl CalendarSyncStore for MockStore {
        type Calendar = TestCalendar;
        type Event = TestEvent;

        fn read(
            &self,
        ) -> Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<(Vec<Self::Calendar>, Vec<Self::Event>), BoxError>,
                    > + Send
                    + '_,
            >,
        > {
            Box::pin(async move { Ok((Vec::new(), Vec::new())) })
        }

        fn apply<'a>(
            &'a self,
            _calendar_plan: CalendarPlan<'a>,
            _event_plan: EventPlan<'a>,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<bool, BoxError>> + Send + 'a>>
        {
            Box::pin(async move { Ok(false) })
        }
    }

    #[tokio::test(start_paused = true)]
    async fn manual_sync_resets_interval_timer() {
        let source = MockSource::new(false);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let worker = SyncWorker::new(
            Arc::new(source.clone()),
            Arc::new(MockStore),
            Arc::new(MockRuntime),
            Arc::new(Mutex::new(SyncStatus::Idle)),
            rx,
            Duration::from_secs(60),
            Duration::from_secs(5),
        );

        let task = tokio::spawn(worker.run());

        tokio::time::advance(Duration::from_secs(30)).await;
        tokio::task::yield_now().await;
        assert_eq!(source.recorded_calls(), 0);

        tx.send(()).unwrap();
        source.wait_for_calls(1).await;
        assert_eq!(source.recorded_calls(), 1);

        tokio::time::advance(Duration::from_secs(59)).await;
        tokio::task::yield_now().await;
        assert_eq!(source.recorded_calls(), 1);

        tokio::time::advance(Duration::from_secs(1)).await;
        source.wait_for_calls(2).await;
        assert_eq!(source.recorded_calls(), 2);

        drop(tx);
        task.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn queued_requests_are_coalesced_between_runs() {
        let source = MockSource::new(true);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let worker = SyncWorker::new(
            Arc::new(source.clone()),
            Arc::new(MockStore),
            Arc::new(MockRuntime),
            Arc::new(Mutex::new(SyncStatus::Idle)),
            rx,
            Duration::from_secs(60 * 60),
            Duration::from_secs(5),
        );

        let task = tokio::spawn(worker.run());

        tx.send(()).unwrap();
        source.wait_for_calls(1).await;

        tx.send(()).unwrap();
        tx.send(()).unwrap();
        tx.send(()).unwrap();
        source.release_blocked_call();
        source.wait_for_calls(2).await;

        assert_eq!(source.recorded_calls(), 2);

        drop(tx);
        task.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn timer_driven_sync_emits_scheduled_before_running() {
        let source = MockSource::new(false);
        let runtime = RecordingRuntime::default();
        let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let worker = SyncWorker::new(
            Arc::new(source.clone()),
            Arc::new(MockStore),
            Arc::new(runtime.clone()),
            Arc::new(Mutex::new(SyncStatus::Idle)),
            rx,
            Duration::from_secs(60),
            Duration::from_secs(5),
        );

        let task = tokio::spawn(worker.run());

        tokio::time::advance(Duration::from_secs(60)).await;
        source.wait_for_calls(1).await;

        let events = runtime.recorded_events();
        assert!(
            matches!(
                events.as_slice(),
                [
                    CalendarSyncWorkerEvent::StatusChanged {
                        status: SyncStatus::Scheduled
                    },
                    CalendarSyncWorkerEvent::StatusChanged {
                        status: SyncStatus::Running
                    },
                    CalendarSyncWorkerEvent::SyncStarted,
                    ..,
                    CalendarSyncWorkerEvent::StatusChanged {
                        status: SyncStatus::Idle
                    }
                ]
            ),
            "unexpected event sequence: {events:?}"
        );

        task.abort();
    }
}
