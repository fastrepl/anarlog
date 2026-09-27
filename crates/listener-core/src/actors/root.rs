use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use ractor::{
    Actor, ActorCell, ActorProcessingErr, ActorRef, ActorStatus, RpcReplyPort, SupervisionEvent,
};
use tracing::Instrument;

use crate::actors::session::lifecycle::{
    clear_sentry_session_context, configure_sentry_session_context, emit_session_ended,
};
use crate::actors::{
    SessionConfigUpdate, SessionContext, SessionMsg, SessionParams, session_span,
    spawn_session_supervisor,
};
use crate::{ListenerRuntime, SessionLifecycleEvent, Snapshot, StartSessionError, State};
use anlg_audio::AudioProvider;

pub enum RootMsg {
    RetryCleanup,
    StartSession(SessionParams, RpcReplyPort<Result<(), StartSessionError>>),
    UpdateSessionConfig(SessionConfigUpdate, RpcReplyPort<()>),
    StopSession(RpcReplyPort<()>),
    GetState(RpcReplyPort<State>),
    GetSnapshot(RpcReplyPort<Snapshot>),
}

pub struct RootArgs {
    pub runtime: Arc<dyn ListenerRuntime>,
    pub audio: Arc<dyn AudioProvider>,
}

pub struct RootState {
    runtime: Arc<dyn ListenerRuntime>,
    audio: Arc<dyn AudioProvider>,
    active_session_id: Option<String>,
    active_supervisor: Option<ActorCell>,
    finalizing_sessions: HashMap<String, ActorCell>,
    cleanup_failed_sessions: HashMap<String, bool>,
    cleanup_retry_scheduled: bool,
}

pub struct RootActor;

impl RootActor {
    pub fn name() -> ractor::ActorName {
        "listener_root_actor".into()
    }
}

#[ractor::async_trait]
impl Actor for RootActor {
    type Msg = RootMsg;
    type State = RootState;
    type Arguments = RootArgs;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let (retry, cleanup_failed_sessions) =
            cleanup_interrupted_audio(args.runtime.clone(), HashSet::new(), HashMap::new()).await;
        if retry {
            myself.send_after(std::time::Duration::from_secs(30), || RootMsg::RetryCleanup);
        }
        Ok(RootState {
            runtime: args.runtime,
            audio: args.audio,
            active_session_id: None,
            active_supervisor: None,
            finalizing_sessions: HashMap::new(),
            cleanup_failed_sessions,
            cleanup_retry_scheduled: retry,
        })
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            RootMsg::RetryCleanup => {
                let mut active = state
                    .finalizing_sessions
                    .keys()
                    .cloned()
                    .collect::<HashSet<_>>();
                active.extend(state.active_session_id.iter().cloned());
                let (retry, failed) = cleanup_interrupted_audio(
                    state.runtime.clone(),
                    active,
                    std::mem::take(&mut state.cleanup_failed_sessions),
                )
                .await;
                state.cleanup_failed_sessions = failed;
                state.cleanup_retry_scheduled = retry;
                if retry {
                    myself.send_after(std::time::Duration::from_secs(30), || RootMsg::RetryCleanup);
                }
            }
            RootMsg::StartSession(params, reply) => {
                let result = start_session_impl(myself.get_cell(), params, state).await;
                let _ = reply.send(result);
            }
            RootMsg::UpdateSessionConfig(update, reply) => {
                update_session_config_impl(update, state).await;
                let _ = reply.send(());
            }
            RootMsg::StopSession(reply) => {
                stop_session_impl(state).await;
                let _ = reply.send(());
            }
            RootMsg::GetState(reply) => {
                let _ = reply.send(root_snapshot(state).state);
            }
            RootMsg::GetSnapshot(reply) => {
                let _ = reply.send(root_snapshot(state));
            }
        }
        Ok(())
    }

    async fn handle_supervisor_evt(
        &self,
        myself: ActorRef<Self::Msg>,
        message: SupervisionEvent,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            SupervisionEvent::ActorStarted(_) | SupervisionEvent::ProcessGroupChanged(_) => {
                return Ok(());
            }
            SupervisionEvent::ActorTerminated(cell, _, reason) => {
                handle_supervisor_completion(state, cell, reason, false);
            }
            SupervisionEvent::ActorFailed(cell, error) => {
                handle_supervisor_completion(state, cell, Some(format!("{:?}", error)), true);
            }
        }
        if !state.cleanup_retry_scheduled {
            state.cleanup_retry_scheduled = true;
            myself.send_after(std::time::Duration::from_secs(30), || RootMsg::RetryCleanup);
        }
        Ok(())
    }
}

async fn cleanup_interrupted_audio(
    runtime: Arc<dyn ListenerRuntime>,
    active: HashSet<String>,
    previous_failures: HashMap<String, bool>,
) -> (bool, HashMap<String, bool>) {
    let cleanup_runtime = runtime.clone();
    let fallback_failures = previous_failures.clone();
    let cleanup = tokio::task::spawn_blocking(move || {
        let mut failures = previous_failures;
        let result = (|| {
            let sessions_dir = cleanup_runtime
                .vault_base()
                .map_err(std::io::Error::other)?
                .join("sessions");
            crate::actors::recorder::recover_interrupted_captures_except(
                &sessions_dir,
                &active,
                &mut |session_id, deleting, result| {
                    let previous = failures.get(session_id).copied();
                    match result {
                        Ok(()) => {
                            failures.remove(session_id);
                        }
                        Err(_) => {
                            failures.insert(session_id.to_string(), deleting);
                        }
                    }
                    if deleting || previous.is_some() || result.is_err() {
                        emit_audio_cleanup(
                            &*cleanup_runtime,
                            session_id,
                            if result.is_ok() {
                                previous.unwrap_or(deleting)
                            } else {
                                deleting
                            },
                            result.as_ref().err().map(ToString::to_string),
                        );
                    }
                },
            )
        })();
        if result.is_ok() {
            // A prior attempt may have succeeded elsewhere (for example on stop).
            let completed: Vec<_> = failures
                .keys()
                .filter(|id| !active.contains(*id))
                .cloned()
                .collect();
            for session_id in completed {
                let deleting = failures.remove(&session_id).unwrap();
                emit_audio_cleanup(&*cleanup_runtime, &session_id, deleting, None);
            }
        }
        (result, failures)
    })
    .await;
    let (result, failures) = match cleanup {
        Ok((result, failures)) => (result.map_err(|error| error.to_string()), failures),
        Err(error) => (Err(error.to_string()), fallback_failures),
    };
    match result {
        Ok(deferred) => {
            emit_audio_cleanup(
                &*runtime,
                "",
                true,
                (!failures.is_empty())
                    .then(|| "Cleanup is deferred while the recording is in use".to_string()),
            );
            (deferred || !failures.is_empty(), failures)
        }
        Err(error) => {
            tracing::warn!(%error, "capture_startup_cleanup_failed");
            emit_audio_cleanup(&*runtime, "", true, Some(error));
            (true, failures)
        }
    }
}

fn emit_audio_cleanup(
    runtime: &dyn ListenerRuntime,
    session_id: &str,
    deleting: bool,
    error: Option<String>,
) {
    let operation = if deleting { "deletion" } else { "recovery" };
    runtime.emit_error(crate::SessionErrorEvent::AudioError {
        session_id: session_id.to_string(),
        error: error.map_or_else(
            || format!("audio_{operation}_completed"),
            |error| format!("audio_{operation}_failed: {error}"),
        ),
        device: None,
        is_fatal: false,
    });
}

fn root_snapshot(state: &RootState) -> Snapshot {
    let active_session_id = state
        .active_supervisor
        .as_ref()
        .and(state.active_session_id.clone());
    let finalizing_session_ids = state
        .finalizing_sessions
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let state = if active_session_id.is_some() {
        State::Active
    } else if !finalizing_session_ids.is_empty() {
        State::Finalizing
    } else {
        State::Inactive
    };

    Snapshot {
        state,
        active_session_id,
        finalizing_session_ids,
    }
}

async fn start_session_impl(
    root_cell: ActorCell,
    mut params: SessionParams,
    state: &mut RootState,
) -> Result<(), StartSessionError> {
    let requested_transcription_mode = params.transcription_mode;
    params.transcription_mode = params.effective_transcription_mode();
    let session_id = params.session_id.clone();
    let span = session_span(&session_id);

    async {
        clear_stopped_supervisors(state);

        if state.active_supervisor.is_some() {
            tracing::warn!("session_already_running");
            return Err(StartSessionError::SessionAlreadyRunning);
        }

        if state.finalizing_sessions.contains_key(&params.session_id) {
            tracing::warn!("session_is_still_finalizing");
            return Err(StartSessionError::SessionAlreadyRunning);
        }

        configure_sentry_session_context(&params);

        let app_dir = match state.runtime.vault_base() {
            Ok(base) => base.join("sessions"),
            Err(e) => {
                tracing::error!(error.message = %e, "failed_to_resolve_sessions_dir");
                clear_sentry_session_context();
                return Err(StartSessionError::FailedToResolveSessionsDir);
            }
        };

        let ctx = SessionContext {
            runtime: state.runtime.clone(),
            audio: state.audio.clone(),
            requested_transcription_mode,
            params: params.clone(),
            app_dir,
            started_at_instant: Instant::now(),
            started_at_system: SystemTime::now(),
            live_transcript: Default::default(),
        };

        match spawn_session_supervisor(ctx).await {
            Ok((supervisor_cell, _handle)) => {
                supervisor_cell.link(root_cell);

                if supervisor_cell.get_status() == ActorStatus::Stopped {
                    clear_sentry_session_context();
                    return Err(StartSessionError::FailedToStartSession);
                }

                state.active_session_id = Some(params.session_id.clone());
                state.active_supervisor = Some(supervisor_cell);

                let evt = SessionLifecycleEvent::Active {
                    session_id: params.session_id,
                    requested_transcription_mode,
                    current_transcription_mode: params.transcription_mode,
                    error: None,
                };

                state.runtime.emit_lifecycle(evt);

                tracing::info!("session_started");
                Ok(())
            }
            Err(e) => {
                tracing::error!(error.message = ?e, "failed_to_start_session");
                clear_sentry_session_context();
                Err(StartSessionError::FailedToStartSession)
            }
        }
    }
    .instrument(span)
    .await
}

async fn update_session_config_impl(update: SessionConfigUpdate, state: &mut RootState) {
    let Some(active_session_id) = &state.active_session_id else {
        return;
    };

    if active_session_id != &update.session_id {
        return;
    }

    let Some(supervisor) = &state.active_supervisor else {
        return;
    };

    let session_ref: ActorRef<SessionMsg> = supervisor.clone().into();
    if let Err(error) = session_ref.cast(SessionMsg::UpdateConfig(update)) {
        tracing::warn!(?error, "failed_to_cast_session_config_update");
    }
}

fn clear_stopped_supervisors(state: &mut RootState) {
    if state
        .active_supervisor
        .as_ref()
        .is_some_and(|supervisor| supervisor.get_status() == ActorStatus::Stopped)
    {
        let session_id = state.active_session_id.take().unwrap_or_default();
        tracing::warn!(%session_id, "clearing_stale_active_session_supervisor");
        state.active_supervisor = None;
    }

    state.finalizing_sessions.retain(|session_id, supervisor| {
        let should_keep = supervisor.get_status() != ActorStatus::Stopped;
        if !should_keep {
            tracing::warn!(%session_id, "clearing_stale_finalizing_session_supervisor");
        }
        should_keep
    });
}

async fn stop_session_impl(state: &mut RootState) {
    if let Some(supervisor) = state.active_supervisor.take() {
        let session_id = state.active_session_id.take().unwrap_or_default();
        state
            .finalizing_sessions
            .insert(session_id.clone(), supervisor.clone());

        let span = session_span(&session_id);
        let _guard = span.enter();
        tracing::info!("session_finalizing");

        state
            .runtime
            .emit_lifecycle(SessionLifecycleEvent::Finalizing {
                session_id: session_id.clone(),
            });

        let session_ref: ActorRef<SessionMsg> = supervisor.clone().into();
        if let Err(error) = session_ref.cast(SessionMsg::Shutdown) {
            tracing::warn!(
                ?error,
                "failed_to_cast_session_shutdown_falling_back_to_stop"
            );
            supervisor.stop(Some("session_stop_cast_failed".to_string()));
        }
    }
}

fn handle_supervisor_completion(
    state: &mut RootState,
    cell: ActorCell,
    reason: Option<String>,
    failed: bool,
) {
    if let Some(supervisor) = &state.active_supervisor
        && cell.get_id() == supervisor.get_id()
    {
        let session_id = state.active_session_id.take().unwrap_or_default();
        let span = session_span(&session_id);
        let _guard = span.enter();

        if failed {
            tracing::warn!(?reason, "active_session_supervisor_failed");
        } else {
            tracing::info!(?reason, "active_session_supervisor_terminated");
        }

        state.active_supervisor = None;

        let sessions_base = state
            .runtime
            .vault_base()
            .map(|base| base.join("sessions"))
            .unwrap_or_else(|_| std::env::temp_dir());
        emit_session_ended(
            &*state.runtime,
            &sessions_base,
            &session_id,
            reason,
            state.active_session_id.is_none(),
        );
        return;
    }

    if let Some((session_id, _)) = state
        .finalizing_sessions
        .iter()
        .find(|(_, tracked)| tracked.get_id() == cell.get_id())
        .map(|(session_id, tracked)| (session_id.clone(), tracked.clone()))
    {
        let span = session_span(&session_id);
        let _guard = span.enter();

        if failed {
            tracing::warn!(?reason, "finalizing_session_supervisor_failed");
        } else {
            tracing::info!(?reason, "finalizing_session_supervisor_terminated");
        }

        state.finalizing_sessions.remove(&session_id);

        let sessions_base = state
            .runtime
            .vault_base()
            .map(|base| base.join("sessions"))
            .unwrap_or_else(|_| std::env::temp_dir());
        emit_session_ended(
            &*state.runtime,
            &sessions_base,
            &session_id,
            reason,
            state.active_session_id.is_none(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_audio::{CaptureConfig, CaptureStream};
    use std::path::PathBuf;

    struct Runtime(PathBuf, std::sync::Mutex<Vec<(String, String)>>);
    impl anlg_storage::StorageRuntime for Runtime {
        fn global_base(&self) -> Result<PathBuf, anlg_storage::Error> {
            Ok(self.0.clone())
        }
        fn vault_base(&self) -> Result<PathBuf, anlg_storage::Error> {
            Ok(self.0.clone())
        }
    }
    impl ListenerRuntime for Runtime {
        fn emit_lifecycle(&self, _: SessionLifecycleEvent) {}
        fn emit_progress(&self, _: crate::SessionProgressEvent) {}
        fn emit_error(&self, event: crate::SessionErrorEvent) {
            if let crate::SessionErrorEvent::AudioError {
                session_id, error, ..
            } = event
            {
                self.1.lock().unwrap().push((session_id, error));
            }
        }
        fn emit_data(&self, _: crate::SessionDataEvent) {}
    }
    impl AudioProvider for Runtime {
        fn open_capture(&self, _: CaptureConfig) -> Result<CaptureStream, anlg_audio::Error> {
            unreachable!()
        }
        fn open_speaker_capture(
            &self,
            _: u32,
            _: usize,
        ) -> Result<CaptureStream, anlg_audio::Error> {
            unreachable!()
        }
        fn open_mic_capture(
            &self,
            _: Option<String>,
            _: u32,
            _: usize,
        ) -> Result<CaptureStream, anlg_audio::Error> {
            unreachable!()
        }
        fn default_device_name(&self) -> String {
            "test".into()
        }
        fn list_mic_devices(&self) -> Vec<String> {
            vec![]
        }
        fn play_silence(&self) -> std::sync::mpsc::Sender<()> {
            unreachable!()
        }
        fn play_bytes(&self, _: &'static [u8]) -> std::sync::mpsc::Sender<()> {
            unreachable!()
        }
        fn probe_mic(&self, _: Option<String>) -> Result<(), anlg_audio::Error> {
            Ok(())
        }
        fn probe_speaker(&self) -> Result<(), anlg_audio::Error> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn stopped_session_failure_starts_cleanup_after_a_clean_startup() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = Arc::new(Runtime(dir.path().to_path_buf(), Default::default()));
        let args = || RootArgs {
            runtime: runtime.clone(),
            audio: runtime.clone(),
        };
        let (root, task) = Actor::spawn(None, RootActor, args()).await.unwrap();
        let mut state = RootActor.pre_start(root.clone(), args()).await.unwrap();
        assert!(!state.cleanup_retry_scheduled);

        let session = dir
            .path()
            .join("sessions")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&session).unwrap();
        std::fs::write(session.join(".delete-audio-on-stop"), b"").unwrap();
        std::fs::write(session.join("audio.mp3"), b"private audio").unwrap();
        RootActor
            .handle_supervisor_evt(
                root.clone(),
                SupervisionEvent::ActorFailed(
                    root.get_cell(),
                    std::io::Error::other("cleanup failed").into(),
                ),
                &mut state,
            )
            .await
            .unwrap();
        assert!(state.cleanup_retry_scheduled);
        RootActor
            .handle(root.clone(), RootMsg::RetryCleanup, &mut state)
            .await
            .unwrap();
        assert!(!state.cleanup_retry_scheduled);
        assert!(!session.join("audio.mp3").exists());
        root.stop(None);
        task.await.unwrap();
    }

    #[tokio::test]
    async fn cleanup_retries_failed_deletions_but_preserves_active_capture_audio() {
        let dir = tempfile::tempdir().unwrap();
        let session_id = uuid::Uuid::new_v4().to_string();
        let session = dir.path().join("sessions").join(&session_id);
        std::fs::create_dir_all(&session).unwrap();
        std::fs::write(session.join(".delete-audio-on-stop"), b"").unwrap();
        let recovery = session.join("audio-recovery");
        std::fs::write(&recovery, b"blocked directory").unwrap();
        let runtime = Arc::new(Runtime(dir.path().to_path_buf(), Default::default()));
        let (retry, failed) =
            cleanup_interrupted_audio(runtime.clone(), HashSet::new(), HashMap::new()).await;
        assert!(retry);
        assert!(failed.contains_key(&session_id));
        assert!(runtime.1.lock().unwrap().iter().any(|(id, error)| id == &session_id && error.starts_with("audio_deletion_failed:")));
        std::fs::remove_file(&recovery).unwrap();
        std::fs::create_dir(&recovery).unwrap();
        std::fs::write(recovery.join("private.part"), b"private audio").unwrap();
        let (retry, failed) =
            cleanup_interrupted_audio(runtime.clone(), HashSet::from([session_id.clone()]), failed)
                .await;
        assert!(retry);
        assert!(recovery.join("private.part").exists());
        assert!(
            runtime
                .1
                .lock()
                .unwrap()
                .last()
                .unwrap()
                .1
                .starts_with("audio_deletion_failed:")
        );
        let (retry, failed) =
            cleanup_interrupted_audio(runtime.clone(), HashSet::new(), failed).await;
        assert!(!retry);
        assert!(failed.is_empty());
        assert!(!recovery.exists());
        assert!(!session.join(".delete-audio-on-stop").exists());
        assert!(
            runtime
                .1
                .lock()
                .unwrap()
                .iter()
                .any(|(id, error)| id == &session_id && error == "audio_deletion_completed")
        );
    }

    #[tokio::test]
    async fn retained_recovery_failures_retry_without_reporting_audio_deletion() {
        let dir = tempfile::tempdir().unwrap();
        let session_id = uuid::Uuid::new_v4().to_string();
        let session = dir.path().join("sessions").join(&session_id);
        std::fs::create_dir_all(&session).unwrap();
        std::fs::write(session.join("audio.mp3"), b"retained").unwrap();
        let recovery = session.join("audio-recovery");
        std::fs::write(&recovery, b"blocked directory").unwrap();
        let runtime = Arc::new(Runtime(dir.path().to_path_buf(), Default::default()));
        let (retry, failed) =
            cleanup_interrupted_audio(runtime.clone(), HashSet::new(), HashMap::new()).await;
        assert!(retry);
        assert_eq!(failed.get(&session_id), Some(&false));
        assert!(runtime.1.lock().unwrap().iter().any(|(id, error)| id == &session_id && error.starts_with("audio_recovery_failed:")));
        std::fs::remove_file(&recovery).unwrap();
        let (retry, failed) =
            cleanup_interrupted_audio(runtime.clone(), HashSet::new(), failed).await;
        assert!(!retry);
        assert!(failed.is_empty());
        assert_eq!(
            std::fs::read(session.join("audio.mp3")).unwrap(),
            b"retained"
        );
        let events = runtime.1.lock().unwrap();
        assert!(
            events
                .iter()
                .any(|(id, error)| id == &session_id && error == "audio_recovery_completed")
        );
        assert!(
            !events
                .iter()
                .any(|(id, error)| id == &session_id && error.starts_with("audio_deletion"))
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn unreadable_cleanup_paths_do_not_clear_prior_failures() {
        let dir = tempfile::tempdir().unwrap();
        let sessions = dir.path().join("sessions");
        std::os::unix::fs::symlink(&sessions, &sessions).unwrap();
        let runtime = Arc::new(Runtime(dir.path().to_path_buf(), Default::default()));
        let (retry, failed) = cleanup_interrupted_audio(
            runtime.clone(),
            HashSet::new(),
            HashMap::from([("session".to_string(), true)]),
        )
        .await;
        assert!(retry);
        assert!(failed.contains_key("session"));
        assert!(
            runtime
                .1
                .lock()
                .unwrap()
                .iter()
                .all(|(_, error)| error.starts_with("audio_deletion_failed:"))
        );
    }

    #[tokio::test]
    async fn root_remains_available_when_startup_cleanup_fails() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("sessions"), b"unreadable directory").unwrap();
        let runtime = Arc::new(Runtime(dir.path().to_path_buf(), Default::default()));
        let (root, task) = Actor::spawn(
            None,
            RootActor,
            RootArgs {
                runtime: runtime.clone(),
                audio: runtime,
            },
        )
        .await
        .unwrap();
        let state = root
            .call(RootMsg::GetState, Some(std::time::Duration::from_secs(1)))
            .await
            .unwrap();
        assert!(matches!(state, ractor::rpc::CallResult::Success(_)));
        root.stop(None);
        task.await.unwrap();
    }
}
