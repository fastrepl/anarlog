use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use ractor::{Actor, ActorCell, ActorProcessingErr, ActorRef, RpcReplyPort, SupervisionEvent};
use tracing::Instrument;

use crate::actors::session::lifecycle::{
    clear_sentry_session_context, configure_sentry_session_context, emit_session_ended,
};
use crate::actors::{
    SessionContext, SessionMsg, SessionParams, session_span, spawn_session_supervisor,
};
use crate::{ListenerRuntime, SessionLifecycleEvent, StartSessionError, State};
use hypr_audio::AudioProvider;

pub enum RootMsg {
    StartSession(SessionParams, RpcReplyPort<Result<(), StartSessionError>>),
    StopSession(RpcReplyPort<()>),
    GetState(RpcReplyPort<State>),
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
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(RootState {
            runtime: args.runtime,
            audio: args.audio,
            active_session_id: None,
            active_supervisor: None,
            finalizing_sessions: HashMap::new(),
        })
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            RootMsg::StartSession(params, reply) => {
                let result = start_session_impl(myself.get_cell(), params, state).await;
                let _ = reply.send(result);
            }
            RootMsg::StopSession(reply) => {
                stop_session_impl(state).await;
                let _ = reply.send(());
            }
            RootMsg::GetState(reply) => {
                let fsm_state = if state.active_supervisor.is_some() {
                    State::Active
                } else if !state.finalizing_sessions.is_empty() {
                    State::Finalizing
                } else {
                    State::Inactive
                };
                let _ = reply.send(fsm_state);
            }
        }
        Ok(())
    }

    async fn handle_supervisor_evt(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: SupervisionEvent,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            SupervisionEvent::ActorStarted(_) | SupervisionEvent::ProcessGroupChanged(_) => {}
            SupervisionEvent::ActorTerminated(cell, _, reason) => {
                handle_supervisor_completion(state, cell, reason, false);
            }
            SupervisionEvent::ActorFailed(cell, error) => {
                handle_supervisor_completion(state, cell, Some(format!("{:?}", error)), true);
            }
        }
        Ok(())
    }
}

async fn start_session_impl(
    root_cell: ActorCell,
    params: SessionParams,
    state: &mut RootState,
) -> Result<(), StartSessionError> {
    let session_id = params.session_id.clone();
    let span = session_span(&session_id);

    async {
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
            params: params.clone(),
            app_dir,
            started_at_instant: Instant::now(),
            started_at_system: SystemTime::now(),
        };

        match spawn_session_supervisor(ctx).await {
            Ok((supervisor_cell, _handle)) => {
                supervisor_cell.link(root_cell);

                state.active_session_id = Some(params.session_id.clone());
                state.active_supervisor = Some(supervisor_cell);

                let evt = SessionLifecycleEvent::Active {
                    session_id: params.session_id,
                    requested_transcription_mode: params.transcription_mode,
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
