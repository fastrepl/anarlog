use hypr_supervisor::{RestartBudget, RestartTracker, RetryStrategy, spawn_with_retry};
use ractor::concurrency::Duration;
use ractor::{Actor, ActorCell, ActorProcessingErr, ActorRef, SupervisionEvent};
use tracing::Instrument;

use crate::actors::session::types::{SessionContext, session_span, session_supervisor_name};
use crate::actors::{
    ChannelMode, ListenerActor, ListenerArgs, RecArgs, RecMsg, RecorderActor, SourceActor,
    SourceArgs, SourceMsg,
};
use crate::{DegradedError, InMemoryAudioDisposition, SessionLifecycleEvent, StopSessionParams};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChildKind {
    Source,
    Listener,
    Recorder,
}

const RESTART_BUDGET: RestartBudget = RestartBudget {
    max_restarts: 3,
    max_window: Duration::from_secs(15),
    reset_after: Some(Duration::from_secs(30)),
};

const RETRY_STRATEGY: RetryStrategy = RetryStrategy {
    max_attempts: 3,
    base_delay: Duration::from_millis(100),
};

const CHILD_STOP_TIMEOUT: Duration = Duration::from_secs(30);

pub struct SessionState {
    ctx: SessionContext,
    source_cell: Option<ActorCell>,
    listener_cell: Option<ActorCell>,
    recorder_cell: Option<ActorCell>,
    source_restarts: RestartTracker,
    recorder_restarts: RestartTracker,
    listener_buffering_enabled: bool,
    shutting_down: bool,
}

pub struct SessionActor;

#[derive(Debug)]
pub enum SessionMsg {
    Shutdown(StopSessionParams),
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
            let recorder_cell = if ctx.params.audio_retention != crate::AudioRetention::None {
                Some(
                    spawn_recorder(myself.get_cell(), &ctx)
                        .await
                        .map_err(|e| -> ActorProcessingErr { Box::new(e) })?,
                )
            } else {
                None
            };

            let source_ref = spawn_source(
                myself.get_cell(),
                &ctx,
                recorder_cell.as_ref().cloned(),
                crate::actors::source::ListenerRouting::Buffering,
            )
            .await
            .map_err(|e| -> ActorProcessingErr { Box::new(e) })?;

            Ok(SessionState {
                ctx,
                source_cell: Some(source_ref.get_cell()),
                listener_cell: None,
                recorder_cell,
                source_restarts: RestartTracker::new(),
                recorder_restarts: RestartTracker::new(),
                listener_buffering_enabled: true,
                shutting_down: false,
            })
        }
        .instrument(span)
        .await
    }

    // Listener is spawned in post_start so that a connection failure enters
    // degraded mode instead of killing the session -- source and recorder keep running.
    async fn post_start(
        &self,
        myself: ActorRef<Self::Msg>,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        let span = session_span(&state.ctx.params.session_id);

        async {
            let mode = ChannelMode::determine(state.ctx.params.onboarding);
            match Actor::spawn_linked(
                Some(ListenerActor::name()),
                ListenerActor,
                ListenerArgs {
                    runtime: state.ctx.runtime.clone(),
                    languages: state.ctx.params.languages.clone(),
                    onboarding: state.ctx.params.onboarding,
                    model: state.ctx.params.model.clone(),
                    base_url: state.ctx.params.base_url.clone(),
                    api_key: state.ctx.params.api_key.clone(),
                    keywords: state.ctx.params.keywords.clone(),
                    mode,
                    session_started_at: state.ctx.started_at_instant,
                    session_started_at_unix: state.ctx.started_at_system,
                    session_id: state.ctx.params.session_id.clone(),
                },
                myself.get_cell(),
            )
            .await
            {
                Ok((listener_ref, _)) => {
                    state.listener_cell = Some(listener_ref.get_cell());
                    attach_listener_to_source(state).await;
                }
                Err(e) => {
                    tracing::warn!(?e, "listener_spawn_failed_entering_degraded_mode");
                    enter_degraded_mode(
                        state,
                        DegradedError::UpstreamUnavailable {
                            message: classify_connection_failure(&state.ctx.params.base_url),
                        },
                    )
                    .await;
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
            SessionMsg::Shutdown(params) => {
                state.shutting_down = true;
                apply_stop_session_params(state, &params).await;
                shutdown_children(state, "session_stop").await;
                myself.stop(None);
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
                match identify_child(state, &cell) {
                    Some(ChildKind::Listener) => {
                        tracing::info!(?reason, "listener_terminated_entering_degraded_mode");
                        state.listener_cell = None;
                        enter_degraded_mode(state, parse_degraded_reason(reason.as_ref())).await;
                    }
                    Some(ChildKind::Source) => {
                        tracing::info!(?reason, "source_terminated_attempting_restart");
                        state.source_cell = None;
                        let is_device_change = reason.as_deref() == Some("device_change");
                        if !try_restart_source(myself.get_cell(), state, !is_device_change).await {
                            tracing::error!("source_restart_limit_exceeded_meltdown");
                            meltdown(myself, state).await;
                        }
                    }
                    Some(ChildKind::Recorder) => {
                        tracing::info!(?reason, "recorder_terminated_attempting_restart");
                        state.recorder_cell = None;
                        sync_source_recorder(state).await;
                        if !try_restart_recorder(myself.get_cell(), state).await {
                            tracing::error!("recorder_restart_limit_exceeded_meltdown");
                            meltdown(myself, state).await;
                        }
                    }
                    None => {
                        tracing::warn!("unknown_child_terminated");
                    }
                }
            }

            SupervisionEvent::ActorFailed(cell, error) => match identify_child(state, &cell) {
                Some(ChildKind::Listener) => {
                    tracing::info!(?error, "listener_failed_entering_degraded_mode");
                    state.listener_cell = None;
                    enter_degraded_mode(
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
                    if !try_restart_source(myself.get_cell(), state, true).await {
                        tracing::error!("source_restart_limit_exceeded_meltdown");
                        meltdown(myself, state).await;
                    }
                }
                Some(ChildKind::Recorder) => {
                    tracing::warn!(?error, "recorder_failed_attempting_restart");
                    state.recorder_cell = None;
                    sync_source_recorder(state).await;
                    if !try_restart_recorder(myself.get_cell(), state).await {
                        tracing::error!("recorder_restart_limit_exceeded_meltdown");
                        meltdown(myself, state).await;
                    }
                }
                None => {
                    tracing::warn!("unknown_child_failed");
                }
            },
        }
        Ok(())
    }
}

fn identify_child(state: &SessionState, cell: &ActorCell) -> Option<ChildKind> {
    if state
        .source_cell
        .as_ref()
        .is_some_and(|c| c.get_id() == cell.get_id())
    {
        return Some(ChildKind::Source);
    }
    if state
        .listener_cell
        .as_ref()
        .is_some_and(|c| c.get_id() == cell.get_id())
    {
        return Some(ChildKind::Listener);
    }
    if state
        .recorder_cell
        .as_ref()
        .is_some_and(|c| c.get_id() == cell.get_id())
    {
        return Some(ChildKind::Recorder);
    }
    None
}

async fn try_restart_source(
    supervisor_cell: ActorCell,
    state: &mut SessionState,
    count_against_budget: bool,
) -> bool {
    if count_against_budget && !state.source_restarts.record_restart(&RESTART_BUDGET) {
        return false;
    }

    let sup = supervisor_cell;
    let ctx = state.ctx.clone();
    let recorder_cell = state.recorder_cell.as_ref().cloned();
    let listener_routing = current_listener_routing(state);

    let cell = spawn_with_retry(&RETRY_STRATEGY, || {
        let sup = sup.clone();
        let ctx = ctx.clone();
        let recorder_cell = recorder_cell.clone();
        let listener_routing = listener_routing.clone();
        async move {
            let r = spawn_source(sup, &ctx, recorder_cell, listener_routing).await?;
            Ok(r.get_cell())
        }
    })
    .await;

    match cell {
        Some(c) => {
            state.source_cell = Some(c);
            true
        }
        None => false,
    }
}

async fn try_restart_recorder(supervisor_cell: ActorCell, state: &mut SessionState) -> bool {
    if state.ctx.params.audio_retention == crate::AudioRetention::None {
        return true;
    }

    if !state.recorder_restarts.record_restart(&RESTART_BUDGET) {
        return false;
    }

    let sup = supervisor_cell;
    let ctx = state.ctx.clone();

    let cell = spawn_with_retry(&RETRY_STRATEGY, || {
        let sup = sup.clone();
        let ctx = ctx.clone();
        async move { Ok(spawn_recorder(sup, &ctx).await?) }
    })
    .await;

    match cell {
        Some(c) => {
            state.recorder_cell = Some(c);
            sync_source_recorder(state).await;
            true
        }
        None => false,
    }
}

async fn meltdown(myself: ActorRef<SessionMsg>, state: &mut SessionState) {
    state.shutting_down = true;
    shutdown_children(state, "meltdown").await;
    myself.stop(Some("restart_limit_exceeded".to_string()));
}

fn classify_connection_failure(base_url: &str) -> String {
    if base_url.contains("localhost") || base_url.contains("127.0.0.1") {
        "Local transcription server is not running".to_string()
    } else {
        format!("Cannot reach transcription server at {}", base_url)
    }
}

fn parse_degraded_reason(reason: Option<&String>) -> DegradedError {
    reason
        .and_then(|r| serde_json::from_str::<DegradedError>(r).ok())
        .unwrap_or_else(|| DegradedError::StreamError {
            message: reason
                .cloned()
                .unwrap_or_else(|| "listener terminated without reason".to_string()),
        })
}

pub async fn spawn_session_supervisor(
    ctx: SessionContext,
) -> Result<(ActorCell, tokio::task::JoinHandle<()>), ActorProcessingErr> {
    let supervisor_name = session_supervisor_name(&ctx.params.session_id);
    let (actor_ref, handle) = Actor::spawn(Some(supervisor_name), SessionActor, ctx).await?;
    Ok((actor_ref.get_cell(), handle))
}

async fn spawn_source(
    supervisor_cell: ActorCell,
    ctx: &SessionContext,
    recorder_cell: Option<ActorCell>,
    listener_routing: crate::actors::source::ListenerRouting,
) -> Result<ActorRef<SourceMsg>, ractor::SpawnErr> {
    let recorder = recorder_cell.map(Into::into);
    let (source_ref, _) = Actor::spawn_linked(
        Some(SourceActor::name()),
        SourceActor,
        SourceArgs {
            mic_device: None,
            onboarding: ctx.params.onboarding,
            runtime: ctx.runtime.clone(),
            session_id: ctx.params.session_id.clone(),
            listener_routing,
            recorder,
        },
        supervisor_cell,
    )
    .await?;
    Ok(source_ref)
}

async fn spawn_recorder(
    supervisor_cell: ActorCell,
    ctx: &SessionContext,
) -> Result<ActorCell, ractor::SpawnErr> {
    let (recorder_ref, _): (ActorRef<RecMsg>, _) = Actor::spawn_linked(
        Some(RecorderActor::name()),
        RecorderActor::new(),
        RecArgs {
            app_dir: ctx.app_dir.clone(),
            session_id: ctx.params.session_id.clone(),
            audio_retention: ctx.params.audio_retention.clone(),
        },
        supervisor_cell,
    )
    .await?;
    Ok(recorder_ref.get_cell())
}

fn current_listener_routing(state: &SessionState) -> crate::actors::source::ListenerRouting {
    if let Some(cell) = &state.listener_cell {
        crate::actors::source::ListenerRouting::Attached(cell.clone().into())
    } else if state.listener_buffering_enabled {
        crate::actors::source::ListenerRouting::Buffering
    } else {
        crate::actors::source::ListenerRouting::Dropped
    }
}

async fn attach_listener_to_source(state: &SessionState) {
    if let Some(source_cell) = &state.source_cell {
        let source_ref: ActorRef<SourceMsg> = source_cell.clone().into();
        if let Err(error) = source_ref.cast(SourceMsg::SetListenerRouting(
            current_listener_routing(state),
        )) {
            tracing::warn!(?error, "failed_to_attach_listener_to_source");
        }
    }
}

async fn sync_source_recorder(state: &SessionState) {
    if let Some(source_cell) = &state.source_cell {
        let source_ref: ActorRef<SourceMsg> = source_cell.clone().into();
        let recorder = state.recorder_cell.as_ref().map(|cell| cell.clone().into());
        if let Err(error) = source_ref.cast(SourceMsg::SetRecorder(recorder)) {
            tracing::warn!(?error, "failed_to_update_source_recorder");
        }
    }
}

async fn enter_degraded_mode(state: &mut SessionState, degraded: DegradedError) {
    state.listener_buffering_enabled = false;
    attach_listener_to_source(state).await;
    state
        .ctx
        .runtime
        .emit_lifecycle(SessionLifecycleEvent::Active {
            session_id: state.ctx.params.session_id.clone(),
            error: Some(degraded),
        });
}

async fn shutdown_children(state: &mut SessionState, reason: &str) {
    if let Some(cell) = state.source_cell.take() {
        stop_child(&cell, reason, "source").await;
    }
    if let Some(cell) = state.listener_cell.take() {
        stop_child(&cell, reason, "listener").await;
    }
    if let Some(cell) = state.recorder_cell.take() {
        stop_child(&cell, reason, "recorder").await;
    }
}

async fn apply_stop_session_params(state: &SessionState, params: &StopSessionParams) {
    if state.ctx.params.audio_retention != crate::AudioRetention::Memory {
        return;
    }

    let disposition = params
        .in_memory_audio
        .clone()
        .unwrap_or(InMemoryAudioDisposition::Discard);

    if let Some(recorder_cell) = &state.recorder_cell {
        let recorder_ref: ActorRef<RecMsg> = recorder_cell.clone().into();
        if let Err(error) = ractor::call!(recorder_ref, |reply| {
            RecMsg::SetStopDispositionAndAck(disposition.clone(), reply)
        }) {
            tracing::warn!(?error, "failed_to_apply_recorder_stop_disposition");
        }
    }
}

async fn stop_child(cell: &ActorCell, reason: &str, child: &str) {
    if let Err(error) = cell
        .stop_and_wait(Some(reason.to_string()), Some(CHILD_STOP_TIMEOUT))
        .await
    {
        tracing::warn!(?error, %child, "child_stop_and_wait_failed");
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::{Instant, SystemTime};

    use ractor::ActorStatus;

    use super::*;
    use crate::{
        ListenerRuntime, SessionDataEvent, SessionErrorEvent, SessionProgressEvent,
        actors::SessionParams,
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
        fn emit_lifecycle(&self, _event: SessionLifecycleEvent) {}

        fn emit_progress(&self, _event: SessionProgressEvent) {}

        fn emit_error(&self, _event: SessionErrorEvent) {}

        fn emit_data(&self, _event: SessionDataEvent) {}
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

    fn test_ctx() -> SessionContext {
        SessionContext {
            runtime: Arc::new(TestRuntime),
            params: SessionParams {
                session_id: "session".to_string(),
                languages: vec![],
                onboarding: false,
                audio_retention: crate::AudioRetention::Disk,
                model: "test-model".to_string(),
                base_url: "http://localhost:1234".to_string(),
                api_key: "test-key".to_string(),
                keywords: vec![],
            },
            app_dir: std::env::temp_dir(),
            started_at_instant: Instant::now(),
            started_at_system: SystemTime::now(),
        }
    }

    #[test]
    fn parse_degraded_reason_uses_json_payload() {
        let reason = serde_json::to_string(&DegradedError::ConnectionTimeout).unwrap();
        let parsed = parse_degraded_reason(Some(&reason));
        assert!(matches!(parsed, DegradedError::ConnectionTimeout));
    }

    #[test]
    fn parse_degraded_reason_falls_back_for_missing_reason() {
        let parsed = parse_degraded_reason(None);
        assert!(matches!(parsed, DegradedError::StreamError { .. }));
    }

    #[test]
    fn parse_degraded_reason_falls_back_for_invalid_json() {
        let reason = "not-json".to_string();
        let parsed = parse_degraded_reason(Some(&reason));
        assert!(matches!(parsed, DegradedError::StreamError { .. }));
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
            listener_buffering_enabled: false,
            shutting_down: false,
        };

        shutdown_children(&mut state, "test_shutdown").await;

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
