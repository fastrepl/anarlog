mod app;
mod device;
mod state;

use cidre::core_audio as ca;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{BackgroundTask, DetectEvent};

use self::state::DetectorState;

pub struct Detector {
    background: BackgroundTask,
}

impl Default for Detector {
    fn default() -> Self {
        Self {
            background: BackgroundTask::default(),
        }
    }
}

struct SharedContext {
    callback: Arc<Mutex<crate::DetectCallback>>,
    current_device: Arc<Mutex<Option<ca::Device>>>,
    state: Arc<Mutex<DetectorState>>,
    polling_active: Arc<AtomicBool>,
}

impl SharedContext {
    fn new(callback: crate::DetectCallback) -> Self {
        Self {
            callback: Arc::new(Mutex::new(callback)),
            current_device: Arc::new(Mutex::new(None)),
            state: Arc::new(Mutex::new(DetectorState::new())),
            polling_active: Arc::new(AtomicBool::new(false)),
        }
    }

    fn clone_shared(&self) -> Self {
        Self {
            callback: self.callback.clone(),
            current_device: self.current_device.clone(),
            state: self.state.clone(),
            polling_active: self.polling_active.clone(),
        }
    }

    fn emit(&self, event: DetectEvent) {
        tracing::info!(?event, "detected");
        if let Ok(guard) = self.callback.lock() {
            (*guard)(event);
        }
    }
}

impl crate::Observer for Detector {
    fn start(&mut self, f: crate::DetectCallback) {
        self.background.start(|running, mut rx| async move {
            let (tx, mut notify_rx) = tokio::sync::mpsc::channel(1);

            std::thread::spawn(move || {
                let ctx = SharedContext::new(f);

                app::spawn(ctx.clone_shared());
                device::start(ctx, tx);
            });

            let _ = notify_rx.recv().await;

            loop {
                tokio::select! {
                    _ = &mut rx => break,
                    _ = tokio::time::sleep(Duration::from_millis(500)) => {
                        if !running.load(std::sync::atomic::Ordering::SeqCst) {
                            break;
                        }
                    }
                }
            }
        });
    }

    fn stop(&mut self) {
        self.background.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::state::DetectorState;
    use super::*;

    #[test]
    fn observe_device_stop_uses_stop_grace() {
        let mut state = DetectorState::new();
        state.last_state = true;

        let outcome = state.observe(false);
        let pending = outcome.pending.expect("expected pending transition");

        assert!(!outcome.cancelled_stop);
        assert!(!pending.expected_state);
        assert_eq!(pending.generation, 1);
        assert_eq!(pending.delay, state::STOP_GRACE);
    }

    #[test]
    fn observe_same_state_cancels_pending_transition() {
        let mut state = DetectorState::new();
        state.last_state = true;

        let pending = state
            .observe(false)
            .pending
            .expect("expected pending transition");
        let recovery = state.observe(true);
        assert!(recovery.pending.is_none());
        assert!(recovery.cancelled_stop);
        assert!(!state.commit(false, pending.generation));
        assert!(state.last_state);
    }

    #[test]
    fn cancel_pending_clears_pending_device_stop() {
        let mut state = DetectorState::new();
        state.last_state = true;

        let pending = state
            .observe(false)
            .pending
            .expect("expected pending transition");

        assert!(state.cancel_pending());
        assert!(!state.commit(false, pending.generation));
        assert!(state.last_state);
    }

    #[test]
    fn commit_updates_state_only_for_latest_generation() {
        let mut state = DetectorState::new();
        state.last_state = true;

        let stale = state
            .observe(false)
            .pending
            .expect("expected pending transition");
        let latest = state
            .observe(false)
            .pending
            .expect("expected pending transition");

        assert!(!state.commit(false, stale.generation));
        assert!(state.commit(false, latest.generation));
        assert!(!state.last_state);
    }
}
