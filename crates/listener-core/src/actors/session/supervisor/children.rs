use hypr_supervisor::{RestartBudget, RetryStrategy, spawn_with_retry};
use ractor::concurrency::Duration;
use ractor::{Actor, ActorCell, ActorRef};

use crate::actors::session::types::SessionContext;
use crate::actors::{
    ChannelMode, ListenerActor, ListenerArgs, RecArgs, RecMsg, RecorderActor, SourceActor,
    SourceArgs, SourceMsg,
};

use super::SessionState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ChildKind {
    Source,
    Listener,
    Recorder,
}

pub(super) const RESTART_BUDGET: RestartBudget = RestartBudget {
    max_restarts: 3,
    max_window: Duration::from_secs(15),
    reset_after: Some(Duration::from_secs(30)),
};

const RETRY_STRATEGY: RetryStrategy = RetryStrategy {
    max_attempts: 3,
    base_delay: Duration::from_millis(100),
};

const CHILD_STOP_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) fn identify_child(state: &SessionState, cell: &ActorCell) -> Option<ChildKind> {
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

pub(super) async fn spawn_source(
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
            audio: ctx.audio.clone(),
            session_id: ctx.params.session_id.clone(),
            listener_routing,
            recorder,
        },
        supervisor_cell,
    )
    .await?;
    Ok(source_ref)
}

pub(super) async fn spawn_recorder(
    supervisor_cell: ActorCell,
    ctx: &SessionContext,
) -> Result<ActorCell, ractor::SpawnErr> {
    let (recorder_ref, _): (ActorRef<RecMsg>, _) = Actor::spawn_linked(
        Some(RecorderActor::name()),
        RecorderActor::new(),
        RecArgs {
            app_dir: ctx.app_dir.clone(),
            session_id: ctx.params.session_id.clone(),
            recording_mode: ctx.params.recording_mode,
        },
        supervisor_cell,
    )
    .await?;
    Ok(recorder_ref.get_cell())
}

pub(super) async fn spawn_listener(
    supervisor_cell: ActorCell,
    ctx: &SessionContext,
) -> Result<ActorCell, ractor::SpawnErr> {
    let mode = ChannelMode::determine(ctx.params.onboarding);
    let (listener_ref, _): (ActorRef<crate::actors::ListenerMsg>, _) = Actor::spawn_linked(
        Some(ListenerActor::name()),
        ListenerActor,
        ListenerArgs {
            runtime: ctx.runtime.clone(),
            languages: ctx.params.languages.clone(),
            onboarding: ctx.params.onboarding,
            model: ctx.params.model.clone(),
            base_url: ctx.params.base_url.clone(),
            api_key: ctx.params.api_key.clone(),
            keywords: ctx.params.keywords.clone(),
            mode,
            session_started_at: ctx.started_at_instant,
            session_started_at_unix: ctx.started_at_system,
            session_id: ctx.params.session_id.clone(),
            participant_human_ids: ctx.params.participant_human_ids.clone(),
            self_human_id: ctx.params.self_human_id.clone(),
        },
        supervisor_cell,
    )
    .await?;
    Ok(listener_ref.get_cell())
}

pub(super) async fn try_restart_source(
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
    let listener_routing = state.mode.listener_routing(state.listener_cell.as_ref());

    let cell = spawn_with_retry(&RETRY_STRATEGY, || {
        let sup = sup.clone();
        let ctx = ctx.clone();
        let recorder_cell = recorder_cell.clone();
        let listener_routing = listener_routing.clone();
        async move {
            let source_ref = spawn_source(sup, &ctx, recorder_cell, listener_routing).await?;
            Ok(source_ref.get_cell())
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

pub(super) async fn try_restart_recorder(
    supervisor_cell: ActorCell,
    state: &mut SessionState,
) -> bool {
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

pub(super) async fn attach_listener_to_source(state: &SessionState) {
    if let Some(source_cell) = &state.source_cell {
        let source_ref: ActorRef<SourceMsg> = source_cell.clone().into();
        if let Err(error) = source_ref.cast(SourceMsg::SetListenerRouting(
            state.mode.listener_routing(state.listener_cell.as_ref()),
        )) {
            tracing::warn!(?error, "failed_to_attach_listener_to_source");
        }
    }
}

pub(super) async fn sync_source_recorder(state: &SessionState) {
    if let Some(source_cell) = &state.source_cell {
        let source_ref: ActorRef<SourceMsg> = source_cell.clone().into();
        let recorder = state.recorder_cell.as_ref().map(|cell| cell.clone().into());
        if let Err(error) = source_ref.cast(SourceMsg::SetRecorder(recorder)) {
            tracing::warn!(?error, "failed_to_update_source_recorder");
        }
    }
}

pub(super) async fn shutdown_children(state: &mut SessionState, reason: &str) {
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

async fn stop_child(cell: &ActorCell, reason: &str, child: &str) {
    if let Err(error) = cell
        .stop_and_wait(Some(reason.to_string()), Some(CHILD_STOP_TIMEOUT))
        .await
    {
        tracing::warn!(?error, %child, "child_stop_and_wait_failed");
    }
}
