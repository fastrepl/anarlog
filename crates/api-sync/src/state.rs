use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::Semaphore;

use crate::config::{ReplicaConfig, SyncConfig};
use crate::live_docs::LiveDocs;

const UPSTREAM_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const ATTACHMENT_VERIFICATION_CONCURRENCY: usize = 1;
const WITNESS_WRITE_CONCURRENCY: usize = 4;
const WITNESS_WRITE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);

type WitnessWorkspaces = Arc<Mutex<HashMap<String, (bool, std::time::Instant)>>>;

#[derive(Clone)]
pub(crate) struct WitnessWrites {
    slots: Arc<Semaphore>,
    workspaces: WitnessWorkspaces,
}

impl Default for WitnessWrites {
    fn default() -> Self {
        Self {
            slots: Arc::new(Semaphore::new(WITNESS_WRITE_CONCURRENCY)),
            workspaces: Arc::default(),
        }
    }
}

impl WitnessWrites {
    pub(crate) fn try_acquire(&self, workspace_id: &str) -> Option<WitnessWritePermit> {
        let slot = Arc::clone(&self.slots).try_acquire_owned().ok()?;
        let now = std::time::Instant::now();
        let mut workspaces = self.workspaces.lock().unwrap();
        workspaces.retain(|_, (active, deadline)| *active || *deadline > now);
        if workspaces.contains_key(workspace_id) {
            return None;
        }
        workspaces.insert(
            workspace_id.to_string(),
            (true, now + WITNESS_WRITE_INTERVAL),
        );
        Some(WitnessWritePermit {
            _slot: slot,
            workspaces: Arc::clone(&self.workspaces),
            workspace_id: workspace_id.to_string(),
        })
    }
}

pub(crate) struct WitnessWritePermit {
    _slot: tokio::sync::OwnedSemaphorePermit,
    workspaces: WitnessWorkspaces,
    workspace_id: String,
}

impl Drop for WitnessWritePermit {
    fn drop(&mut self) {
        if let Some((active, _)) = self.workspaces.lock().unwrap().get_mut(&self.workspace_id) {
            *active = false;
        }
    }
}

/// Instance-local wake channels for witness long-polls. Publishes on this
/// instance wake waiters immediately; cross-instance publishes are covered by
/// the long-poll's periodic storage recheck.
#[derive(Clone, Default)]
pub struct WitnessWakes {
    inner: Arc<Mutex<HashMap<String, Arc<tokio::sync::Notify>>>>,
}

impl WitnessWakes {
    pub(crate) fn subscribe(&self, workspace_id: &str) -> Arc<tokio::sync::Notify> {
        Arc::clone(
            self.inner
                .lock()
                .unwrap()
                .entry(workspace_id.to_string())
                .or_default(),
        )
    }

    pub(crate) fn notify(&self, workspace_id: &str) {
        if let Some(notify) = self.inner.lock().unwrap().get(workspace_id) {
            notify.notify_waiters();
        }
    }
}

#[derive(Clone)]
pub struct ReplicaState {
    pub(crate) config: ReplicaConfig,
    pub(crate) client: reqwest::Client,
    pub(crate) witness_wakes: WitnessWakes,
    pub(crate) witness_writes: WitnessWrites,
}

impl ReplicaState {
    pub fn new(config: ReplicaConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(UPSTREAM_REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("encrypted replica HTTP client must build");

        Self {
            config,
            client,
            witness_wakes: WitnessWakes::default(),
            witness_writes: WitnessWrites::default(),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub config: SyncConfig,
    pub(crate) replica: ReplicaState,
    pub client: reqwest::Client,
    pub storage: anlg_supabase_storage::SupabaseStorage,
    pub attachment_verification_slots: Arc<Semaphore>,
    pub(crate) live_docs: LiveDocs,
}

impl AppState {
    pub fn new(config: SyncConfig) -> Self {
        let replica = ReplicaState::new(config.replica_config());
        let client = replica.client.clone();
        let storage = anlg_supabase_storage::SupabaseStorage::new(
            client.clone(),
            &config.supabase_url,
            &config.supabase_service_role_key,
        );

        Self {
            config,
            replica,
            client,
            storage,
            attachment_verification_slots: Arc::new(Semaphore::new(
                ATTACHMENT_VERIFICATION_CONCURRENCY,
            )),
            live_docs: LiveDocs::default(),
        }
    }
}
