use hypr_supervisor::{
    RestartPolicy,
    dynamic::{
        ChildBackoffFn, DynChildSpec, DynSpawnFn, DynamicSupervisor, DynamicSupervisorMsg,
        DynamicSupervisorOptions, SupervisorError,
    },
};
use ractor::{ActorCell, ActorProcessingErr, ActorRef, concurrency::Duration, registry};

#[cfg(feature = "whisper-cpp")]
use super::internal::{InternalSTTActor, InternalSTTArgs};
#[cfg(target_arch = "aarch64")]
use super::internal2::{Internal2STTActor, Internal2STTArgs};
use super::{
    ServerType,
    external::{ExternalSTTActor, ExternalSTTArgs},
};

pub type SupervisorRef = ActorRef<DynamicSupervisorMsg>;

pub const INTERNAL_STT_ACTOR_NAME: &str = "internal_stt";
#[cfg(target_arch = "aarch64")]
pub const INTERNAL2_STT_ACTOR_NAME: &str = "internal2_stt";
pub const EXTERNAL_STT_ACTOR_NAME: &str = "external_stt";
pub const SUPERVISOR_NAME: &str = "stt_supervisor";

fn make_supervisor_options() -> DynamicSupervisorOptions {
    DynamicSupervisorOptions {
        max_children: Some(1),
        max_restarts: 100,
        max_window: Duration::from_secs(60 * 3),
        reset_after: Some(Duration::from_secs(30)),
    }
}

pub async fn spawn_stt_supervisor(
    parent: Option<ActorCell>,
) -> Result<(ActorRef<DynamicSupervisorMsg>, crate::SupervisorHandle), ActorProcessingErr> {
    let options = make_supervisor_options();

    let (supervisor_ref, handle) =
        DynamicSupervisor::spawn(SUPERVISOR_NAME.to_string(), options).await?;

    if let Some(parent_cell) = parent {
        supervisor_ref.get_cell().link(parent_cell);
    }

    Ok((supervisor_ref, handle))
}

#[cfg(feature = "whisper-cpp")]
pub async fn start_internal_stt(
    supervisor: &ActorRef<DynamicSupervisorMsg>,
    args: InternalSTTArgs,
) -> Result<(), ActorProcessingErr> {
    let child_spec = create_internal_child_spec_with_args(args);
    DynamicSupervisor::spawn_child(supervisor.clone(), child_spec).await
}

#[cfg(target_arch = "aarch64")]
pub async fn start_internal2_stt(
    supervisor: &ActorRef<DynamicSupervisorMsg>,
    args: Internal2STTArgs,
) -> Result<(), ActorProcessingErr> {
    let child_spec = create_internal2_child_spec_with_args(args);
    DynamicSupervisor::spawn_child(supervisor.clone(), child_spec).await
}

pub async fn start_external_stt(
    supervisor: &ActorRef<DynamicSupervisorMsg>,
    args: ExternalSTTArgs,
) -> Result<(), ActorProcessingErr> {
    let child_spec = create_external_child_spec_with_args(args);
    DynamicSupervisor::spawn_child(supervisor.clone(), child_spec).await
}

#[cfg(feature = "whisper-cpp")]
fn create_internal_child_spec_with_args(args: InternalSTTArgs) -> DynChildSpec {
    let spawn_fn = DynSpawnFn::new(move |supervisor: ActorCell, child_id: String| {
        let args = args.clone();
        async move {
            let (actor_ref, _handle) =
                DynamicSupervisor::spawn_linked(child_id, InternalSTTActor, args, supervisor)
                    .await?;
            Ok(actor_ref.get_cell())
        }
    });

    DynChildSpec {
        id: INTERNAL_STT_ACTOR_NAME.to_string(),
        spawn_fn,
        restart: RestartPolicy::Transient,
        backoff_fn: Some(ChildBackoffFn::new(|_, _, _, _| {
            Some(Duration::from_millis(500))
        })),
        reset_after: None,
    }
}

#[cfg(target_arch = "aarch64")]
fn create_internal2_child_spec_with_args(args: Internal2STTArgs) -> DynChildSpec {
    let spawn_fn = DynSpawnFn::new(move |supervisor: ActorCell, child_id: String| {
        let args = args.clone();
        async move {
            let (actor_ref, _handle) =
                DynamicSupervisor::spawn_linked(child_id, Internal2STTActor, args, supervisor)
                    .await?;
            Ok(actor_ref.get_cell())
        }
    });

    DynChildSpec {
        id: INTERNAL2_STT_ACTOR_NAME.to_string(),
        spawn_fn,
        restart: RestartPolicy::Transient,
        backoff_fn: Some(ChildBackoffFn::new(|_, _, _, _| {
            Some(Duration::from_millis(500))
        })),
        reset_after: None,
    }
}

fn create_external_child_spec_with_args(args: ExternalSTTArgs) -> DynChildSpec {
    let spawn_fn = DynSpawnFn::new(move |supervisor: ActorCell, child_id: String| {
        let args = args.clone();
        async move {
            let (actor_ref, _handle) =
                DynamicSupervisor::spawn_linked(child_id, ExternalSTTActor, args, supervisor)
                    .await?;
            Ok(actor_ref.get_cell())
        }
    });

    DynChildSpec {
        id: EXTERNAL_STT_ACTOR_NAME.to_string(),
        spawn_fn,
        restart: RestartPolicy::Transient,
        backoff_fn: Some(ChildBackoffFn::new(|_, _, _, _| {
            Some(Duration::from_secs(1))
        })),
        reset_after: None,
    }
}

pub async fn stop_stt_server(
    supervisor: &ActorRef<DynamicSupervisorMsg>,
    server_type: ServerType,
) -> Result<(), ActorProcessingErr> {
    let child_ids: Vec<&str> = match server_type {
        ServerType::Internal => {
            #[cfg(all(target_arch = "aarch64", feature = "whisper-cpp"))]
            {
                vec![INTERNAL2_STT_ACTOR_NAME, INTERNAL_STT_ACTOR_NAME]
            }
            #[cfg(all(target_arch = "aarch64", not(feature = "whisper-cpp")))]
            {
                vec![INTERNAL2_STT_ACTOR_NAME]
            }
            #[cfg(not(target_arch = "aarch64"))]
            {
                vec![]
            }
        }
        ServerType::External => vec![EXTERNAL_STT_ACTOR_NAME],
    };

    for child_id in child_ids {
        let result =
            DynamicSupervisor::terminate_child(supervisor.clone(), child_id.to_string()).await;

        if let Err(e) = result {
            if let Some(supervisor_error) = e.downcast_ref::<SupervisorError>()
                && matches!(supervisor_error, SupervisorError::ChildNotFound { .. })
            {
                continue;
            }
            return Err(e);
        }
    }

    match server_type {
        ServerType::Internal => {
            #[cfg(target_arch = "aarch64")]
            wait_for_actor_shutdown(Internal2STTActor::name()).await;
            #[cfg(feature = "whisper-cpp")]
            wait_for_actor_shutdown(InternalSTTActor::name()).await;
        }
        ServerType::External => wait_for_actor_shutdown(ExternalSTTActor::name()).await,
    }

    Ok(())
}

pub async fn stop_all_stt_servers(
    supervisor: &ActorRef<DynamicSupervisorMsg>,
) -> Result<(), ActorProcessingErr> {
    let _ = stop_stt_server(supervisor, ServerType::Internal).await;
    let _ = stop_stt_server(supervisor, ServerType::External).await;
    Ok(())
}

async fn wait_for_actor_shutdown(actor_name: ractor::ActorName) {
    for _ in 0..50 {
        if registry::where_is(actor_name.clone()).is_none() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}
