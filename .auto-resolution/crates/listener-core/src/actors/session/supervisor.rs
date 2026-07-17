mod children;
mod mode;

use ractor::concurrency::Duration;
use ractor::{Actor, ActorCell, ActorProcessingErr, ActorRef, SupervisionEvent};
use tracing::Instrument;

use crate::DegradedError;
use crate::actors::session::types::{
    SessionConfigUpdate, SessionContext, SessionParams, session_span, session_supervisor_name,
};
use crate::actors::{ListenerConfigUpdate, ListenerMsg};
use owhisper_client::AdapterKind;

use self::children::{ChildKind, RESTART_BUDGET};
use self::mode::SessionModeState;

const LISTENER_RETRY_DELAYS: [Duration; 5] = [
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(10),
    Duration::from_secs(20),
    Duration::from_secs(30),
];

pub struct SessionState {
    ctx: SessionContext,
    source_cell: Option<ActorCell>,
    listener_cell: Option<ActorCell>,
    recorder_cell: Option<ActorCell>,
    source_restarts: hypr_supervisor::RestartTracker,
    recorder_restarts: hypr_supervisor::RestartTracker,
    mode: SessionModeState,
    listener_retry_attempt: usize,
    shutting_down: bool,
}

pub struct SessionActor;

#[derive(Debug)]
pub enum SessionMsg {
    Shutdown,
    RetryListener,
    UpdateConfig(SessionConfigUpdate),
}

#[ractor::async_trait]
impl Actor for SessionActor {
    type Msg = SessionMsg;
    type State = SessionState;
    type Arguments = SessionContext;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        ctx: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let session_id = ctx.params.session_id.clone();
        let span = session_span(&session_id);

        async {
            let mode = SessionModeState::new(
                ctx.requested_transcription_mode,
                ctx.params.transcription_mode,
            );
            let recorder_cell = Some(
                children::spawn_recorder(myself.get_cell(), &ctx)
                    .await
                    .map_err(|e| -> ActorProcessingErr { Box::new(e) })?,
            );
            let source_ref = children::spawn_source(
                myself.get_cell(),
                &ctx,
                recorder_cell.as_ref().cloned(),
                mode.listener_routing(None),
            )
            .await
            .map_err(|e| -> ActorProcessingErr { Box::new(e) })?;

            Ok(SessionState {
                ctx,
                source_cell: Some(source_ref.get_cell()),
                listener_cell: None,
                recorder_cell,
                source_restarts: hypr_supervisor::RestartTracker::new(),
                recorder_restarts: hypr_supervisor::RestartTracker::new(),
                mode,
                listener_retry_attempt: 0,
                shutting_down: false,
            })
        }
        .instrument(span)
        .await
    }

    // Listener is spawned in post_start so that a connection failure enters
    // batch fallback instead of killing the session -- source and recorder keep running.
    async fn post_start(
        &self,
        myself: ActorRef<Self::Msg>,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        let span = session_span(&state.ctx.params.session_id);

        async {
            if !state.mode.should_spawn_listener() {
                return Ok(());
            }

            match children::spawn_listener(myself.get_cell(), &state.ctx, None).await {
                Ok(listener_cell) => {
                    state.listener_cell = Some(listener_cell);
                    state.mode.on_listener_attached();
                    children::attach_listener_to_source(state).await;
                }
                Err(error) => {
                    tracing::warn!(?error, "listener_spawn_failed");
                    let degraded = if should_stop_on_listener_failure(state) {
                        DegradedError::StreamError {
                            message: error.to_string(),
                        }
                    } else {
                        DegradedError::UpstreamUnavailable {
                            message: mode::classify_connection_failure(&state.ctx.params.base_url),
                        }
                    };

                    handle_listener_failure(&myself, state, degraded).await;
                }
            }

            Ok(())
        }
        .instrument(span)
        .await
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            SessionMsg::Shutdown => {
                state.shutting_down = true;
                children::shutdown_children(state, "session_stop").await;
                myself.stop(None);
            }
            SessionMsg::RetryListener => {
                retry_listener(myself, state).await;
            }
            SessionMsg::UpdateConfig(update) => {
                update_config(myself, state, update).await;
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
        let span = session_span(&state.ctx.params.session_id);
        let _guard = span.enter();

        state.source_restarts.maybe_reset(&RESTART_BUDGET);
        state.recorder_restarts.maybe_reset(&RESTART_BUDGET);

        if state.shutting_down {
            return Ok(());
        }

        match message {
            SupervisionEvent::ActorStarted(_) | SupervisionEvent::ProcessGroupChanged(_) => {}

            SupervisionEvent::ActorTerminated(cell, _, reason) => {
                match children::identify_child(state, &cell) {
                    Some(ChildKind::Listener) => {
                        tracing::info!(?reason, "listener_terminated");
                        state.listener_cell = None;
                        handle_listener_failure(
                            &myself,
                            state,
                            mode::parse_degraded_reason(reason.as_ref()),
                        )
                        .await;
                    }
                    Some(ChildKind::Source) => {
                        tracing::info!(?reason, "source_terminated_attempting_restart");
                        state.source_cell = None;
                        let is_device_change = reason.as_deref() == Some("device_change");
                        if !children::try_restart_source(
                            myself.get_cell(),
                            state,
                            !is_device_change,
                        )
                        .await
                        {
                            tracing::error!("source_restart_limit_exceeded_meltdown");
                            meltdown(myself, state).await;
                        }
                    }
                    Some(ChildKind::Recorder) => {
                        tracing::info!(?reason, "recorder_terminated_attempting_restart");
                        state.recorder_cell = None;
                        children::sync_source_recorder(state).await;
                        if !children::try_restart_recorder(myself.get_cell(), state).await {
                            tracing::error!("recorder_restart_limit_exceeded_meltdown");
                            meltdown(myself, state).await;
                        }
                    }
                    None => {
                        tracing::warn!("unknown_child_terminated");
                    }
                }
            }

            SupervisionEvent::ActorFailed(cell, error) => {
                match children::identify_child(state, &cell) {
                    Some(ChildKind::Listener) => {
                        tracing::info!(?error, "listener_failed");
                        state.listener_cell = None;
                        handle_listener_failure(
                            &myself,
                            state,
                            DegradedError::StreamError {
                                message: format!("{:?}", error),
                            },
                        )
                        .await;
                    }
                    Some(ChildKind::Source) => {
                        tracing::warn!(?error, "source_failed_attempting_restart");
                        state.source_cell = None;
                        if !children::try_restart_source(myself.get_cell(), state, true).await {
                            tracing::error!("source_restart_limit_exceeded_meltdown");
                            meltdown(myself, state).await;
                        }
                    }
                    Some(ChildKind::Recorder) => {
                        tracing::warn!(?error, "recorder_failed_attempting_restart");
                        state.recorder_cell = None;
                        children::sync_source_recorder(state).await;
                        if !children::try_restart_recorder(myself.get_cell(), state).await {
                            tracing::error!("recorder_restart_limit_exceeded_meltdown");
                            meltdown(myself, state).await;
                        }
                    }
                    None => {
                        tracing::warn!("unknown_child_failed");
                    }
                }
            }
        }
        Ok(())
    }
}

pub async fn spawn_session_supervisor(
    ctx: SessionContext,
) -> Result<(ActorCell, tokio::task::JoinHandle<()>), ActorProcessingErr> {
    let supervisor_name = session_supervisor_name(&ctx.params.session_id);
    let (actor_ref, handle) = Actor::spawn(Some(supervisor_name), SessionActor, ctx).await?;
    Ok((actor_ref.get_cell(), handle))
}

async fn emit_active_lifecycle_event(state: &SessionState, error: Option<DegradedError>) {
    state.ctx.runtime.emit_lifecycle(
        state
            .mode
            .active_event(state.ctx.params.session_id.clone(), error),
    );
}

async fn enter_batch_fallback(state: &mut SessionState, degraded: DegradedError) {
    state.mode.enter_batch_fallback();
    children::attach_listener_to_source(state).await;
    emit_active_lifecycle_event(state, Some(degraded)).await;
}

async fn update_config(
    myself: ActorRef<SessionMsg>,
    state: &mut SessionState,
    update: SessionConfigUpdate,
) {
    if update.session_id != state.ctx.params.session_id {
        return;
    }

    let should_refresh_listener = state.mode.should_spawn_listener()
        && update_requires_listener_refresh(&state.ctx.params, &update);

    state.ctx.params.languages = update.languages;
    state.ctx.params.participant_human_ids = update.participant_human_ids;
    state.ctx.params.self_human_id = update.self_human_id;

    if should_refresh_listener {
        refresh_listener(myself, state).await;
        return;
    }

    if let Some(listener_cell) = &state.listener_cell {
        let listener_ref: ractor::ActorRef<ListenerMsg> = listener_cell.clone().into();
        if let Err(error) = listener_ref.cast(ListenerMsg::UpdateConfig(ListenerConfigUpdate {
            languages: state.ctx.params.languages.clone(),
            participant_human_ids: state.ctx.params.participant_human_ids.clone(),
            self_human_id: state.ctx.params.self_human_id.clone(),
        })) {
            tracing::warn!(?error, "failed_to_cast_listener_config_update");
        }
    }
}

async fn refresh_listener(myself: ActorRef<SessionMsg>, state: &mut SessionState) {
    let replay_duration_secs = children::stop_listener(state, "config_update").await;

    if !state.mode.should_spawn_listener() {
        return;
    }

    let replay_offset_secs =
        (state.ctx.started_at_instant.elapsed().as_secs_f64() - replay_duration_secs).max(0.0);

    match children::spawn_listener(myself.get_cell(), &state.ctx, Some(replay_offset_secs)).await {
        Ok(listener_cell) => {
            state.listener_cell = Some(listener_cell);
            state.mode.on_listener_attached();
            children::attach_listener_to_source(state).await;
        }
        Err(error) => {
            tracing::warn!(?error, "listener_refresh_failed");
            let degraded = if should_stop_on_listener_failure(state) {
                DegradedError::StreamError {
                    message: error.to_string(),
                }
            } else {
                DegradedError::UpstreamUnavailable {
                    message: mode::classify_connection_failure(&state.ctx.params.base_url),
                }
            };
            handle_listener_failure(&myself, state, degraded).await;
        }
    }
}

fn update_requires_listener_refresh(current: &SessionParams, update: &SessionConfigUpdate) -> bool {
    current.languages != update.languages
        || expected_speaker_count(
            &current.participant_human_ids,
            current.self_human_id.as_deref(),
        ) != expected_speaker_count(
            &update.participant_human_ids,
            update.self_human_id.as_deref(),
        )
}

fn expected_speaker_count(
    participant_human_ids: &[String],
    self_human_id: Option<&str>,
) -> Option<u32> {
    let mut participants = participant_human_ids.to_vec();

    if let Some(self_human_id) = self_human_id
        && !participants.iter().any(|id| id == self_human_id)
    {
        participants.push(self_human_id.to_string());
    }

    participants.sort();
    participants.dedup();

    (participants.len() > 1).then_some(participants.len() as u32)
}

async fn handle_listener_failure(
    myself: &ActorRef<SessionMsg>,
    state: &mut SessionState,
    degraded: DegradedError,
) {
    if should_stop_on_listener_failure(state) {
        tracing::warn!("listener_failed_stopping_session");
        stop_after_listener_failure(myself, state, degraded).await;
    } else {
        let should_retry = should_retry_listener_failure(&degraded);
        enter_batch_fallback(state, degraded).await;
        if should_retry {
            schedule_listener_retry(myself, state);
        }
    }
}

fn should_retry_listener_failure(degraded: &DegradedError) -> bool {
    !matches!(degraded, DegradedError::AuthenticationFailed { .. })
}

fn schedule_listener_retry(myself: &ActorRef<SessionMsg>, state: &mut SessionState) {
    let delay = listener_retry_delay(state.listener_retry_attempt);
    state.listener_retry_attempt = state.listener_retry_attempt.saturating_add(1);
    tracing::info!(
        ?delay,
        attempt = state.listener_retry_attempt,
        "listener_retry_scheduled"
    );
    myself.send_after(delay, || SessionMsg::RetryListener);
}

fn listener_retry_delay(attempt: usize) -> Duration {
    LISTENER_RETRY_DELAYS[attempt.min(LISTENER_RETRY_DELAYS.len() - 1)]
}

async fn retry_listener(myself: ActorRef<SessionMsg>, state: &mut SessionState) {
    if state.shutting_down || state.listener_cell.is_some() || !state.mode.should_retry_listener() {
        return;
    }

    let replay_duration_secs = children::prepare_listener_refresh(state).await;
    let replay_offset_secs =
        (state.ctx.started_at_instant.elapsed().as_secs_f64() - replay_duration_secs).max(0.0);

    match children::spawn_listener(myself.get_cell(), &state.ctx, Some(replay_offset_secs)).await {
        Ok(listener_cell) => {
            tracing::info!(
                attempts = state.listener_retry_attempt,
                "listener_reconnected"
            );
            state.listener_cell = Some(listener_cell);
            state.listener_retry_attempt = 0;
            state.mode.on_listener_attached();
            children::attach_listener_to_source(state).await;
            emit_active_lifecycle_event(state, None).await;
        }
        Err(error) => {
            tracing::warn!(?error, "listener_retry_failed");
            handle_listener_failure(
                &myself,
                state,
                DegradedError::UpstreamUnavailable {
                    message: mode::classify_connection_failure(&state.ctx.params.base_url),
                },
            )
            .await;
        }
    }
}

fn should_stop_on_listener_failure(state: &SessionState) -> bool {
    state.ctx.params.uses_local_soniqo_live_model()
        || matches!(
            AdapterKind::from_url_and_languages(
                &state.ctx.params.base_url,
                &state.ctx.params.languages,
                Some(&state.ctx.params.model),
            ),
            AdapterKind::Soniox
        )
}

async fn stop_after_listener_failure(
    myself: &ActorRef<SessionMsg>,
    state: &mut SessionState,
    degraded: DegradedError,
) {
    emit_active_lifecycle_event(state, Some(degraded.clone())).await;
    state.shutting_down = true;
    children::shutdown_children(state, "listener_failure").await;
    let reason = serde_json::to_string(&degraded).ok();
    myself.stop(reason);
}

async fn meltdown(myself: ActorRef<SessionMsg>, state: &mut SessionState) {
    state.shutting_down = true;
    children::shutdown_children(state, "meltdown").await;
    myself.stop(Some("restart_limit_exceeded".to_string()));
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::{Instant, SystemTime};

    use hypr_audio::{AudioProvider, CaptureConfig, CaptureStream};
    use hypr_supervisor::RestartTracker;
    use ractor::ActorStatus;

    use super::*;
    use crate::{
        ListenerRuntime, SessionDataEvent, SessionErrorEvent, SessionProgressEvent,
        TranscriptionMode, actors::SessionParams,
    };

    struct TestRuntime;

    impl hypr_storage::StorageRuntime for TestRuntime {
        fn global_base(&self) -> Result<PathBuf, hypr_storage::Error> {
            Ok(std::env::temp_dir())
        }

        fn vault_base(&self) -> Result<PathBuf, hypr_storage::Error> {
            Ok(std::env::temp_dir())
        }
    }

    impl ListenerRuntime for TestRuntime {
        fn emit_lifecycle(&self, _event: crate::SessionLifecycleEvent) {}

        fn emit_progress(&self, _event: SessionProgressEvent) {}

        fn emit_error(&self, _event: SessionErrorEvent) {}

        fn emit_data(&self, _event: SessionDataEvent) {}
    }

    impl AudioProvider for TestRuntime {
        fn open_capture(&self, _config: CaptureConfig) -> Result<CaptureStream, hypr_audio::Error> {
            unimplemented!()
        }
        fn open_speaker_capture(
            &self,
            _sample_rate: u32,
            _chunk_size: usize,
        ) -> Result<CaptureStream, hypr_audio::Error> {
            unimplemented!()
        }
        fn open_mic_capture(
            &self,
            _device: Option<String>,
            _sample_rate: u32,
            _chunk_size: usize,
        ) -> Result<CaptureStream, hypr_audio::Error> {
            unimplemented!()
        }
        fn default_device_name(&self) -> String {
            "test".to_string()
        }
        fn list_mic_devices(&self) -> Vec<String> {
            vec![]
        }
        fn play_silence(&self) -> std::sync::mpsc::Sender<()> {
            let (tx, _rx) = std::sync::mpsc::channel();
            tx
        }
        fn play_bytes(&self, _bytes: &'static [u8]) -> std::sync::mpsc::Sender<()> {
            let (tx, _rx) = std::sync::mpsc::channel();
            tx
        }
        fn probe_mic(&self, _device: Option<String>) -> Result<(), hypr_audio::Error> {
            Ok(())
        }
        fn probe_speaker(&self) -> Result<(), hypr_audio::Error> {
            Ok(())
        }
    }

    struct StopProbe {
        label: &'static str,
        tx: tokio::sync::mpsc::UnboundedSender<&'static str>,
    }

    #[ractor::async_trait]
    impl Actor for StopProbe {
        type Msg = ();
        type State = ();
        type Arguments = ();

        async fn pre_start(
            &self,
            _myself: ActorRef<Self::Msg>,
            _args: Self::Arguments,
        ) -> Result<Self::State, ActorProcessingErr> {
            Ok(())
        }

        async fn post_stop(
            &self,
            _myself: ActorRef<Self::Msg>,
            _state: &mut Self::State,
        ) -> Result<(), ActorProcessingErr> {
            let _ = self.tx.send(self.label);
            Ok(())
        }
    }

    struct SessionStopProbe;

    #[ractor::async_trait]
    impl Actor for SessionStopProbe {
        type Msg = SessionMsg;
        type State = ();
        type Arguments = ();

        async fn pre_start(
            &self,
            _myself: ActorRef<Self::Msg>,
            _args: Self::Arguments,
        ) -> Result<Self::State, ActorProcessingErr> {
            Ok(())
        }
    }

    struct RecordingRuntime {
        lifecycle_events: std::sync::Mutex<Vec<crate::SessionLifecycleEvent>>,
    }

    impl hypr_storage::StorageRuntime for RecordingRuntime {
        fn global_base(&self) -> Result<PathBuf, hypr_storage::Error> {
            Ok(std::env::temp_dir())
        }

        fn vault_base(&self) -> Result<PathBuf, hypr_storage::Error> {
            Ok(std::env::temp_dir())
        }
    }

    impl ListenerRuntime for RecordingRuntime {
        fn emit_lifecycle(&self, event: crate::SessionLifecycleEvent) {
            self.lifecycle_events.lock().unwrap().push(event);
        }

        fn emit_progress(&self, _event: SessionProgressEvent) {}

        fn emit_error(&self, _event: SessionErrorEvent) {}

        fn emit_data(&self, _event: SessionDataEvent) {}
    }

    fn test_ctx() -> SessionContext {
        SessionContext {
            runtime: Arc::new(TestRuntime),
            audio: Arc::new(TestRuntime),
            requested_transcription_mode: crate::TranscriptionMode::Live,
            params: SessionParams {
                session_id: "session".to_string(),
                languages: vec![],
                onboarding: false,
                transcription_mode: crate::TranscriptionMode::Live,
                model: "test-model".to_string(),
                base_url: "http://localhost:1234".to_string(),
                api_key: "test-key".to_string(),
                keywords: vec![],
                participant_human_ids: vec![],
                self_human_id: None,
            },
            app_dir: std::env::temp_dir(),
            started_at_instant: Instant::now(),
            started_at_system: SystemTime::now(),
        }
    }

    fn test_state(ctx: SessionContext) -> SessionState {
        SessionState {
            ctx,
            source_cell: None,
            listener_cell: None,
            recorder_cell: None,
            source_restarts: RestartTracker::new(),
            recorder_restarts: RestartTracker::new(),
            mode: SessionModeState::new(TranscriptionMode::Live, TranscriptionMode::Live),
            listener_retry_attempt: 0,
            shutting_down: false,
        }
    }

    fn test_update(
        languages: Vec<hypr_language::Language>,
        participant_human_ids: Vec<&str>,
        self_human_id: Option<&str>,
    ) -> SessionConfigUpdate {
        SessionConfigUpdate {
            session_id: "session".to_string(),
            languages,
            participant_human_ids: participant_human_ids
                .into_iter()
                .map(ToString::to_string)
                .collect(),
            self_human_id: self_human_id.map(ToString::to_string),
        }
    }

    #[test]
    fn config_update_refreshes_when_languages_change() {
        let mut ctx = test_ctx();
        ctx.params.languages = vec![hypr_language::ISO639::En.into()];
        let state = test_state(ctx);
        let update = test_update(
            vec![
                hypr_language::ISO639::En.into(),
                hypr_language::ISO639::Ko.into(),
            ],
            vec![],
            None,
        );

        assert!(update_requires_listener_refresh(&state.ctx.params, &update));
    }

    #[test]
    fn config_update_refreshes_when_expected_speaker_count_changes() {
        let mut ctx = test_ctx();
        ctx.params.participant_human_ids = vec!["self".to_string()];
        ctx.params.self_human_id = Some("self".to_string());
        let state = test_state(ctx);
        let update = test_update(vec![], vec!["self", "remote"], Some("self"));

        assert!(update_requires_listener_refresh(&state.ctx.params, &update));
    }

    #[test]
    fn config_update_does_not_refresh_for_same_speaker_count() {
        let mut ctx = test_ctx();
        ctx.params.participant_human_ids = vec!["self".to_string(), "remote-a".to_string()];
        ctx.params.self_human_id = Some("self".to_string());
        let state = test_state(ctx);
        let update = test_update(vec![], vec!["self", "remote-b"], Some("self"));

        assert!(!update_requires_listener_refresh(
            &state.ctx.params,
            &update
        ));
    }

    #[test]
    fn local_soniqo_live_listener_failure_stops_session() {
        let mut ctx = test_ctx();
        ctx.params.base_url = hypr_transcribe_soniqo::LOCAL_BASE_URL.to_string();
        ctx.params.model = "soniqo-parakeet-streaming".to_string();
        let state = test_state(ctx);

        assert!(should_stop_on_listener_failure(&state));
    }

    #[test]
    fn direct_soniox_listener_failure_stops_session() {
        let mut ctx = test_ctx();
        ctx.params.base_url = "https://api.soniox.com".to_string();
        ctx.params.model = "stt-v4".to_string();
        let state = test_state(ctx);

        assert!(should_stop_on_listener_failure(&state));
    }

    #[test]
    fn hyprnote_proxy_soniox_listener_failure_enters_batch_fallback() {
        let mut ctx = test_ctx();
        ctx.params.base_url = "https://api.hyprnote.com/stt?provider=soniox".to_string();
        ctx.params.model = "cloud".to_string();
        let state = test_state(ctx);

        assert!(!should_stop_on_listener_failure(&state));
    }

    #[test]
    fn non_soniqo_listener_failure_enters_batch_fallback() {
        let state = test_state(test_ctx());

        assert!(!should_stop_on_listener_failure(&state));
    }

    #[test]
    fn transient_listener_failures_retry_with_capped_backoff() {
        assert!(should_retry_listener_failure(
            &DegradedError::ConnectionTimeout
        ));
        assert_eq!(listener_retry_delay(0), Duration::from_secs(2));
        assert_eq!(listener_retry_delay(3), Duration::from_secs(20));
        assert_eq!(listener_retry_delay(20), Duration::from_secs(30));
    }

    #[test]
    fn authentication_failures_do_not_retry() {
        assert!(!should_retry_listener_failure(
            &DegradedError::AuthenticationFailed {
                provider: "test".to_string(),
            }
        ));
    }

    #[tokio::test]
    async fn stop_after_listener_failure_emits_degraded_active_event() {
        let runtime = Arc::new(RecordingRuntime {
            lifecycle_events: std::sync::Mutex::new(vec![]),
        });
        let mut ctx = test_ctx();
        ctx.runtime = runtime.clone();
        let mut state = test_state(ctx);
        let (actor_ref, handle) = Actor::spawn(None, SessionStopProbe, ()).await.unwrap();

        stop_after_listener_failure(
            &actor_ref,
            &mut state,
            DegradedError::StreamError {
                message: "listener failed".to_string(),
            },
        )
        .await;

        let events = runtime.lifecycle_events.lock().unwrap();
        let Some(crate::SessionLifecycleEvent::Active {
            requested_transcription_mode,
            current_transcription_mode,
            error: Some(DegradedError::StreamError { message }),
            ..
        }) = events.first()
        else {
            panic!("expected degraded active event");
        };
        assert_eq!(*requested_transcription_mode, TranscriptionMode::Live);
        assert_eq!(*current_transcription_mode, TranscriptionMode::Live);
        assert_eq!(message, "listener failed");

        drop(events);
        let _ = handle.await;
    }

    #[tokio::test]
    async fn shutdown_children_waits_in_source_listener_recorder_order() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let (source_ref, _) = Actor::spawn(
            None,
            StopProbe {
                label: "source",
                tx: tx.clone(),
            },
            (),
        )
        .await
        .unwrap();
        let (listener_ref, _) = Actor::spawn(
            None,
            StopProbe {
                label: "listener",
                tx: tx.clone(),
            },
            (),
        )
        .await
        .unwrap();
        let (recorder_ref, _) = Actor::spawn(
            None,
            StopProbe {
                label: "recorder",
                tx,
            },
            (),
        )
        .await
        .unwrap();

        let mut state = SessionState {
            ctx: test_ctx(),
            source_cell: Some(source_ref.get_cell()),
            listener_cell: Some(listener_ref.get_cell()),
            recorder_cell: Some(recorder_ref.get_cell()),
            source_restarts: RestartTracker::new(),
            recorder_restarts: RestartTracker::new(),
            mode: SessionModeState::new(TranscriptionMode::Live, TranscriptionMode::Live),
            listener_retry_attempt: 0,
            shutting_down: false,
        };

        children::shutdown_children(&mut state, "test_shutdown").await;

        let first = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        let second = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        let third = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!([first, second, third], ["source", "listener", "recorder"]);
        assert_eq!(source_ref.get_status(), ActorStatus::Stopped);
        assert_eq!(listener_ref.get_status(), ActorStatus::Stopped);
        assert_eq!(recorder_ref.get_status(), ActorStatus::Stopped);
    }
}
