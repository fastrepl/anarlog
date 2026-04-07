use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use futures_util::StreamExt;
use hypr_activity_capture::{
    ActivityCapture, ActivityScreenshotCoordinator, CapturePolicy, LatestCaptureSink,
    LatestCaptureState, PlatformCapture, PolicyUpdate, ScreenCoreCapturer,
    StableSegmentScreenshotPolicy,
};
use tauri_specta::Event;

use crate::events::{ActivityCaptureErrorKind, ActivityCapturePluginEvent, ActivityCaptureSignal};

pub struct ActivityCaptureRuntime<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
    policy: Mutex<CapturePolicy>,
    running: AtomicBool,
    task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    screenshot: Mutex<ActivityScreenshotCoordinator>,
    latest_screenshot_state: Arc<LatestCaptureState>,
    screenshot_task: Mutex<ScreenshotTaskState>,
}

impl<R: tauri::Runtime> ActivityCaptureRuntime<R> {
    pub fn new(app: tauri::AppHandle<R>) -> Self {
        let latest_screenshot_state = Arc::new(LatestCaptureState::default());
        Self {
            app,
            policy: Mutex::new(CapturePolicy::default()),
            running: AtomicBool::new(false),
            task: Mutex::new(None),
            screenshot: Mutex::new(ActivityScreenshotCoordinator::new(
                Box::new(StableSegmentScreenshotPolicy::default()),
                Arc::new(LatestCaptureSink::new(Arc::clone(&latest_screenshot_state))),
                Arc::new(ScreenCoreCapturer),
            )),
            latest_screenshot_state,
            screenshot_task: Mutex::new(ScreenshotTaskState::default()),
        }
    }

    pub fn policy(&self) -> CapturePolicy {
        self.policy
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn set_policy(self: &Arc<Self>, policy: CapturePolicy) -> Result<(), crate::Error> {
        *self.policy.lock().unwrap_or_else(|e| e.into_inner()) = policy;
        if self.is_running() {
            self.restart()?;
        }
        Ok(())
    }

    pub fn reset_policy(self: &Arc<Self>) -> Result<(), crate::Error> {
        self.set_policy(CapturePolicy::default())
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn latest_screenshot(
        &self,
    ) -> Option<hypr_activity_capture::ActivityScreenshotCapture> {
        self.latest_screenshot_state.latest()
    }

    pub fn start(self: &Arc<Self>) -> Result<(), crate::Error> {
        if self.is_running() {
            return Ok(());
        }

        let capture = PlatformCapture::with_policy(self.policy());
        let mut stream = capture.watch(Default::default())?;

        self.running.store(true, Ordering::SeqCst);
        if let Some(handle) = self.task.lock().unwrap_or_else(|e| e.into_inner()).take() {
            handle.abort();
        }

        let runtime = Arc::clone(self);
        let handle = tauri::async_runtime::spawn(async move {
            while let Some(item) = stream.next().await {
                match item {
                    Ok(transition) => {
                        runtime.handle_screenshot_transition(&transition);
                        let event = ActivityCapturePluginEvent::Signal {
                            signal: ActivityCaptureSignal::from(transition),
                        };
                        if let Err(error) = event.emit(&runtime.app) {
                            tracing::error!(?error, "failed_to_emit_activity_capture_signal");
                        }
                    }
                    Err(error) => {
                        let event = ActivityCapturePluginEvent::Error {
                            kind: ActivityCaptureErrorKind::from(error.kind),
                            message: error.message,
                        };
                        if let Err(emit_error) = event.emit(&runtime.app) {
                            tracing::error!(?emit_error, "failed_to_emit_activity_capture_error");
                        }
                        break;
                    }
                }
            }

            runtime.running.store(false, Ordering::SeqCst);
        });

        *self.task.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
        Ok(())
    }

    fn restart(self: &Arc<Self>) -> Result<(), crate::Error> {
        self.stop();
        self.start()
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.task.lock().unwrap_or_else(|e| e.into_inner()).take() {
            handle.abort();
        }
        if let Some(handle) = self
            .screenshot_task
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .handle
            .take()
        {
            handle.abort();
        }
    }

    fn handle_screenshot_transition(
        self: &Arc<Self>,
        transition: &hypr_activity_capture::Transition,
    ) {
        let update = {
            let mut screenshot = self.screenshot.lock().unwrap_or_else(|e| e.into_inner());
            screenshot.handle_transition(transition, unix_ms_now())
        };

        self.apply_screenshot_update(update);
    }

    fn apply_screenshot_update(self: &Arc<Self>, update: PolicyUpdate) {
        match update {
            PolicyUpdate::None => {}
            PolicyUpdate::CancelPending => self.clear_screenshot_task(),
            PolicyUpdate::Schedule(pending) => self.replace_screenshot_task(pending),
            PolicyUpdate::CancelAndSchedule(pending) => {
                self.clear_screenshot_task();
                self.replace_screenshot_task(pending);
            }
        }
    }

    fn clear_screenshot_task(&self) {
        let mut task_state = self
            .screenshot_task
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        task_state.pending_id = None;
        if let Some(handle) = task_state.handle.take() {
            handle.abort();
        }
    }

    fn replace_screenshot_task(self: &Arc<Self>, pending: hypr_activity_capture::PendingCapture) {
        let delay_ms = pending.due_at_ms.saturating_sub(unix_ms_now()).max(0) as u64;
        let pending_id = pending.pending_id;
        let runtime = Arc::clone(self);
        let handle = tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            runtime.fire_screenshot_capture(pending_id);
        });

        let mut task_state = self
            .screenshot_task
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(existing) = task_state.handle.take() {
            existing.abort();
        }
        task_state.pending_id = Some(pending_id);
        task_state.handle = Some(handle);
    }

    fn fire_screenshot_capture(&self, pending_id: u64) {
        {
            let mut task_state = self
                .screenshot_task
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if task_state.pending_id == Some(pending_id) {
                task_state.pending_id = None;
                task_state.handle = None;
            }
        }

        let mut screenshot = self.screenshot.lock().unwrap_or_else(|e| e.into_inner());
        let captured = match screenshot.fire_pending_capture(pending_id, unix_ms_now()) {
            Ok(captured) => captured,
            Err(error) => {
                tracing::warn!(pending_id, error = %error, "activity_screenshot_capture_failed");
                return;
            }
        };

        if captured && let Some(capture) = self.latest_screenshot_state.latest() {
            tracing::info!(
                fingerprint = %capture.fingerprint,
                reason = ?capture.reason,
                pid = capture.target.pid,
                app_name = %capture.target.app_name,
                title = capture.target.title.as_deref().unwrap_or_default(),
                scheduled_at_ms = capture.scheduled_at_ms,
                captured_at_ms = capture.captured_at_ms,
                "activity_screenshot_capture_succeeded"
            );
        }
    }
}

#[derive(Default)]
struct ScreenshotTaskState {
    pending_id: Option<u64>,
    handle: Option<tauri::async_runtime::JoinHandle<()>>,
}

fn unix_ms_now() -> i64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration.as_millis().min(i64::MAX as u128) as i64,
        Err(error) => -(error.duration().as_millis().min(i64::MAX as u128) as i64),
    }
}
