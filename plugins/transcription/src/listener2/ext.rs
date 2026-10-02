use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, MutexGuard};
use std::time::{Duration, Instant};

use anlg_transcription_core::listener2 as core;
use tauri_specta::Event;
use tokio::task::JoinHandle;

use crate::{
    BatchSessionControl, BatchSessionEntry, BatchSessionRegistry, BatchTerminalState,
    CompletedBatchEntry, TranscriptionEvent, TranscriptionParams,
};

const BATCH_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_ACTIVE_BATCH_SESSIONS: usize = 4;
const COMPLETED_BATCH_RETENTION_MS: i64 = 7 * 24 * 60 * 60 * 1000;

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
        let file_path = params.file_path.clone();
        let provider = params.provider.clone();
        let model = params.model.clone();
        let resume_context = params.resume_context.clone();
        let started_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as i64)
            .unwrap_or_default();
        let idle_timeout = batch_idle_timeout(&params);
        let wait_for_native_completion = matches!(
            &params.provider,
            core::BatchProvider::Soniqo | core::BatchProvider::AppleSpeech
        );

        let (last_activity_tx, _) = tokio::sync::watch::channel(Instant::now());
        let control = Arc::new(BatchSessionControl {
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            last_activity_tx,
            terminal_state: std::sync::Mutex::new(BatchTerminalState::Running),
        });

        reserve_batch_session(
            &registry,
            &session_id,
            control.clone(),
            wait_for_native_completion,
        )?;

        discard_completed_batch(&registry, &session_id);

        let runtime = Arc::new(TauriBatchRuntime {
            app: app.clone(),
            control: control.clone(),
            registry: registry.clone(),
            session: crate::TranscriptionSession {
                session_id: session_id.clone(),
                file_path: file_path.clone(),
                provider: Some(provider.clone()),
                model: model.clone(),
                started_at_ms,
                resume_context: resume_context.clone(),
                completed: true,
            },
        });

        let task = tokio::spawn({
            let runtime = runtime.clone();
            let registry = registry.clone();
            let control = control.clone();
            let session_id = session_id.clone();
            let app = app.clone();
            async move {
                let mut batch_params: core::BatchParams = params.into();
                batch_params.known_speakers =
                    crate::voiceprint::known_speakers_for_session(&app, &session_id).await;
                let _ = core::run_batch(runtime, batch_params).await;
                finish_batch_session(&registry, &session_id, &control);
            }
        });
        let abort_handle = task.abort_handle();

        let is_running = {
            let mut sessions = match lock_batch_sessions(&registry) {
                Ok(sessions) => sessions,
                Err(error) => {
                    abort_handle.abort();
                    return Err(error);
                }
            };
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
            entry.file_path = file_path;
            entry.provider = Some(provider);
            entry.model = model;
            entry.started_at_ms = started_at_ms;
            entry.resume_context = resume_context;

            match lock_terminal_state(&control) {
                Ok(state) => *state == BatchTerminalState::Running,
                Err(error) => {
                    sessions.remove(&session_id);
                    abort_handle.abort();
                    return Err(error);
                }
            }
        };

        if !is_running {
            if !wait_for_native_completion {
                remove_batch_session(&registry, &session_id, &control);
            }
            return Ok(());
        }

        if let Some(idle_timeout) = idle_timeout {
            spawn_idle_timeout_monitor(
                app,
                registry,
                session_id,
                control,
                abort_handle,
                idle_timeout,
            );
        }

        Ok(())
    }

    pub fn list_transcription_sessions(
        &self,
    ) -> Result<Vec<crate::TranscriptionSession>, core::Error> {
        let registry = self.manager.state::<Arc<BatchSessionRegistry>>();
        let mut sessions = {
            let running = lock_batch_sessions(&registry)?;
            running_batch_sessions(&running)
        };
        let mut completed = lock_completed_batches(&registry)?;
        prune_completed_batches(&registry, &mut completed, now_ms());
        let completed_sessions: Vec<_> = completed
            .values()
            .filter(|entry| {
                !sessions
                    .iter()
                    .any(|session| session.session_id == entry.session.session_id)
            })
            .map(|entry| entry.session.clone())
            .collect();
        sessions.extend(completed_sessions);
        sessions.sort_by(|a, b| a.session_id.cmp(&b.session_id));
        Ok(sessions)
    }

    pub fn get_completed_transcription(
        &self,
        session_id: String,
    ) -> Result<Option<crate::CompletedTranscription>, core::Error> {
        let registry = self.manager.state::<Arc<BatchSessionRegistry>>();
        let mut completed = lock_completed_batches(&registry)?;
        prune_completed_batches(&registry, &mut completed, now_ms());
        Ok(completed
            .get(&session_id)
            .map(|entry| crate::CompletedTranscription {
                session_id: session_id.clone(),
                response: entry.response.clone(),
            }))
    }

    pub fn acknowledge_completed_transcription(&self, session_id: String) {
        let registry = self.manager.state::<Arc<BatchSessionRegistry>>();
        discard_completed_batch(&registry, &session_id);
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
    registry: Arc<BatchSessionRegistry>,
    session: crate::TranscriptionSession,
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
        if let core::BatchEvent::BatchResponse { response, .. } = &event {
            store_completed_batch(&self.registry, self.session.clone(), response.clone());
        }
        let _ = TranscriptionEvent::from(event).emit(&self.app);
    }

    fn is_cancelled(&self) -> bool {
        self.control.cancellation_token.is_cancelled()
    }
}

fn batch_lock_poisoned(name: &'static str) -> core::Error {
    tracing::error!(lock = name, "batch_session_lock_poisoned");
    core::Error::BatchError(format!("{name} poisoned"))
}

fn lock_batch_sessions(
    registry: &BatchSessionRegistry,
) -> Result<MutexGuard<'_, HashMap<String, BatchSessionEntry>>, core::Error> {
    registry
        .sessions
        .lock()
        .map_err(|_| batch_lock_poisoned("batch session registry"))
}

fn lock_completed_batches(
    registry: &BatchSessionRegistry,
) -> Result<MutexGuard<'_, HashMap<String, CompletedBatchEntry>>, core::Error> {
    registry
        .completed
        .lock()
        .map_err(|_| batch_lock_poisoned("completed batch registry"))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

fn prune_completed_batches(
    registry: &BatchSessionRegistry,
    completed: &mut HashMap<String, CompletedBatchEntry>,
    now_ms: i64,
) {
    completed.retain(|session_id, entry| {
        let keep = now_ms - entry.completed_at_ms < COMPLETED_BATCH_RETENTION_MS;
        if !keep {
            remove_completed_batch_file(registry, session_id);
        }
        keep
    });
}

fn completed_batch_path(dir: &Path, session_id: &str) -> PathBuf {
    let name: String = session_id.bytes().map(|b| format!("{b:02x}")).collect();
    dir.join(format!("{name}.json"))
}

fn write_completed_batch_file(registry: &BatchSessionRegistry, entry: &CompletedBatchEntry) {
    let Some(dir) = &registry.completed_dir else {
        return;
    };
    let result = (|| -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let path = completed_batch_path(dir, &entry.session.session_id);
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec(entry)?)?;
        std::fs::rename(&tmp, &path)
    })();
    if let Err(error) = result {
        tracing::warn!(?error, "failed_to_persist_completed_batch");
    }
}

fn remove_completed_batch_file(registry: &BatchSessionRegistry, session_id: &str) {
    let Some(dir) = &registry.completed_dir else {
        return;
    };
    match std::fs::remove_file(completed_batch_path(dir, session_id)) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => tracing::warn!(?error, "failed_to_remove_completed_batch"),
    }
}

pub fn load_completed_batches(registry: &BatchSessionRegistry) {
    let Some(dir) = &registry.completed_dir else {
        return;
    };
    let Ok(files) = std::fs::read_dir(dir) else {
        return;
    };
    let Ok(mut completed) = lock_completed_batches(registry) else {
        return;
    };
    for file in files.flatten() {
        let path = file.path();
        let entry = if path.extension().is_some_and(|ext| ext == "json") {
            std::fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<CompletedBatchEntry>(&bytes).ok())
        } else {
            None
        };
        match entry {
            Some(entry) => {
                completed.insert(entry.session.session_id.clone(), entry);
            }
            None => {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    prune_completed_batches(registry, &mut completed, now_ms());
}

fn store_completed_batch(
    registry: &BatchSessionRegistry,
    session: crate::TranscriptionSession,
    response: owhisper_interface::batch::Response,
) {
    let Ok(mut completed) = lock_completed_batches(registry) else {
        return;
    };
    let now = now_ms();
    prune_completed_batches(registry, &mut completed, now);
    let entry = CompletedBatchEntry {
        session,
        response,
        completed_at_ms: now,
    };
    write_completed_batch_file(registry, &entry);
    completed.insert(entry.session.session_id.clone(), entry);
}

fn discard_completed_batch(registry: &BatchSessionRegistry, session_id: &str) {
    if let Ok(mut completed) = lock_completed_batches(registry) {
        completed.remove(session_id);
    }
    remove_completed_batch_file(registry, session_id);
}

fn lock_terminal_state(
    control: &BatchSessionControl,
) -> Result<MutexGuard<'_, BatchTerminalState>, core::Error> {
    control
        .terminal_state
        .lock()
        .map_err(|_| batch_lock_poisoned("batch terminal state"))
}

fn running_batch_sessions(
    sessions: &HashMap<String, BatchSessionEntry>,
) -> Vec<crate::TranscriptionSession> {
    let mut running: Vec<_> = sessions
        .iter()
        .filter(|(_, entry)| entry.abort_handle.is_some())
        .filter(|(_, entry)| {
            lock_terminal_state(&entry.control)
                .is_ok_and(|state| *state == BatchTerminalState::Running)
        })
        .map(|(session_id, entry)| crate::TranscriptionSession {
            session_id: session_id.clone(),
            file_path: entry.file_path.clone(),
            provider: entry.provider.clone(),
            model: entry.model.clone(),
            started_at_ms: entry.started_at_ms,
            resume_context: entry.resume_context.clone(),
            completed: false,
        })
        .collect();
    running.sort_by(|a, b| a.session_id.cmp(&b.session_id));
    running
}

fn reserve_batch_session(
    registry: &BatchSessionRegistry,
    session_id: &str,
    control: Arc<BatchSessionControl>,
    wait_for_native_completion: bool,
) -> Result<(), core::Error> {
    let mut sessions = lock_batch_sessions(registry)?;
    if sessions.contains_key(session_id) {
        return Err(core::Error::BatchError(
            "session already running".to_string(),
        ));
    }
    if sessions.len() >= MAX_ACTIVE_BATCH_SESSIONS {
        return Err(core::Error::BatchError(format!(
            "too many active transcription sessions (maximum {MAX_ACTIVE_BATCH_SESSIONS})"
        )));
    }

    sessions.insert(
        session_id.to_string(),
        BatchSessionEntry {
            control,
            abort_handle: None,
            wait_for_native_completion,
            file_path: String::new(),
            provider: None,
            model: None,
            started_at_ms: 0,
            resume_context: None,
        },
    );
    Ok(())
}

fn should_emit_event(control: &BatchSessionControl, event: &core::BatchEvent) -> bool {
    let Ok(state) = lock_terminal_state(control) else {
        return false;
    };
    let state = *state;

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
    let Ok(mut state) = lock_terminal_state(control) else {
        return false;
    };

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
        if let Ok(mut state) = lock_terminal_state(control)
            && *state == BatchTerminalState::Running
        {
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
    let Ok(mut sessions) = lock_batch_sessions(registry) else {
        return;
    };

    let should_remove = sessions
        .get(session_id)
        .is_some_and(|entry| Arc::ptr_eq(&entry.control, control));
    if should_remove {
        sessions.remove(session_id);
    }
}

fn abort_batch_entry(entry: BatchSessionEntry) {
    if let Some(abort_handle) = entry.abort_handle {
        abort_handle.abort();
    }
}

fn prepare_batch_stop(
    registry: &BatchSessionRegistry,
    session_id: &str,
) -> Option<(Arc<BatchSessionControl>, Option<BatchSessionEntry>)> {
    let mut sessions = lock_batch_sessions(registry).ok()?;
    let entry = sessions.get(session_id)?;

    if entry.wait_for_native_completion {
        Some((entry.control.clone(), None))
    } else {
        sessions
            .remove(session_id)
            .map(|entry| (entry.control.clone(), Some(entry)))
    }
}

fn stop_batch_session(
    app: &tauri::AppHandle,
    registry: &Arc<BatchSessionRegistry>,
    session_id: &str,
) {
    let Some((control, abort_entry)) = prepare_batch_stop(registry, session_id) else {
        return;
    };

    if mark_terminal_state(&control, BatchTerminalState::Stopped) {
        let _ = TranscriptionEvent::Stopped {
            session_id: session_id.to_string(),
        }
        .emit(app);
    }

    if let Some(entry) = abort_entry {
        abort_batch_entry(entry);
    }
}

fn batch_idle_timeout(params: &TranscriptionParams) -> Option<Duration> {
    let batch_params: core::BatchParams = params.clone().into();

    core::expects_progressive_batch(&batch_params).then_some(BATCH_IDLE_TIMEOUT)
}

fn spawn_idle_timeout_monitor(
    app: tauri::AppHandle,
    registry: Arc<BatchSessionRegistry>,
    session_id: String,
    control: Arc<BatchSessionControl>,
    abort_handle: tokio::task::AbortHandle,
    idle_timeout: Duration,
) -> JoinHandle<()> {
    let mut activity_rx = control.last_activity_tx.subscribe();

    tokio::spawn(async move {
        loop {
            let deadline = *activity_rx.borrow() + idle_timeout;
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
                        error: format!(
                            "Transcription timed out after {} seconds without progress.",
                            idle_timeout.as_secs()
                        ),
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

    fn make_registry(control: Arc<BatchSessionControl>) -> Arc<BatchSessionRegistry> {
        Arc::new(BatchSessionRegistry {
            sessions: std::sync::Mutex::new(std::collections::HashMap::from([(
                "session-1".to_string(),
                BatchSessionEntry {
                    control,
                    abort_handle: None,
                    wait_for_native_completion: false,
                    file_path: String::new(),
                    provider: None,
                    model: None,
                    started_at_ms: 0,
                    resume_context: None,
                },
            )])),
            ..Default::default()
        })
    }

    fn poison_terminal_state(control: Arc<BatchSessionControl>) {
        assert!(
            std::thread::spawn(move || {
                let _guard = control.terminal_state.lock().unwrap();
                panic!("poison terminal state");
            })
            .join()
            .is_err()
        );
    }

    fn poison_registry(registry: Arc<BatchSessionRegistry>) {
        assert!(
            std::thread::spawn(move || {
                let _guard = registry.sessions.lock().unwrap();
                panic!("poison registry");
            })
            .join()
            .is_err()
        );
    }

    fn transcription_params(
        provider: core::BatchProvider,
        base_url: &str,
        model: Option<&str>,
    ) -> TranscriptionParams {
        TranscriptionParams {
            session_id: "session-1".to_string(),
            provider,
            file_path: "/tmp/audio.wav".to_string(),
            model: model.map(ToOwned::to_owned),
            base_url: base_url.to_string(),
            api_key: "key".to_string(),
            languages: vec![anlg_language::ISO639::En.into()],
            keywords: vec![],
            num_speakers: None,
            min_speakers: None,
            max_speakers: None,
            resume_context: None,
        }
    }

    fn empty_response() -> owhisper_interface::batch::Response {
        serde_json::from_value(serde_json::json!({
            "metadata": {},
            "results": { "channels": [] }
        }))
        .unwrap()
    }

    fn completed_session(session_id: &str) -> crate::TranscriptionSession {
        crate::TranscriptionSession {
            session_id: session_id.to_string(),
            file_path: "/tmp/audio.wav".to_string(),
            provider: None,
            model: None,
            started_at_ms: 0,
            resume_context: None,
            completed: true,
        }
    }

    #[test]
    fn completed_batches_expire_after_retention() {
        let registry = BatchSessionRegistry::default();
        store_completed_batch(&registry, completed_session("session-1"), empty_response());
        let mut completed = lock_completed_batches(&registry).unwrap();
        let stored_at = completed["session-1"].completed_at_ms;
        prune_completed_batches(
            &registry,
            &mut completed,
            stored_at + COMPLETED_BATCH_RETENTION_MS,
        );
        assert!(completed.is_empty());
    }

    #[test]
    fn completed_batches_survive_restart_until_discarded() {
        let dir = std::env::temp_dir().join(format!(
            "anarlog-batch-results-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let registry = BatchSessionRegistry {
            completed_dir: Some(dir.clone()),
            ..Default::default()
        };
        store_completed_batch(
            &registry,
            completed_session("session-1:recovery"),
            empty_response(),
        );

        let restarted = BatchSessionRegistry {
            completed_dir: Some(dir.clone()),
            ..Default::default()
        };
        load_completed_batches(&restarted);
        assert!(
            lock_completed_batches(&restarted)
                .unwrap()
                .contains_key("session-1:recovery")
        );

        discard_completed_batch(&restarted, "session-1:recovery");
        let reloaded = BatchSessionRegistry {
            completed_dir: Some(dir.clone()),
            ..Default::default()
        };
        load_completed_batches(&reloaded);
        assert!(lock_completed_batches(&reloaded).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn terminal_state_transitions_once_and_stops_events() {
        let control = make_control();
        let event = core::BatchEvent::BatchStarted {
            session_id: "session-1".to_string(),
        };

        assert!(should_emit_event(&control, &event));
        assert!(mark_terminal_state(&control, BatchTerminalState::Stopped));
        assert!(!mark_terminal_state(&control, BatchTerminalState::TimedOut));
        assert!(!should_emit_event(&control, &event));
        assert!(control.cancellation_token.is_cancelled());
        assert_eq!(
            *control
                .terminal_state
                .lock()
                .expect("batch terminal state poisoned"),
            BatchTerminalState::Stopped,
        );
    }

    #[test]
    fn poisoned_locks_fail_closed_without_panicking() {
        let control = make_control();
        let registry = make_registry(control.clone());
        let event = core::BatchEvent::BatchStarted {
            session_id: "session-1".to_string(),
        };
        poison_terminal_state(control.clone());

        assert!(!should_emit_event(&control, &event));
        assert!(!mark_terminal_state(&control, BatchTerminalState::Stopped));
        finish_batch_session(&registry, "session-1", &control);
        assert!(
            !registry
                .sessions
                .lock()
                .expect("batch session registry poisoned")
                .contains_key("session-1")
        );

        poison_registry(registry.clone());
        assert!(matches!(
            lock_batch_sessions(&registry),
            Err(core::Error::BatchError(_))
        ));
    }

    #[test]
    fn reserve_batch_session_rejects_duplicate_session_id_without_replacement() {
        let original = make_control();
        let registry = make_registry(original.clone());

        let error = reserve_batch_session(&registry, "session-1", make_control(), false)
            .expect_err("duplicate session should be rejected");

        assert!(error.to_string().contains("session already running"));
        let sessions = registry
            .sessions
            .lock()
            .expect("batch session registry poisoned");
        assert!(Arc::ptr_eq(
            &sessions.get("session-1").unwrap().control,
            &original
        ));
    }

    #[test]
    fn reserve_batch_session_enforces_active_session_capacity() {
        let registry = Arc::new(BatchSessionRegistry::default());

        for index in 0..MAX_ACTIVE_BATCH_SESSIONS {
            reserve_batch_session(
                &registry,
                &format!("session-{index}"),
                make_control(),
                false,
            )
            .unwrap();
        }

        let error = reserve_batch_session(&registry, "one-too-many", make_control(), false)
            .expect_err("session beyond capacity should be rejected");
        assert!(
            error
                .to_string()
                .contains("too many active transcription sessions")
        );
        assert_eq!(
            registry
                .sessions
                .lock()
                .expect("batch session registry poisoned")
                .len(),
            MAX_ACTIVE_BATCH_SESSIONS
        );
    }

    #[test]
    fn stopped_native_sessions_hold_admission_until_workers_finish() {
        let registry = Arc::new(BatchSessionRegistry::default());
        let mut controls = Vec::new();

        for index in 0..MAX_ACTIVE_BATCH_SESSIONS {
            let session_id = format!("native-{index}");
            let control = make_control();
            reserve_batch_session(&registry, &session_id, control.clone(), true).unwrap();

            let (stopped_control, abort_entry) = prepare_batch_stop(&registry, &session_id)
                .expect("native session should still be registered");
            assert!(abort_entry.is_none());
            assert!(Arc::ptr_eq(&stopped_control, &control));
            assert!(mark_terminal_state(
                &stopped_control,
                BatchTerminalState::Stopped
            ));

            let (_, repeated_abort_entry) = prepare_batch_stop(&registry, &session_id)
                .expect("repeated stop should retain native session");
            assert!(repeated_abort_entry.is_none());
            assert!(!mark_terminal_state(
                &stopped_control,
                BatchTerminalState::Stopped
            ));
            controls.push((session_id, control));
        }

        let error = reserve_batch_session(&registry, "replacement", make_control(), true)
            .expect_err("stopped native workers must continue occupying admission");
        assert!(
            error
                .to_string()
                .contains("too many active transcription sessions")
        );

        let (finished_id, finished_control) = &controls[0];
        finish_batch_session(&registry, finished_id, finished_control);
        reserve_batch_session(&registry, "replacement", make_control(), true)
            .expect("admission should reopen after the native worker exits");
    }

    #[test]
    fn concurrent_same_id_reservation_admits_exactly_one_session() {
        let registry = Arc::new(BatchSessionRegistry::default());
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let attempts = (0..2)
            .map(|_| {
                let registry = registry.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let control = make_control();
                    barrier.wait();
                    reserve_batch_session(&registry, "same-id", control, false).is_ok()
                })
            })
            .collect::<Vec<_>>();

        barrier.wait();
        let results = attempts
            .into_iter()
            .map(|attempt| attempt.join().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(results.iter().filter(|result| **result).count(), 1);
        assert_eq!(
            registry
                .sessions
                .lock()
                .expect("batch session registry poisoned")
                .len(),
            1
        );
    }

    #[test]
    fn batch_idle_timeout_applies_only_to_local_batch() {
        let cases = [
            (
                core::BatchProvider::Anarlog,
                "https://api.anarlog.so/stt",
                None,
            ),
            (core::BatchProvider::Am, "https://api.anarlog.so/stt", None),
            (
                core::BatchProvider::Am,
                "http://localhost:50060/v1",
                Some(BATCH_IDLE_TIMEOUT),
            ),
        ];

        for (provider, base_url, expected) in cases {
            let params = transcription_params(provider, base_url, None);
            assert_eq!(batch_idle_timeout(&params), expected, "{base_url}");
        }
    }
}
