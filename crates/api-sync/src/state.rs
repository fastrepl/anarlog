use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::Semaphore;

use crate::config::{ReplicaConfig, SyncConfig};

const UPSTREAM_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const ATTACHMENT_VERIFICATION_CONCURRENCY: usize = 1;

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
        }
    }
}
