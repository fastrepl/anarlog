use std::sync::Arc;

use hypr_supervisor::{RestartBudget, RestartTracker, RetryStrategy, spawn_with_retry};
use ractor::concurrency::Duration;
use ractor::{Actor, ActorCell, ActorProcessingErr, ActorRef, SupervisionEvent};
use tracing::Instrument;

use crate::actors::session::lifecycle;
use crate::actors::session::types::{SessionContext, session_span, session_supervisor_name};
use crate::actors::{
    ChannelMode, ListenerActor, ListenerArgs, RecArgs, RecMsg, RecorderActor, SourceActor,
    SourceArgs,
};
use crate::{DegradedError, SessionLifecycleEvent};

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

pub struct SessionState {
    ctx: SessionContext,
    source_cell: Option<ActorCell>,
    listener_cell: Option<ActorCell>,
    recorder_cell: Option<ActorCell>,
    recorder_done: Option<tokio::sync::oneshot::Receiver<()>>,
    source_restarts: RestartTracker,
    recorder_restarts: RestartTracker,
    shutting_down: bool,
}

pub struct SessionActor;

#[derive(Debug)]
pub enum SessionMsg {
    Shutdown,
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
            let (source_ref, _) = Actor::spawn_linked(
                Some(SourceActor::name()),
                SourceActor,
                SourceArgs {
                    mic_device: None,
                    onboarding: ctx.params.onboarding,
                    runtime: ctx.runtime.clone(),
                    session_id: ctx.params.session_id.clone(),
                },
                myself.get_cell(),
            )
            .await?;

            let (recorder_cell, recorder_done) = if ctx.params.record_enabled {
                let (done_tx, done_rx) = tokio::sync::oneshot::channel();
                let (recorder_ref, _): (ActorRef<RecMsg>, _) = Actor::spawn_linked(
                    Some(RecorderActor::name()),
                    RecorderActor::new(),
                    RecArgs {
                        app_dir: ctx.app_dir.clone(),
                        session_id: ctx.params.session_id.clone(),
                        done_tx: Some(done_tx),
                    },
                    myself.get_cell(),
                )
                .await?;
                (Some(recorder_ref.get_cell()), Some(done_rx))
            } else {
                (None, None)
            };

            Ok(SessionState {
                ctx,
                source_cell: Some(source_ref.get_cell()),
                listener_cell: None,
                recorder_cell,
                recorder_done,
                source_restarts: RestartTracker::new(),
                recorder_restarts: RestartTracker::new(),
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
                }
                Err(e) => {
                    tracing::warn!(?e, "listener_spawn_failed_entering_degraded_mode");
                    let base_url = &state.ctx.params.base_url;
                    let degraded = DegradedError::UpstreamUnavailable {
                        message: classify_connection_failure(base_url),
                    };
                    state
                        .ctx
                        .runtime
                        .emit_lifecycle(SessionLifecycleEvent::Active {
                            session_id: state.ctx.params.session_id.clone(),
                            error: Some(degraded),
                        });
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

                if let Some(cell) = state.recorder_cell.take() {
                    let done = state.recorder_done.take();
                    cell.stop(Some("session_stop".to_string()));
                    wait_for_recorder_done(done).await;
                }

                if let Some(cell) = state.source_cell.take() {
                    cell.stop(Some("session_stop".to_string()));
                }
                if let Some(cell) = state.listener_cell.take() {
                    cell.stop(Some("session_stop".to_string()));
                }

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
                        let degraded = parse_degraded_reason(reason.as_ref());
                        state.listener_cell = None;

                        state
                            .ctx
                            .runtime
                            .emit_lifecycle(SessionLifecycleEvent::Active {
                                session_id: state.ctx.params.session_id.clone(),
                                error: Some(degraded),
                            });
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
                    let degraded = DegradedError::StreamError {
                        message: format!("{:?}", error),
                    };
                    state.listener_cell = None;

                    state
                        .ctx
                        .runtime
                        .emit_lifecycle(SessionLifecycleEvent::Active {
                            session_id: state.ctx.params.session_id.clone(),
                            error: Some(degraded),
                        });
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
    let onboarding = state.ctx.params.onboarding;
    let runtime = state.ctx.runtime.clone();
    let session_id = state.ctx.params.session_id.clone();

    let cell = spawn_with_retry(&RETRY_STRATEGY, || {
        let sup = sup.clone();
        let runtime = runtime.clone();
        let session_id = session_id.clone();
        async move {
            let (r, _) = Actor::spawn_linked(
                Some(SourceActor::name()),
                SourceActor,
                SourceArgs {
                    mic_device: None,
                    onboarding,
                    runtime,
                    session_id,
                },
                sup,
            )
            .await?;
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
    if !state.ctx.params.record_enabled {
        return true;
    }

    if !state.recorder_restarts.record_restart(&RESTART_BUDGET) {
        return false;
    }

    let sup = supervisor_cell;
    let app_dir = state.ctx.app_dir.clone();
    let session_id = state.ctx.params.session_id.clone();
    let (done_tx, done_rx) = tokio::sync::oneshot::channel();
    let done_tx = Arc::new(std::sync::Mutex::new(Some(done_tx)));

    let cell = spawn_with_retry(&RETRY_STRATEGY, || {
        let sup = sup.clone();
        let app_dir = app_dir.clone();
        let session_id = session_id.clone();
        let done_tx = done_tx.lock().unwrap().take();
        async move {
            let (r, _): (ActorRef<RecMsg>, _) = Actor::spawn_linked(
                Some(RecorderActor::name()),
                RecorderActor::new(),
                RecArgs {
                    app_dir,
                    session_id,
                    done_tx,
                },
                sup,
            )
            .await?;
            Ok(r.get_cell())
        }
    })
    .await;

    match cell {
        Some(c) => {
            state.recorder_cell = Some(c);
            state.recorder_done = Some(done_rx);
            true
        }
        None => false,
    }
}

async fn meltdown(myself: ActorRef<SessionMsg>, state: &mut SessionState) {
    state.shutting_down = true;

    if let Some(cell) = state.source_cell.take() {
        cell.stop(Some("meltdown".to_string()));
    }
    if let Some(cell) = state.listener_cell.take() {
        cell.stop(Some("meltdown".to_string()));
    }
    if let Some(cell) = state.recorder_cell.take() {
        let done = state.recorder_done.take();
        cell.stop(Some("meltdown".to_string()));
        wait_for_recorder_done(done).await;
    }
    myself.stop(Some("restart_limit_exceeded".to_string()));
}

async fn wait_for_recorder_done(done: Option<tokio::sync::oneshot::Receiver<()>>) {
    match done {
        Some(rx) => {
            tokio::time::timeout(Duration::from_secs(30), rx).await.ok();
        }
        None => {
            lifecycle::wait_for_actor_shutdown(RecorderActor::name()).await;
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
