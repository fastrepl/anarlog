use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};
use tokio::sync::Semaphore;

use crate::config::{ReplicaConfig, SyncConfig};
use crate::live_docs::LiveDocs;

const UPSTREAM_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const WITNESS_WAKE_PEER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
const WITNESS_WAKE_HMAC_DOMAIN: &[u8] = b"anlg-witness-wake-v1:";
const WITNESS_WAKE_SKEW_SECS: i64 = 60;
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

#[derive(Clone, Default)]
pub(crate) enum PeerWakeSource {
    #[default]
    None,
    Static(Vec<String>),
    FlyDns {
        app_name: String,
        self_ip: Option<String>,
        port: u16,
    },
}

fn derive_wake_signing_key(secret: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(WITNESS_WAKE_HMAC_DOMAIN);
    hasher.update(secret.as_bytes());
    hasher.finalize().into()
}

fn unix_now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        })
}

fn sign_witness_wake(key: &[u8; 32], workspace_id: &str, timestamp_secs: i64) -> (String, String) {
    let ts = timestamp_secs.to_string();
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts 32-byte keys");
    mac.update(ts.as_bytes());
    mac.update(b":");
    mac.update(workspace_id.as_bytes());
    let sig = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
    (ts, sig)
}

/// Wake channels for witness long-polls, with optional HMAC-signed peer fan-out across instances.
#[derive(Clone)]
pub struct WitnessWakes {
    inner: Arc<Mutex<HashMap<String, Arc<tokio::sync::Notify>>>>,
    signing_key: Arc<[u8; 32]>,
    client: reqwest::Client,
    peers: Arc<PeerWakeSource>,
}

impl Default for WitnessWakes {
    fn default() -> Self {
        Self::new("default-dev-secret")
    }
}

impl WitnessWakes {
    pub(crate) fn new(secret: &str) -> Self {
        let client = reqwest::Client::builder()
            .timeout(WITNESS_WAKE_PEER_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("witness wake HTTP client must build");
        Self {
            inner: Arc::default(),
            signing_key: Arc::new(derive_wake_signing_key(secret)),
            client,
            peers: Arc::default(),
        }
    }

    pub(crate) fn with_peers(mut self, peers: PeerWakeSource) -> Self {
        self.peers = Arc::new(peers);
        self
    }

    pub(crate) fn has_peer_fanout(&self) -> bool {
        !matches!(*self.peers, PeerWakeSource::None)
    }

    pub(crate) fn subscribe(&self, workspace_id: &str) -> Arc<tokio::sync::Notify> {
        Arc::clone(
            self.inner
                .lock()
                .unwrap()
                .entry(workspace_id.to_string())
                .or_default(),
        )
    }

    pub(crate) fn notify_local(&self, workspace_id: &str) {
        if let Some(notify) = self.inner.lock().unwrap().get(workspace_id) {
            notify.notify_waiters();
        }
    }

    pub(crate) fn notify(&self, workspace_id: &str) {
        self.notify_local(workspace_id);
        if matches!(*self.peers, PeerWakeSource::None) {
            return;
        }
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let peers = Arc::clone(&self.peers);
        let client = self.client.clone();
        let signing_key = Arc::clone(&self.signing_key);
        let workspace_id = workspace_id.to_string();
        handle.spawn(async move {
            let targets = match &*peers {
                PeerWakeSource::None => return,
                PeerWakeSource::Static(urls) => urls.clone(),
                PeerWakeSource::FlyDns {
                    app_name,
                    self_ip,
                    port,
                } => {
                    let Ok(addrs) =
                        tokio::net::lookup_host((format!("{app_name}.internal"), *port)).await
                    else {
                        return;
                    };
                    let mut origins = Vec::new();
                    for addr in addrs {
                        let ip = addr.ip();
                        if self_ip
                            .as_deref()
                            .is_some_and(|self_ip| self_ip == ip.to_string())
                        {
                            continue;
                        }
                        let origin = match ip {
                            std::net::IpAddr::V4(v4) => format!("http://{v4}:{port}"),
                            std::net::IpAddr::V6(v6) => format!("http://[{v6}]:{port}"),
                        };
                        if !origins.contains(&origin) {
                            origins.push(origin);
                        }
                    }
                    origins
                }
            };
            if targets.is_empty() {
                return;
            }
            let (ts, sig) = sign_witness_wake(&signing_key, &workspace_id, unix_now_secs());
            let body = serde_json::json!({ "workspaceId": workspace_id });
            let requests = targets.into_iter().map(|origin| {
                let url = format!("{origin}/internal/e2ee/witness/wake");
                client
                    .post(url)
                    .header("x-anlg-wake-ts", &ts)
                    .header("x-anlg-wake-sig", &sig)
                    .json(&body)
                    .send()
            });
            let _ = futures_util::future::join_all(requests).await;
        });
    }

    pub(crate) fn verify_wake(&self, workspace_id: &str, ts_str: &str, sig_str: &str) -> bool {
        let ts_trimmed = ts_str.trim();
        let Ok(ts) = ts_trimmed.parse::<i64>() else {
            return false;
        };
        if (unix_now_secs() - ts).abs() > WITNESS_WAKE_SKEW_SECS {
            return false;
        }
        let Ok(sig_bytes) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(sig_str.trim())
        else {
            return false;
        };
        let mut mac =
            Hmac::<Sha256>::new_from_slice(&*self.signing_key).expect("HMAC accepts 32-byte keys");
        mac.update(ts_trimmed.as_bytes());
        mac.update(b":");
        mac.update(workspace_id.as_bytes());
        mac.verify_slice(&sig_bytes).is_ok()
    }
}

#[derive(Clone)]
pub struct ReplicaState {
    pub(crate) config: ReplicaConfig,
    pub(crate) client: reqwest::Client,
    pub(crate) storage: anlg_supabase_storage::SupabaseStorage,
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
        let storage = anlg_supabase_storage::SupabaseStorage::new(
            client.clone(),
            &config.supabase_url,
            &config.supabase_service_role_key,
        );
        let witness_wakes = WitnessWakes::new(&config.supabase_service_role_key);

        Self {
            config,
            client,
            storage,
            witness_wakes,
            witness_writes: WitnessWrites::default(),
        }
    }

    pub fn with_wake_peers(mut self, peers: Vec<String>) -> Self {
        let normalized = peers
            .into_iter()
            .map(|origin| origin.trim_end_matches('/').to_string())
            .filter(|origin| !origin.is_empty())
            .collect();
        self.witness_wakes = self
            .witness_wakes
            .with_peers(PeerWakeSource::Static(normalized));
        self
    }

    pub fn with_fly_wake_discovery(
        mut self,
        app_name: impl Into<String>,
        self_ip: Option<String>,
        port: u16,
    ) -> Self {
        self.witness_wakes = self.witness_wakes.with_peers(PeerWakeSource::FlyDns {
            app_name: app_name.into(),
            self_ip,
            port,
        });
        self
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
        let storage = replica.storage.clone();

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

    pub fn with_fly_wake_discovery(
        mut self,
        app_name: impl Into<String>,
        self_ip: Option<String>,
        port: u16,
    ) -> Self {
        self.replica = self
            .replica
            .with_fly_wake_discovery(app_name, self_ip, port);
        self
    }

    /// The Supabase-backed half of sync, shared with the credential routes so
    /// witness wakes and write admission stay per process.
    pub fn replica_state(&self) -> ReplicaState {
        self.replica.clone()
    }
}
