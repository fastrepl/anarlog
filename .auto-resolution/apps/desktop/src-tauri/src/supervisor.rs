use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use hypr_supervisor::dynamic::{DynamicSupervisor, DynamicSupervisorMsg, DynamicSupervisorOptions};
use ractor::ActorRef;
use ractor::concurrency::Duration;

pub type SupervisorRef = ActorRef<DynamicSupervisorMsg>;
pub type SupervisorHandle = tokio::task::JoinHandle<()>;

const ROOT_SUPERVISOR_NAME: &str = "root_supervisor";

#[derive(Clone)]
pub struct RootSupervisorContext {
    pub supervisor: SupervisorRef,
    pub is_exiting: Arc<AtomicBool>,
}

impl RootSupervisorContext {
    pub fn mark_exiting(&self) {
        self.is_exiting.store(true, Ordering::SeqCst);
    }

    pub fn stop(&self) {
        self.supervisor.stop(Some("app_exit".to_string()));
    }
}

pub async fn spawn_root_supervisor() -> Option<(RootSupervisorContext, SupervisorHandle)> {
    let options = DynamicSupervisorOptions {
        max_children: Some(10),
        max_restarts: 50,
        max_window: Duration::from_secs(60),
        reset_after: Some(Duration::from_secs(30)),
    };

    match DynamicSupervisor::spawn(ROOT_SUPERVISOR_NAME.to_string(), options).await {
        Ok((supervisor_ref, handle)) => {
            tracing::info!("root_supervisor_spawned");

            let ctx = RootSupervisorContext {
                supervisor: supervisor_ref,
                is_exiting: Arc::new(AtomicBool::new(false)),
            };

            Some((ctx, handle))
        }
        Err(e) => {
            tracing::error!("failed_to_spawn_root_supervisor: {:?}", e);
            None
        }
    }
}

pub fn monitor_supervisor<R: tauri::Runtime>(
    handle: SupervisorHandle,
    is_exiting: Arc<AtomicBool>,
    app_handle: tauri::AppHandle<R>,
) {
    tokio::spawn(async move {
        match handle.await {
            Ok(()) => {
                if !is_exiting.load(Ordering::SeqCst) {
                    tracing::error!("root_supervisor_meltdown");
                    app_handle.restart();
                }
            }
            Err(e) => {
                if !is_exiting.load(Ordering::SeqCst) {
                    tracing::error!("root_supervisor_panicked: {:?}", e);
                    app_handle.restart();
                }
            }
        }
    });
}
