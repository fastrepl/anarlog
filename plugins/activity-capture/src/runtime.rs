use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use futures_util::StreamExt;
use hypr_activity_capture::{ActivityCapture, CapturePolicy, PlatformCapture};
use tauri_specta::Event;

use crate::events::{ActivityCaptureErrorKind, ActivityCapturePluginEvent, ActivityCaptureSignal};

pub struct ActivityCaptureRuntime<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
    policy: Mutex<CapturePolicy>,
    running: AtomicBool,
    task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

impl<R: tauri::Runtime> ActivityCaptureRuntime<R> {
    pub fn new(app: tauri::AppHandle<R>) -> Self {
        Self {
            app,
            policy: Mutex::new(CapturePolicy::default()),
            running: AtomicBool::new(false),
            task: Mutex::new(None),
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
    }
}
