use std::sync::Arc;
use std::time::{Duration, Instant};

use hypr_transcription_core::listener2 as core;
use tauri_specta::Event;
use tokio::task::JoinHandle;

use crate::{
    BatchSessionControl, BatchSessionEntry, BatchSessionRegistry, BatchTerminalState,
    TranscriptionEvent, TranscriptionParams,
};

const BATCH_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

pub struct Listener2<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Listener2<'a, R, M> {
    pub async fn start_transcription(
        &self,
        params: TranscriptionParams,
    ) -> Result<(), core::Error> {
        let state = self.manager.state::<crate::SharedState>();
        let guard = state.lock().await;
        let app = guard.app.clone();
        drop(guard);

        let registry = self
            .manager
            .state::<Arc<BatchSessionRegistry>>()
            .inner()
            .clone();
        let session_id = params.session_id.clone();

        {
            let mut sessions = registry
                .sessions
                .lock()
                .expect("batch session registry poisoned");
            if let Some(entry) = sessions.get(&session_id) {
                let state = *entry
                    .control
                    .terminal_state
                    .lock()
                    .expect("batch terminal state poisoned");
                if state == BatchTerminalState::Running {
                    return Err(core::Error::BatchError(
                        "session already running".to_string(),
                    ));
                }

                sessions.remove(&session_id);
            }
        }

        let (last_activity_tx, _) = tokio::sync::watch::channel(Instant::now());
        let control = Arc::new(BatchSessionControl {
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            last_activity_tx,
            terminal_state: std::sync::Mutex::new(BatchTerminalState::Running),
        });

        {
            let mut sessions = registry
                .sessions
                .lock()
                .expect("batch session registry poisoned");
            sessions.insert(
                session_id.clone(),
                BatchSessionEntry {
                    control: control.clone(),
                    abort_handle: None,
                },
            );
        }

        let runtime = Arc::new(TauriBatchRuntime {
            app: app.clone(),
            control: control.clone(),
        });

        let task = tokio::spawn({
            let runtime = runtime.clone();
            let registry = registry.clone();
            let control = control.clone();
            let session_id = session_id.clone();
            async move {
                let _ = core::run_batch(runtime, params.into()).await;
                finish_batch_session(&registry, &session_id, &control);
            }
        });
        let abort_handle = task.abort_handle();

        let is_running = {
            let mut sessions = registry
                .sessions
                .lock()
                .expect("batch session registry poisoned");
            let Some(entry) = sessions.get_mut(&session_id) else {
                abort_handle.abort();
                return Ok(());
            };

            if !Arc::ptr_eq(&entry.control, &control) {
                abort_handle.abort();
                return Err(core::Error::BatchError(
                    "session already running".to_string(),
                ));
            }

            entry.abort_handle = Some(abort_handle.clone());

            *control
                .terminal_state
                .lock()
                .expect("batch terminal state poisoned")
                == BatchTerminalState::Running
        };

        if !is_running {
            remove_batch_session(&registry, &session_id, &control);
            return Ok(());
        }

        spawn_idle_timeout_monitor(app, registry, session_id, control, abort_handle);

        Ok(())
    }

    pub async fn stop_transcription(&self, session_id: String) {
        let state = self.manager.state::<crate::SharedState>();
        let guard = state.lock().await;
        let app = guard.app.clone();
        drop(guard);

        let registry = self
            .manager
            .state::<Arc<BatchSessionRegistry>>()
            .inner()
            .clone();
        stop_batch_session(&app, &registry, &session_id);
    }

    pub async fn run_denoise(&self, params: core::DenoiseParams) -> Result<(), core::Error> {
        let state = self.manager.state::<crate::SharedState>();
        let guard = state.lock().await;
        let app = guard.app.clone();
        drop(guard);

        let runtime = Arc::new(TauriDenoiseRuntime { app });
        core::run_denoise(runtime, params).await
    }

    pub fn parse_subtitle(&self, path: String) -> Result<core::Subtitle, String> {
        core::parse_subtitle_from_path(path)
    }

    pub fn export_to_vtt(
        &self,
        session_id: String,
        words: Vec<core::VttWord>,
    ) -> Result<String, String> {
        use tauri_plugin_settings::SettingsPluginExt;

        let base = self
            .manager
            .settings()
            .vault_base()
            .map_err(|e| e.to_string())?;
        let session_dir = base.join("sessions").join(&session_id);

        std::fs::create_dir_all(&session_dir).map_err(|e| e.to_string())?;

        let vtt_path = session_dir.join("transcript.vtt");

        core::export_words_to_vtt_file(words, &vtt_path)?;
        Ok(vtt_path.to_string())
    }
}

pub trait Listener2PluginExt<R: tauri::Runtime> {
    fn listener2(&self) -> Listener2<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> Listener2PluginExt<R> for T {
    fn listener2(&self) -> Listener2<'_, R, Self>
    where
        Self: Sized,
    {
        Listener2 {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}

struct TauriBatchRuntime {
    app: tauri::AppHandle,
    control: Arc<BatchSessionControl>,
}

impl core::BatchRuntime for TauriBatchRuntime {
    fn emit(&self, event: core::BatchEvent) {
        if !should_emit_event(&self.control, &event) {
            return;
        }

        if matches!(
            event,
            core::BatchEvent::BatchResponseStreamed { .. } | core::BatchEvent::BatchResponse { .. }
        ) {
            let _ = self.control.last_activity_tx.send(Instant::now());
        }

        if let core::BatchEvent::BatchCompleted { .. } = event {
            return;
        }
        let _ = TranscriptionEvent::from(event).emit(&self.app);
    }
}

struct TauriDenoiseRuntime {
    app: tauri::AppHandle,
}

impl core::DenoiseRuntime for TauriDenoiseRuntime {
    fn emit(&self, event: core::DenoiseEvent) {
        let _ = event.emit(&self.app);
    }
}

fn should_emit_event(control: &BatchSessionControl, event: &core::BatchEvent) -> bool {
    let state = *control
        .terminal_state
        .lock()
        .expect("batch terminal state poisoned");
    state == BatchTerminalState::Running
        || matches!(
            (state, event),
            (
                BatchTerminalState::Finished,
                core::BatchEvent::BatchResponse { .. }
            )
        )
}

fn mark_terminal_state(control: &BatchSessionControl, next: BatchTerminalState) -> bool {
    let mut state = control
        .terminal_state
        .lock()
        .expect("batch terminal state poisoned");
    if *state != BatchTerminalState::Running {
        return false;
    }
    *state = next;
    control.cancellation_token.cancel();
    true
}

fn finish_batch_session(
    registry: &Arc<BatchSessionRegistry>,
    session_id: &str,
    control: &Arc<BatchSessionControl>,
) {
    {
        let mut state = control
            .terminal_state
            .lock()
            .expect("batch terminal state poisoned");
        if *state == BatchTerminalState::Running {
            *state = BatchTerminalState::Finished;
            control.cancellation_token.cancel();
        }
    }

    remove_batch_session(registry, session_id, control);
}

fn remove_batch_session(
    registry: &Arc<BatchSessionRegistry>,
    session_id: &str,
    control: &Arc<BatchSessionControl>,
) {
    let mut sessions = registry
        .sessions
        .lock()
        .expect("batch session registry poisoned");
    let should_remove = sessions
        .get(session_id)
        .is_some_and(|entry| Arc::ptr_eq(&entry.control, control));
    if should_remove {
        sessions.remove(session_id);
    }
}

fn stop_batch_session(
    app: &tauri::AppHandle,
    registry: &Arc<BatchSessionRegistry>,
    session_id: &str,
) {
    let entry = {
        let mut sessions = registry
            .sessions
            .lock()
            .expect("batch session registry poisoned");
        sessions.remove(session_id)
    };

    let Some(entry) = entry else {
        return;
    };

    if mark_terminal_state(&entry.control, BatchTerminalState::Stopped) {
        let _ = TranscriptionEvent::Stopped {
            session_id: session_id.to_string(),
        }
        .emit(app);
    }

    if let Some(abort_handle) = entry.abort_handle {
        abort_handle.abort();
    }
}

fn spawn_idle_timeout_monitor(
    app: tauri::AppHandle,
    registry: Arc<BatchSessionRegistry>,
    session_id: String,
    control: Arc<BatchSessionControl>,
    abort_handle: tokio::task::AbortHandle,
) -> JoinHandle<()> {
    let mut activity_rx = control.last_activity_tx.subscribe();

    tokio::spawn(async move {
        loop {
            let deadline = *activity_rx.borrow() + BATCH_IDLE_TIMEOUT;
            let sleep = tokio::time::sleep_until(tokio::time::Instant::from_std(deadline));
            tokio::pin!(sleep);

            tokio::select! {
                _ = control.cancellation_token.cancelled() => return,
                _ = &mut sleep => {
                    if !mark_terminal_state(&control, BatchTerminalState::TimedOut) {
                        return;
                    }

                    remove_batch_session(&registry, &session_id, &control);
                    let _ = TranscriptionEvent::Failed {
                        session_id: session_id.clone(),
                        code: core::BatchErrorCode::TimedOut,
                        error: "Transcription timed out after 60 seconds without progress.".to_string(),
                    }
                    .emit(&app);
                    abort_handle.abort();
                    return;
                }
                changed = activity_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_control() -> Arc<BatchSessionControl> {
        let (last_activity_tx, _) = tokio::sync::watch::channel(Instant::now());
        Arc::new(BatchSessionControl {
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            last_activity_tx,
            terminal_state: std::sync::Mutex::new(BatchTerminalState::Running),
        })
    }

    #[test]
    fn mark_terminal_state_only_transitions_once() {
        let control = make_control();

        assert!(mark_terminal_state(&control, BatchTerminalState::Stopped));
        assert!(!mark_terminal_state(&control, BatchTerminalState::TimedOut));
        assert_eq!(
            *control
                .terminal_state
                .lock()
                .expect("batch terminal state poisoned"),
            BatchTerminalState::Stopped,
        );
    }

    #[test]
    fn should_emit_event_stops_after_terminal_transition() {
        let control = make_control();
        let event = core::BatchEvent::BatchStarted {
            session_id: "session-1".to_string(),
        };

        assert!(should_emit_event(&control, &event));
        assert!(mark_terminal_state(&control, BatchTerminalState::Stopped));
        assert!(!should_emit_event(&control, &event));
    }

    #[test]
    fn finish_batch_session_removes_matching_registry_entry() {
        let control = make_control();
        let registry = Arc::new(BatchSessionRegistry {
            sessions: std::sync::Mutex::new(std::collections::HashMap::from([(
                "session-1".to_string(),
                BatchSessionEntry {
                    control: control.clone(),
                    abort_handle: None,
                },
            )])),
        });

        finish_batch_session(&registry, "session-1", &control);

        assert!(
            !registry
                .sessions
                .lock()
                .expect("batch session registry poisoned")
                .contains_key("session-1")
        );
    }
}
