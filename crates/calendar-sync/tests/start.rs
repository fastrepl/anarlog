use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use calendar_sync::{
    BoxError, CalendarKey, CalendarPlan, CalendarSyncHandle, CalendarSyncRuntime,
    CalendarSyncSource, CalendarSyncStore, CalendarSyncWorkerEvent, Config, EventPlan,
    IncomingSnapshot, PersistedCalendar, PersistedEvent, SyncRange, SyncStatus,
};
use tokio::sync::Notify;

#[derive(Clone, Default)]
struct RecordingRuntime {
    events: Arc<Mutex<Vec<CalendarSyncWorkerEvent>>>,
    finished: Arc<Notify>,
    failure: Arc<Mutex<Option<String>>>,
}

impl RecordingRuntime {
    fn recorded_events(&self) -> Vec<CalendarSyncWorkerEvent> {
        self.events.lock().unwrap().clone()
    }

    fn failure(&self) -> Option<String> {
        self.failure.lock().unwrap().clone()
    }

    async fn wait_for_finish(&self) {
        self.finished.notified().await;
    }
}

impl CalendarSyncRuntime for RecordingRuntime {
    fn emit(&self, event: CalendarSyncWorkerEvent) {
        match &event {
            CalendarSyncWorkerEvent::SyncFinished { .. } => self.finished.notify_waiters(),
            CalendarSyncWorkerEvent::SyncFailed { error } => {
                *self.failure.lock().unwrap() = Some(error.clone());
                self.finished.notify_waiters();
            }
            _ => {}
        }

        self.events.lock().unwrap().push(event);
    }
}

#[derive(Clone)]
struct BlockingSource {
    calls: Arc<Mutex<usize>>,
    calls_changed: Arc<Notify>,
    block_first_call: Arc<AtomicBool>,
    release_first_call: Arc<Notify>,
}

impl BlockingSource {
    fn new(block_first_call: bool) -> Self {
        Self {
            calls: Arc::new(Mutex::new(0)),
            calls_changed: Arc::new(Notify::new()),
            block_first_call: Arc::new(AtomicBool::new(block_first_call)),
            release_first_call: Arc::new(Notify::new()),
        }
    }

    async fn wait_for_calls(&self, count: usize) {
        loop {
            if *self.calls.lock().unwrap() >= count {
                return;
            }

            self.calls_changed.notified().await;
        }
    }

    fn release_blocked_call(&self) {
        self.release_first_call.notify_waiters();
    }
}

impl CalendarSyncSource for BlockingSource {
    fn fetch(
        &self,
        _range: SyncRange,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<IncomingSnapshot, BoxError>> + Send + '_>>
    {
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
    ) -> Pin<Box<dyn std::future::Future<Output = Result<bool, BoxError>> + Send + 'a>> {
        Box::pin(async move { Ok(false) })
    }
}

#[tokio::test]
async fn start_requests_sync_and_reports_lifecycle() {
    let source = BlockingSource::new(true);
    let runtime = RecordingRuntime::default();
    let handle = calendar_sync::start(
        source.clone(),
        Arc::new(MockStore),
        runtime.clone(),
        Config {
            interval: Duration::from_secs(60 * 60),
            sync_timeout: Duration::from_secs(5),
        },
    );

    assert_eq!(handle.status(), SyncStatus::Idle);

    handle.request_sync().unwrap();
    source.wait_for_calls(1).await;
    wait_for_status(&handle, SyncStatus::Running).await;

    source.release_blocked_call();
    tokio::time::timeout(Duration::from_secs(1), runtime.wait_for_finish())
        .await
        .expect("sync should finish");

    wait_for_status(&handle, SyncStatus::Idle).await;
    assert_eq!(runtime.failure(), None, "sync should not fail");

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
                CalendarSyncWorkerEvent::SyncFinished {
                    data_changed: false
                },
                CalendarSyncWorkerEvent::StatusChanged {
                    status: SyncStatus::Idle
                }
            ]
        ),
        "unexpected event sequence: {events:?}"
    );
}

async fn wait_for_status(handle: &CalendarSyncHandle, expected: SyncStatus) {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if handle.status() == expected {
                return;
            }

            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for status {expected:?}"));
}
