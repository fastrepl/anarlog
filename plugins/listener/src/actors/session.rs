use std::path::PathBuf;
use std::time::{Instant, SystemTime};

use ractor::concurrency::Duration;
use ractor::{Actor, ActorCell, ActorProcessingErr};
use ractor_supervisor::SupervisorStrategy;
use ractor_supervisor::core::{ChildBackoffFn, ChildSpec, Restart, SpawnFn};
use ractor_supervisor::supervisor::{Supervisor, SupervisorArguments, SupervisorOptions};

use crate::actors::{
    ChannelMode, ListenerActor, ListenerArgs, RecArgs, RecorderActor, RecorderSink, SourceActor,
    SourceArgs,
};

pub const SESSION_SUPERVISOR_PREFIX: &str = "session_supervisor_";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SessionParams {
    pub session_id: String,
    pub languages: Vec<hypr_language::Language>,
    pub onboarding: bool,
    pub record_enabled: bool,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub keywords: Vec<String>,
}

#[derive(Clone)]
pub struct SessionContext {
    pub app: tauri::AppHandle,
    pub params: SessionParams,
    pub app_dir: PathBuf,
    pub started_at_instant: Instant,
    pub started_at_system: SystemTime,
}

pub fn session_supervisor_name(session_id: &str) -> String {
    format!("{}{}", SESSION_SUPERVISOR_PREFIX, session_id)
}

fn make_supervisor_options() -> SupervisorOptions {
    SupervisorOptions {
        strategy: SupervisorStrategy::RestForOne,
        max_restarts: 3,
        max_window: Duration::from_secs(15),
        reset_after: Some(Duration::from_secs(30)),
    }
}

fn make_listener_backoff() -> ChildBackoffFn {
    ChildBackoffFn::new(|_id, count, _, _| {
        if count == 0 {
            None
        } else {
            Some(Duration::from_millis(500))
        }
    })
}

pub async fn spawn_session_supervisor(
    ctx: SessionContext,
) -> Result<(ActorCell, tokio::task::JoinHandle<()>), ActorProcessingErr> {
    let supervisor_name = session_supervisor_name(&ctx.params.session_id);

    let mut child_specs = Vec::new();

    let ctx_source = ctx.clone();
    child_specs.push(ChildSpec {
        id: SourceActor::name().to_string(),
        restart: Restart::Permanent,
        spawn_fn: SpawnFn::new(move |supervisor_cell, _id| {
            let ctx = ctx_source.clone();
            async move {
                let (actor_ref, _) = Actor::spawn_linked(
                    Some(SourceActor::name()),
                    SourceActor,
                    SourceArgs {
                        mic_device: None,
                        onboarding: ctx.params.onboarding,
                        app: ctx.app.clone(),
                        session_id: ctx.params.session_id.clone(),
                    },
                    supervisor_cell,
                )
                .await?;
                Ok(actor_ref.get_cell())
            }
        }),
        backoff_fn: None,
        reset_after: Some(Duration::from_secs(30)),
    });

    let ctx_listener = ctx.clone();
    child_specs.push(ChildSpec {
        id: ListenerActor::name().to_string(),
        restart: Restart::Permanent,
        spawn_fn: SpawnFn::new(move |supervisor_cell, _id| {
            let ctx = ctx_listener.clone();
            async move {
                let mode = ChannelMode::determine(ctx.params.onboarding);

                let (actor_ref, _) = Actor::spawn_linked(
                    Some(ListenerActor::name()),
                    ListenerActor,
                    ListenerArgs {
                        app: ctx.app.clone(),
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
                    },
                    supervisor_cell,
                )
                .await?;
                Ok(actor_ref.get_cell())
            }
        }),
        backoff_fn: Some(make_listener_backoff()),
        reset_after: Some(Duration::from_secs(30)),
    });

    if ctx.params.record_enabled {
        let ctx_recorder = ctx.clone();
        child_specs.push(ChildSpec {
            id: RecorderActor::name().to_string(),
            restart: Restart::Transient,
            spawn_fn: SpawnFn::new(move |supervisor_cell, _id| {
                let ctx = ctx_recorder.clone();
                async move {
                    let (actor_ref, _) = Actor::spawn_linked(
                        Some(RecorderActor::name()),
                        RecorderActor,
                        RecArgs {
                            app_dir: ctx.app_dir.clone(),
                            session_id: ctx.params.session_id.clone(),
                            sink: RecorderSink::File,
                        },
                        supervisor_cell,
                    )
                    .await?;
                    Ok(actor_ref.get_cell())
                }
            }),
            backoff_fn: None,
            reset_after: None,
        });
    }

    let args = SupervisorArguments {
        child_specs,
        options: make_supervisor_options(),
    };

    let (supervisor_ref, handle) = Supervisor::spawn(supervisor_name, args).await?;

    Ok((supervisor_ref.get_cell(), handle))
}
