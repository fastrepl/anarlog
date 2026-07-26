use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::{DetectEvent, InstalledApp};

pub(super) struct DetectorState {
    pub(super) last_state: bool,
    last_change: Instant,
    debounce_duration: Duration,
    pub(super) active_apps: Vec<InstalledApp>,
}

impl DetectorState {
    fn new() -> Self {
        Self {
            last_state: false,
            last_change: Instant::now(),
            debounce_duration: Duration::from_millis(500),
            active_apps: Vec::new(),
        }
    }

    fn should_trigger(&mut self, new_state: bool) -> bool {
        let now = Instant::now();
        if new_state == self.last_state {
            return false;
        }
        if now.duration_since(self.last_change) < self.debounce_duration {
            return false;
        }
        self.last_state = new_state;
        self.last_change = now;
        true
    }
}

pub(super) struct SharedContext {
    pub(super) callback: Arc<Mutex<crate::DetectCallback>>,
    pub(super) current_device: Arc<Mutex<Option<cidre::core_audio::Device>>>,
    pub(super) state: Arc<Mutex<DetectorState>>,
    pub(super) polling_active: Arc<AtomicBool>,
}

impl SharedContext {
    pub(super) fn new(callback: crate::DetectCallback) -> Self {
        Self {
            callback: Arc::new(Mutex::new(callback)),
            current_device: Arc::new(Mutex::new(None)),
            state: Arc::new(Mutex::new(DetectorState::new())),
            polling_active: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(super) fn clone_shared(&self) -> Self {
        Self {
            callback: self.callback.clone(),
            current_device: self.current_device.clone(),
            state: self.state.clone(),
            polling_active: self.polling_active.clone(),
        }
    }

    pub(super) fn emit(&self, event: DetectEvent) {
        tracing::info!(?event, "detected");
        if let Ok(guard) = self.callback.lock() {
            (*guard)(event);
        }
    }

    pub(super) fn handle_mic_change(&self, mic_in_use: bool) {
        let app_snapshot = if mic_in_use {
            crate::list_mic_using_apps()
        } else {
            Ok(Vec::new())
        };
        self.handle_mic_change_with_snapshot(mic_in_use, app_snapshot);
    }

    pub(super) fn seed_running_state(&self, mic_in_use: bool) {
        let app_snapshot = if mic_in_use {
            crate::list_mic_using_apps()
        } else {
            Ok(Vec::new())
        };
        self.seed_running_state_with_snapshot(mic_in_use, app_snapshot);
    }

    fn seed_running_state_with_snapshot(
        &self,
        mic_in_use: bool,
        app_snapshot: Result<Vec<InstalledApp>, crate::Error>,
    ) {
        let Ok(mut state_guard) = self.state.lock() else {
            return;
        };

        state_guard.last_state = mic_in_use;

        if !mic_in_use {
            return;
        }

        self.polling_active.store(true, Ordering::SeqCst);

        match app_snapshot {
            Ok(apps) => {
                state_guard.active_apps = apps;
            }
            Err(error) => {
                tracing::warn!(?error, "seed_mic_snapshot_failed");
            }
        }
    }

    fn handle_mic_change_with_snapshot(
        &self,
        mic_in_use: bool,
        app_snapshot: Result<Vec<InstalledApp>, crate::Error>,
    ) {
        let Ok(mut state_guard) = self.state.lock() else {
            return;
        };

        if !state_guard.should_trigger(mic_in_use) {
            return;
        }

        if mic_in_use {
            self.polling_active.store(true, Ordering::SeqCst);

            match app_snapshot {
                Ok(apps) => {
                    state_guard.active_apps = apps.clone();
                    if apps.is_empty() {
                        return;
                    }
                    drop(state_guard);
                    self.emit(DetectEvent::MicStarted(apps));
                }
                Err(error) => {
                    tracing::warn!(?error, "mic_started_snapshot_failed");
                }
            }
        } else {
            self.polling_active.store(false, Ordering::SeqCst);
            let stopped_apps = std::mem::take(&mut state_guard.active_apps);
            drop(state_guard);
            self.emit(DetectEvent::MicStopped(stopped_apps));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str) -> InstalledApp {
        InstalledApp {
            id: id.to_string(),
            name: id.to_string(),
        }
    }

    fn test_context() -> (SharedContext, Arc<Mutex<Vec<DetectEvent>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let event_sink = events.clone();
        let ctx = SharedContext::new(crate::new_callback(move |event| {
            event_sink.lock().unwrap().push(event);
        }));

        {
            let mut state = ctx.state.lock().unwrap();
            state.last_change = Instant::now() - Duration::from_secs(1);
        }

        (ctx, events)
    }

    #[test]
    fn test_seed_running_state_error_keeps_previous_apps_and_enables_polling() {
        let (ctx, events) = test_context();
        {
            let mut state = ctx.state.lock().unwrap();
            state.active_apps = vec![app("existing")];
        }

        ctx.seed_running_state_with_snapshot(
            true,
            Err(crate::Error::AudioProcessState(
                "snapshot failed".to_string(),
            )),
        );

        let state = ctx.state.lock().unwrap();
        assert!(ctx.polling_active.load(Ordering::SeqCst));
        assert_eq!(state.last_state, true);
        assert_eq!(state.active_apps.len(), 1);
        assert!(events.lock().unwrap().is_empty());
    }

    #[test]
    fn test_handle_mic_change_error_does_not_emit_or_replace_active_apps() {
        let (ctx, events) = test_context();
        {
            let mut state = ctx.state.lock().unwrap();
            state.active_apps = vec![app("existing")];
        }

        ctx.handle_mic_change_with_snapshot(
            true,
            Err(crate::Error::AudioProcessState(
                "snapshot failed".to_string(),
            )),
        );

        let state = ctx.state.lock().unwrap();
        assert!(ctx.polling_active.load(Ordering::SeqCst));
        assert_eq!(state.last_state, true);
        assert_eq!(state.active_apps.len(), 1);
        assert!(events.lock().unwrap().is_empty());
    }

    #[test]
    fn test_handle_mic_change_empty_snapshot_does_not_emit() {
        let (ctx, events) = test_context();

        ctx.handle_mic_change_with_snapshot(true, Ok(Vec::new()));

        let state = ctx.state.lock().unwrap();
        assert!(ctx.polling_active.load(Ordering::SeqCst));
        assert_eq!(state.last_state, true);
        assert!(state.active_apps.is_empty());
        assert!(events.lock().unwrap().is_empty());
    }
}
