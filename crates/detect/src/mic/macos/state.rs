use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use crate::{DetectEvent, InstalledApp};

use super::WorkerSignal;

pub(super) struct DetectorState {
    /// Latest physical state reported by CoreAudio.
    pub(super) last_state: bool,
    emitted_state: bool,
    last_change: Instant,
    debounce_duration: Duration,
    pending_change: Option<PendingMicChange>,
    pub(super) active_apps: Vec<InstalledApp>,
}

struct PendingMicChange {
    state: bool,
    event: Option<DetectEvent>,
    emit_at: Instant,
}

impl DetectorState {
    fn new() -> Self {
        Self {
            last_state: false,
            emitted_state: false,
            last_change: Instant::now(),
            debounce_duration: Duration::from_millis(500),
            pending_change: None,
            active_apps: Vec::new(),
        }
    }

    fn record_edge(
        &mut self,
        new_state: bool,
        event: Option<DetectEvent>,
        now: Instant,
    ) -> Option<DetectEvent> {
        if new_state == self.last_state {
            return None;
        }

        self.last_state = new_state;
        if new_state == self.emitted_state {
            self.pending_change = None;
            return None;
        }

        if now.saturating_duration_since(self.last_change) < self.debounce_duration {
            self.pending_change = Some(PendingMicChange {
                state: new_state,
                event,
                emit_at: self.last_change + self.debounce_duration,
            });
            return None;
        }

        self.emitted_state = new_state;
        self.last_change = now;
        self.pending_change = None;
        event
    }

    fn flush_pending(&mut self, now: Instant) -> Option<DetectEvent> {
        let pending = self.pending_change.as_ref()?;
        if now < pending.emit_at {
            return None;
        }

        let pending = self.pending_change.take().unwrap();
        if pending.state != self.last_state || pending.state == self.emitted_state {
            return None;
        }

        self.emitted_state = pending.state;
        self.last_change = now;
        pending.event
    }

    fn seed(&mut self, state: bool, now: Instant) {
        self.last_state = state;
        self.emitted_state = state;
        self.last_change = now;
        self.pending_change = None;
    }
}

pub(super) struct SharedContext {
    pub(super) callback: Arc<Mutex<crate::DetectCallback>>,
    pub(super) current_device: Arc<Mutex<Option<cidre::core_audio::Device>>>,
    pub(super) state: Arc<Mutex<DetectorState>>,
    pub(super) polling_active: Arc<AtomicBool>,
    polling_fallback_active: Arc<AtomicBool>,
    polling_fallback_required: Arc<AtomicBool>,
    device_listener_context_retention_required: Arc<AtomicBool>,
    listener_callbacks_active: Arc<AtomicBool>,
    worker_signal: Option<mpsc::SyncSender<WorkerSignal>>,
}

impl SharedContext {
    #[cfg(test)]
    pub(super) fn new(callback: crate::DetectCallback) -> Self {
        Self::new_inner(callback, None)
    }

    pub(super) fn with_worker_signal(
        callback: crate::DetectCallback,
        worker_signal: mpsc::SyncSender<WorkerSignal>,
    ) -> Self {
        Self::new_inner(callback, Some(worker_signal))
    }

    fn new_inner(
        callback: crate::DetectCallback,
        worker_signal: Option<mpsc::SyncSender<WorkerSignal>>,
    ) -> Self {
        Self {
            callback: Arc::new(Mutex::new(callback)),
            current_device: Arc::new(Mutex::new(None)),
            state: Arc::new(Mutex::new(DetectorState::new())),
            polling_active: Arc::new(AtomicBool::new(false)),
            polling_fallback_active: Arc::new(AtomicBool::new(false)),
            polling_fallback_required: Arc::new(AtomicBool::new(false)),
            device_listener_context_retention_required: Arc::new(AtomicBool::new(false)),
            listener_callbacks_active: Arc::new(AtomicBool::new(true)),
            worker_signal,
        }
    }

    pub(super) fn clone_shared(&self) -> Self {
        Self {
            callback: self.callback.clone(),
            current_device: self.current_device.clone(),
            state: self.state.clone(),
            polling_active: self.polling_active.clone(),
            polling_fallback_active: self.polling_fallback_active.clone(),
            polling_fallback_required: self.polling_fallback_required.clone(),
            device_listener_context_retention_required: self
                .device_listener_context_retention_required
                .clone(),
            listener_callbacks_active: self.listener_callbacks_active.clone(),
            worker_signal: self.worker_signal.clone(),
        }
    }

    fn wake_worker(&self) {
        if let Some(worker_signal) = &self.worker_signal {
            let _ = worker_signal.try_send(WorkerSignal::Wake);
        }
    }

    pub(super) fn deactivate_listener_callbacks(&self) {
        self.listener_callbacks_active
            .store(false, Ordering::SeqCst);
        self.polling_active.store(false, Ordering::SeqCst);
        self.polling_fallback_active.store(false, Ordering::SeqCst);
        self.polling_fallback_required
            .store(false, Ordering::SeqCst);
    }

    pub(super) fn listener_callbacks_active(&self) -> bool {
        self.listener_callbacks_active.load(Ordering::SeqCst)
    }

    pub(super) fn enable_polling_fallback(&self) {
        self.polling_fallback_active.store(true, Ordering::SeqCst);
        self.wake_worker();
    }

    pub(super) fn require_polling_fallback(&self) {
        self.polling_fallback_required.store(true, Ordering::SeqCst);
        self.enable_polling_fallback();
    }

    pub(super) fn disable_polling_fallback(&self) {
        if !self.polling_fallback_required.load(Ordering::SeqCst) {
            self.polling_fallback_active.store(false, Ordering::SeqCst);
            self.wake_worker();
        }
    }

    pub(super) fn polling_fallback_active(&self) -> bool {
        self.polling_fallback_active.load(Ordering::SeqCst)
    }

    pub(super) fn app_polling_active(&self) -> bool {
        self.polling_active.load(Ordering::SeqCst)
            || self.polling_fallback_active.load(Ordering::SeqCst)
    }

    pub(super) fn require_device_listener_context_retention(&self) {
        self.device_listener_context_retention_required
            .store(true, Ordering::SeqCst);
    }

    pub(super) fn device_listener_context_retention_required(&self) -> bool {
        self.device_listener_context_retention_required
            .load(Ordering::SeqCst)
    }

    pub(super) fn emit(&self, event: DetectEvent) {
        tracing::info!(?event, "detected");
        if let Ok(guard) = self.callback.lock() {
            (*guard)(event);
        }
    }

    pub(super) fn handle_mic_change(&self, mic_in_use: bool) {
        if !self.listener_callbacks_active() {
            return;
        }

        let app_snapshot = if mic_in_use {
            crate::list_mic_using_apps()
        } else {
            Ok(Vec::new())
        };
        self.handle_mic_change_with_snapshot(mic_in_use, app_snapshot);
    }

    pub(super) fn sync_polling_fallback_state(
        &self,
        mic_in_use: bool,
        current_apps: Vec<InstalledApp>,
    ) -> bool {
        let should_sync = self
            .state
            .lock()
            .is_ok_and(|state| state.last_state != mic_in_use);
        if should_sync {
            self.handle_mic_change_with_snapshot(mic_in_use, Ok(current_apps));
        }
        should_sync
    }

    pub(super) fn flush_pending_mic_change(&self) {
        self.flush_pending_mic_change_at(Instant::now());
    }

    pub(super) fn next_pending_delay(&self, now: Instant) -> Option<Duration> {
        self.state.lock().ok().and_then(|state| {
            state
                .pending_change
                .as_ref()
                .map(|pending| pending.emit_at.saturating_duration_since(now))
        })
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

        state_guard.seed(mic_in_use, Instant::now());
        self.polling_active.store(mic_in_use, Ordering::SeqCst);
        self.wake_worker();

        if !mic_in_use {
            state_guard.active_apps.clear();
            return;
        }

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
        self.handle_mic_change_with_snapshot_at(mic_in_use, app_snapshot, Instant::now());
    }

    fn handle_mic_change_with_snapshot_at(
        &self,
        mic_in_use: bool,
        app_snapshot: Result<Vec<InstalledApp>, crate::Error>,
        now: Instant,
    ) {
        let Ok(mut state_guard) = self.state.lock() else {
            return;
        };

        if mic_in_use == state_guard.last_state {
            return;
        }

        let event = if mic_in_use {
            self.polling_active.store(true, Ordering::SeqCst);

            match app_snapshot {
                Ok(apps) => {
                    state_guard.active_apps = apps.clone();
                    if apps.is_empty() {
                        None
                    } else {
                        Some(DetectEvent::MicStarted(apps))
                    }
                }
                Err(error) => {
                    tracing::warn!(?error, "mic_started_snapshot_failed");
                    None
                }
            }
        } else {
            self.polling_active.store(false, Ordering::SeqCst);
            let stopped_apps = std::mem::take(&mut state_guard.active_apps);
            Some(DetectEvent::MicStopped(stopped_apps))
        };

        let event = state_guard.record_edge(mic_in_use, event, now);
        drop(state_guard);
        self.wake_worker();
        if let Some(event) = event {
            self.emit(event);
        }
    }

    fn flush_pending_mic_change_at(&self, now: Instant) {
        let event = self
            .state
            .lock()
            .ok()
            .and_then(|mut state| state.flush_pending(now));
        if let Some(event) = event {
            self.emit(event);
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

    #[test]
    fn rapid_stop_updates_state_immediately_and_emits_on_trailing_edge() {
        let (ctx, events) = test_context();
        let started_at = Instant::now();
        {
            let mut state = ctx.state.lock().unwrap();
            state.last_change = started_at - Duration::from_secs(1);
        }

        ctx.handle_mic_change_with_snapshot_at(true, Ok(vec![app("recorder")]), started_at);
        ctx.handle_mic_change_with_snapshot_at(
            false,
            Ok(Vec::new()),
            started_at + Duration::from_millis(100),
        );

        {
            let state = ctx.state.lock().unwrap();
            assert!(!state.last_state);
            assert!(state.pending_change.is_some());
        }
        assert!(!ctx.polling_active.load(Ordering::SeqCst));
        assert_eq!(events.lock().unwrap().len(), 1);

        ctx.flush_pending_mic_change_at(started_at + Duration::from_millis(499));
        assert_eq!(events.lock().unwrap().len(), 1);

        ctx.flush_pending_mic_change_at(started_at + Duration::from_millis(500));
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert!(
            matches!(&events[0], DetectEvent::MicStarted(apps) if apps.len() == 1 && apps[0].id == "recorder")
        );
        assert!(
            matches!(&events[1], DetectEvent::MicStopped(apps) if apps.len() == 1 && apps[0].id == "recorder")
        );
    }

    #[test]
    fn rapid_bounce_back_cancels_pending_trailing_edge() {
        let (ctx, events) = test_context();
        let started_at = Instant::now();
        {
            let mut state = ctx.state.lock().unwrap();
            state.last_change = started_at - Duration::from_secs(1);
        }

        ctx.handle_mic_change_with_snapshot_at(true, Ok(vec![app("recorder")]), started_at);
        ctx.handle_mic_change_with_snapshot_at(
            false,
            Ok(Vec::new()),
            started_at + Duration::from_millis(100),
        );
        ctx.handle_mic_change_with_snapshot_at(
            true,
            Ok(vec![app("recorder")]),
            started_at + Duration::from_millis(200),
        );
        ctx.flush_pending_mic_change_at(started_at + Duration::from_secs(1));

        let state = ctx.state.lock().unwrap();
        assert!(state.last_state);
        assert!(state.pending_change.is_none());
        assert!(ctx.polling_active.load(Ordering::SeqCst));
        drop(state);
        assert_eq!(events.lock().unwrap().len(), 1);
    }

    #[test]
    fn startup_edge_inside_debounce_window_is_emitted_at_deadline() {
        let (ctx, events) = test_context();
        let seeded_at = Instant::now();
        {
            let mut state = ctx.state.lock().unwrap();
            state.seed(false, seeded_at);
        }

        ctx.handle_mic_change_with_snapshot_at(
            true,
            Ok(vec![app("recorder")]),
            seeded_at + Duration::from_millis(100),
        );
        assert!(events.lock().unwrap().is_empty());

        ctx.flush_pending_mic_change_at(seeded_at + Duration::from_millis(500));

        assert_eq!(events.lock().unwrap().len(), 1);
    }

    #[test]
    fn deactivated_listener_context_ignores_late_callbacks() {
        let (ctx, events) = test_context();

        ctx.enable_polling_fallback();
        ctx.deactivate_listener_callbacks();
        ctx.handle_mic_change(true);

        assert!(!ctx.app_polling_active());
        assert!(!ctx.state.lock().unwrap().last_state);
        assert!(events.lock().unwrap().is_empty());
    }

    #[test]
    fn polling_fallback_keeps_app_polling_active_without_a_running_mic() {
        let (ctx, _) = test_context();

        assert!(!ctx.app_polling_active());
        ctx.enable_polling_fallback();
        assert!(ctx.app_polling_active());
        ctx.disable_polling_fallback();
        assert!(!ctx.app_polling_active());
    }

    #[test]
    fn required_polling_fallback_survives_healthy_device_callbacks() {
        let (ctx, _) = test_context();

        ctx.require_polling_fallback();
        ctx.disable_polling_fallback();

        assert!(ctx.polling_fallback_active());
    }

    #[test]
    fn polling_fallback_updates_coreaudio_state_without_duplicate_recovery_edge() {
        let (ctx, events) = test_context();

        ctx.enable_polling_fallback();
        assert!(ctx.sync_polling_fallback_state(true, vec![app("recorder")]));
        ctx.disable_polling_fallback();
        ctx.handle_mic_change_with_snapshot(true, Ok(vec![app("recorder")]));

        let state = ctx.state.lock().unwrap();
        assert!(state.last_state);
        assert_eq!(state.active_apps.len(), 1);
        assert_eq!(state.active_apps[0].id, "recorder");
        drop(state);
        assert_eq!(events.lock().unwrap().len(), 1);
    }

    #[test]
    fn polling_fallback_preserves_running_state_while_apps_are_not_visible() {
        let (ctx, events) = test_context();

        ctx.handle_mic_change_with_snapshot(true, Ok(Vec::new()));

        assert!(!ctx.sync_polling_fallback_state(true, Vec::new()));
        assert!(ctx.state.lock().unwrap().last_state);
        assert!(events.lock().unwrap().is_empty());
    }

    #[test]
    fn device_listener_context_retention_is_shared() {
        let (ctx, _) = test_context();
        let cloned_ctx = ctx.clone_shared();

        cloned_ctx.require_device_listener_context_retention();

        assert!(ctx.device_listener_context_retention_required());
    }
}
