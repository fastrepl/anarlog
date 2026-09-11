use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;
use tokio::sync::watch;

use anlg_desktop_db_runtime::{
    CloudsyncE2eeWitness, CloudsyncWorkspaceKeyGrant, CloudsyncWorkspaceProjection,
    CloudsyncWorkspaceProjectionEntry, DesktopDbRuntime, QueryEventSink,
    cloudsync_config::{
        E2eeSecretReader, E2eeSecretWriter, create_e2ee_recovery_code, import_e2ee_recovery_key,
        inspect_e2ee_recovery_key, load_e2ee_recovery_key, open_shared_workspace_keyrings,
        shared_workspace_ids,
    },
    runtime::{CloudsyncTokenConfiguration, E2eeWorkspaceKeyConfiguration},
};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::db::Store;

const API_URL: Option<&str> = option_env!("VITE_API_URL");
const REFRESH_LEAD_MS: u64 = 2 * 60 * 1000;
const RETRY_DELAY_MS: u64 = 60 * 1000;
const MIN_REFRESH_DELAY_MS: u64 = 1000;
const DEVICE_NAME_HEADER: &str = "x-anarlog-device-name";
const E2EE_KEY_ID_HEADER: &str = "X-Anarlog-E2EE-Key-Id";
const MEMBER_KEY_HEADER: &str = "x-anarlog-e2ee-member-public-key";
const TRANSPORTS_HEADER: &str = "x-anarlog-cloudsync-transports";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudsyncStatus {
    Off,
    Syncing,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialBlock {
    ActivationFailed,
    ApprovalPending,
    ClockSkew,
    DeviceLimit,
    IdentityMismatch,
    KeychainAccess,
    NotEntitled,
    ReauthRequired,
    SetupRequired,
    Unavailable,
}

impl CredentialBlock {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ActivationFailed => "activation_failed",
            Self::ApprovalPending => "approval_pending",
            Self::ClockSkew => "clock_skew",
            Self::DeviceLimit => "device_limit",
            Self::IdentityMismatch => "identity_mismatch",
            Self::KeychainAccess => "keychain_access",
            Self::NotEntitled => "not_entitled",
            Self::ReauthRequired => "reauth_required",
            Self::SetupRequired => "setup_required",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub status: CloudsyncStatus,
    pub block: Option<CredentialBlock>,
}

#[derive(Debug)]
struct CredentialBlockError(CredentialBlock);

impl std::fmt::Display for CredentialBlockError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "CloudSync credentials rejected")
    }
}

impl std::error::Error for CredentialBlockError {}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LegacyCredentials {
    pub encryption_version: u8,
    pub encryption_key_id: String,
    pub database_id: String,
    pub token: String,
    pub expires_at: String,
    pub workspace_id: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct E2eeCredentials {
    pub encryption_version: u8,
    pub encryption_key_id: String,
    pub database_id: String,
    pub token: String,
    pub expires_at: String,
    pub workspace_id: String,
    pub account_user_id: String,
    pub personal_workspace_id: String,
    pub workspaces: Vec<Workspace>,
    pub workspace_key_grants: Vec<Grant>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplicaCredentials {
    pub transport: String,
    pub encryption_version: u8,
    pub encryption_key_id: String,
    pub expires_at: String,
    pub workspace_id: String,
    pub account_user_id: String,
    pub personal_workspace_id: Option<String>,
    pub workspaces: Option<Vec<Workspace>>,
    pub workspace_key_grants: Option<Vec<Grant>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum CredentialResponse {
    Replica(ReplicaCredentials),
    E2ee(E2eeCredentials),
    Legacy(LegacyCredentials),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct E2eeIdentityResponse {
    key_id: String,
}

impl CredentialResponse {
    fn encryption_key_id(&self) -> &str {
        match self {
            Self::Replica(credentials) => &credentials.encryption_key_id,
            Self::E2ee(credentials) => &credentials.encryption_key_id,
            Self::Legacy(credentials) => &credentials.encryption_key_id,
        }
    }

    fn expires_at(&self) -> &str {
        match self {
            Self::Replica(credentials) => &credentials.expires_at,
            Self::E2ee(credentials) => &credentials.expires_at,
            Self::Legacy(credentials) => &credentials.expires_at,
        }
    }

    fn account_user_id(&self) -> &str {
        match self {
            Self::Replica(credentials) => &credentials.account_user_id,
            Self::E2ee(credentials) => &credentials.account_user_id,
            Self::Legacy(credentials) => &credentials.workspace_id,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub owner_user_id: String,
    pub kind: String,
    pub name: String,
    pub membership_id: String,
    pub role: String,
    pub membership_created_at: String,
    pub membership_updated_at: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Grant {
    pub workspace_id: String,
    pub key_id: String,
    pub ephemeral_public_key: String,
    pub nonce: String,
    pub ciphertext: String,
    pub is_active: bool,
}

impl From<Workspace> for CloudsyncWorkspaceProjectionEntry {
    fn from(value: Workspace) -> Self {
        Self {
            id: value.id,
            owner_user_id: value.owner_user_id,
            kind: value.kind,
            name: value.name,
            membership_id: value.membership_id,
            role: value.role,
            membership_created_at: value.membership_created_at,
            membership_updated_at: value.membership_updated_at,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<Grant> for CloudsyncWorkspaceKeyGrant {
    fn from(value: Grant) -> Self {
        Self {
            workspace_id: value.workspace_id,
            key_id: value.key_id,
            ephemeral_public_key: value.ephemeral_public_key,
            nonce: value.nonce,
            ciphertext: value.ciphertext,
            is_active: value.is_active,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyProvisioning {
    Ready,
    Provisioned,
    Waiting,
}

#[derive(Debug, Clone)]
struct WorkspaceRecipient {
    user_id: String,
    public_key: Option<String>,
}

fn credentials_expired(expires_at_ms: u64, now_ms: u64) -> bool {
    expires_at_ms <= now_ms
}

fn parse_recipients(
    value: Value,
    account_user_id: &str,
    active_grant: Option<&Grant>,
) -> anyhow::Result<(Vec<WorkspaceRecipient>, bool, bool)> {
    let values = value
        .as_array()
        .filter(|values| !values.is_empty())
        .ok_or_else(|| anyhow!("workspace E2EE recipients are invalid"))?;
    let mut recipient_ids = HashSet::with_capacity(values.len());
    let mut recipients = Vec::with_capacity(values.len());
    let mut waiting_for_identity = false;
    let mut all_granted = true;
    for value in values {
        let recipient = value
            .as_object()
            .ok_or_else(|| anyhow!("workspace E2EE recipients are invalid"))?;
        let user_id = recipient
            .get("userId")
            .and_then(Value::as_str)
            .filter(|user_id| !user_id.is_empty())
            .ok_or_else(|| anyhow!("workspace E2EE recipients are invalid"))?
            .to_string();
        if !recipient_ids.insert(user_id.clone()) {
            return Err(anyhow!("workspace E2EE recipients are invalid"));
        }
        let public_key = match recipient.get("publicKey") {
            Some(Value::Null) => {
                waiting_for_identity = true;
                None
            }
            Some(Value::String(public_key))
                if public_key.len() == 43
                    && public_key.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
                    }) =>
            {
                Some(public_key.clone())
            }
            _ => return Err(anyhow!("workspace E2EE recipients are invalid")),
        };
        let granted_key_ids = recipient
            .get("grantedKeyIds")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("workspace E2EE recipients are invalid"))?
            .iter()
            .map(|key_id| {
                let key_id = key_id
                    .as_str()
                    .filter(|key_id| {
                        key_id.len() == 22
                            && key_id.bytes().all(|byte| {
                                byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
                            })
                    })
                    .ok_or_else(|| anyhow!("workspace E2EE recipients are invalid"))?;
                Ok(key_id.to_string())
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        if active_grant.is_some_and(|grant| !granted_key_ids.iter().any(|id| id == &grant.key_id)) {
            all_granted = false;
        }
        recipients.push(WorkspaceRecipient {
            user_id,
            public_key,
        });
    }
    if !recipient_ids.contains(account_user_id) {
        return Err(anyhow!("workspace E2EE issuer is missing"));
    }
    Ok((recipients, waiting_for_identity, all_granted))
}

pub fn sanitize_device_name(name: Option<&str>) -> Option<String> {
    let ascii: String = name?
        .trim()
        .chars()
        .filter(|character| (' '..='~').contains(character))
        .collect::<String>()
        .trim()
        .chars()
        .take(128)
        .collect();
    (!ascii.is_empty()).then_some(ascii)
}

fn forbidden_credential_block(code: Option<&str>) -> CredentialBlock {
    if code == Some("sync_device_limit_reached") {
        CredentialBlock::DeviceLimit
    } else {
        CredentialBlock::NotEntitled
    }
}

pub fn refresh_delay(expires_at_ms: u64, now_ms: u64) -> Duration {
    let time_until_expiry_ms = expires_at_ms.saturating_sub(now_ms);
    let refresh_lead_ms = REFRESH_LEAD_MS.min(MIN_REFRESH_DELAY_MS.max(time_until_expiry_ms / 5));
    Duration::from_millis(
        MIN_REFRESH_DELAY_MS.max(time_until_expiry_ms.saturating_sub(refresh_lead_ms)),
    )
}

pub struct GpuiE2eeSecrets {
    pub app_id: String,
}

impl E2eeSecretReader for GpuiE2eeSecrets {
    fn read(
        &self,
        scope: &str,
        key: &str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Option<String>, String>> + Send + '_>,
    > {
        let result = crate::secrets::read(&self.app_id, scope, key);
        Box::pin(async move { result })
    }
}

impl E2eeSecretWriter for GpuiE2eeSecrets {
    fn write(
        &self,
        scope: &str,
        key: &str,
        value: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        let result = crate::secrets::write(&self.app_id, scope, key, value);
        Box::pin(async move { result })
    }
}

pub struct Cloudsync<S: QueryEventSink> {
    pub runtime: Arc<DesktopDbRuntime<S>>,
    pub auth: Arc<crate::auth::Auth>,
    pub http: reqwest::Client,
    state_tx: watch::Sender<State>,
    generation: AtomicU64,
    ops: tokio::sync::Mutex<()>,
    timer: Mutex<Option<tokio::task::JoinHandle<()>>>,
    handle: tokio::runtime::Handle,
    app_id: String,
}

impl<S: QueryEventSink> Cloudsync<S> {
    pub fn new(
        runtime: Arc<DesktopDbRuntime<S>>,
        auth: Arc<crate::auth::Auth>,
        handle: tokio::runtime::Handle,
        app_id: impl Into<String>,
    ) -> Self {
        let (state_tx, _) = watch::channel(State {
            status: CloudsyncStatus::Off,
            block: None,
        });
        Self {
            runtime,
            auth,
            http: reqwest::Client::new(),
            state_tx,
            generation: AtomicU64::new(0),
            ops: tokio::sync::Mutex::new(()),
            timer: Mutex::new(None),
            handle,
            app_id: app_id.into(),
        }
    }

    pub fn state(&self) -> State {
        self.state_tx.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<State> {
        self.state_tx.subscribe()
    }

    pub fn start(self: &Arc<Self>, store: Arc<Store>) {
        let service = Arc::clone(self);
        let mut auth_state = service.auth.subscribe();
        self.handle.spawn(async move {
            let enabled = store
                .load_provider_settings()
                .await
                .ok()
                .and_then(|settings| settings.ok())
                .map(|settings| {
                    settings.bool_setting(
                        "cloud_sync_enabled",
                        &["general", "cloud_sync_enabled"],
                        true,
                    )
                })
                .unwrap_or(true);
            if let Err(error) = service.activate_with_enabled(enabled).await {
                tracing::warn!(%error, "failed to activate CloudSync");
            }

            while auth_state.changed().await.is_ok() {
                let enabled = store
                    .load_provider_settings()
                    .await
                    .ok()
                    .and_then(|settings| settings.ok())
                    .map(|settings| {
                        settings.bool_setting(
                            "cloud_sync_enabled",
                            &["general", "cloud_sync_enabled"],
                            true,
                        )
                    })
                    .unwrap_or(true);
                if let Err(error) = service.activate_with_enabled(enabled).await {
                    tracing::warn!(%error, "failed to activate CloudSync");
                }
            }
        });
    }

    fn cancel_timer_locked(&self) {
        if let Some(timer) = self.timer.lock().expect("cloudsync timer poisoned").take() {
            timer.abort();
        }
    }

    fn schedule_locked(self: &Arc<Self>, generation: u64, delay: Duration, enabled: bool) {
        let service = Arc::clone(self);
        let task = self.handle.spawn(async move {
            tokio::time::sleep(delay).await;
            let activate = {
                let _ops = service.ops.lock().await;
                if service.generation.load(Ordering::SeqCst) != generation {
                    false
                } else {
                    service
                        .timer
                        .lock()
                        .expect("cloudsync timer poisoned")
                        .take();
                    true
                }
            };
            if activate {
                let _ = service.activate_with_enabled(enabled).await;
            }
        });
        *self.timer.lock().expect("cloudsync timer poisoned") = Some(task);
    }

    async fn schedule_if_current(
        self: &Arc<Self>,
        generation: u64,
        delay: Duration,
        enabled: bool,
    ) -> bool {
        let _ops = self.ops.lock().await;
        if self.generation.load(Ordering::SeqCst) != generation {
            return false;
        }
        self.schedule_locked(generation, delay, enabled);
        true
    }

    async fn suspend_and_set_state(
        &self,
        generation: u64,
        sign_out: bool,
        state: State,
    ) -> anyhow::Result<bool> {
        let _ops = self.ops.lock().await;
        if self.generation.load(Ordering::SeqCst) != generation {
            return Ok(false);
        }
        if sign_out {
            self.runtime.suspend_cloudsync_for_sign_out().await?;
        } else {
            self.runtime.suspend_cloudsync().await?;
        }
        if self.generation.load(Ordering::SeqCst) != generation {
            return Ok(false);
        }
        self.state_tx.send_replace(state);
        Ok(true)
    }

    fn api_url() -> anyhow::Result<&'static str> {
        API_URL.ok_or_else(|| anyhow!("VITE_API_URL is not configured"))
    }

    fn credential_request(
        &self,
        url: String,
        access_token: &str,
        encryption_key_id: &str,
        member_public_key: &str,
        device_name: Option<&str>,
    ) -> reqwest::RequestBuilder {
        let request = self
            .http
            .post(url)
            .bearer_auth(access_token)
            .header(E2EE_KEY_ID_HEADER, encryption_key_id)
            .header(MEMBER_KEY_HEADER, member_public_key)
            .header(TRANSPORTS_HEADER, "replica")
            .header("x-device-fingerprint", anlg_host::fingerprint());
        match device_name {
            Some(device_name) => request.header(DEVICE_NAME_HEADER, device_name),
            None => request,
        }
    }

    pub async fn claim_e2ee_identity(&self, key_id: &str) -> anyhow::Result<()> {
        let api_url = Self::api_url()?;
        let session = self
            .auth
            .session()
            .map_err(anyhow::Error::msg)?
            .ok_or_else(|| anyhow!("authentication is required"))?;
        let response = self
            .http
            .put(format!("{api_url}/sync/e2ee/identity"))
            .bearer_auth(session.access_token)
            .json(&serde_json::json!({ "keyId": key_id }))
            .send()
            .await?;
        if response.status() == reqwest::StatusCode::CONFLICT {
            anyhow::bail!(
                "This account already uses another recovery key. Use the key from your first device."
            );
        }
        if !response.status().is_success() {
            anyhow::bail!("Could not protect this account. Try again.");
        }
        let identity = response
            .json::<E2eeIdentityResponse>()
            .await
            .map_err(|_| anyhow!("The server returned an invalid key identity."))?;
        if identity.key_id != key_id {
            anyhow::bail!("The server returned an invalid key identity.");
        }
        Ok(())
    }

    pub async fn create_e2ee_recovery_code(&self) -> anyhow::Result<String> {
        let session = self
            .auth
            .session()
            .map_err(anyhow::Error::msg)?
            .ok_or_else(|| anyhow!("authentication is required"))?;
        let Some(user) = session.user.as_ref() else {
            anyhow::bail!("authenticated session has no user");
        };
        let secrets = GpuiE2eeSecrets {
            app_id: self.app_id.clone(),
        };
        if load_e2ee_recovery_key(&secrets, &user.id)
            .await
            .map_err(anyhow::Error::msg)?
            .is_some()
        {
            anyhow::bail!("E2EE recovery key is already configured");
        }
        create_e2ee_recovery_code().map_err(anyhow::Error::msg)
    }

    fn workspace_projection(
        credentials: &CredentialResponse,
    ) -> Option<(Vec<Workspace>, Vec<Grant>)> {
        match credentials {
            CredentialResponse::E2ee(credentials) => Some((
                credentials.workspaces.clone(),
                credentials.workspace_key_grants.clone(),
            )),
            CredentialResponse::Replica(credentials) => {
                credentials.personal_workspace_id.as_ref().map(|_| {
                    (
                        credentials.workspaces.clone().unwrap_or_default(),
                        credentials.workspace_key_grants.clone().unwrap_or_default(),
                    )
                })
            }
            CredentialResponse::Legacy(_) => None,
        }
    }

    async fn provision_missing_workspace_keys(
        &self,
        credentials: &CredentialResponse,
        access_token: &str,
        account_user_id: &str,
    ) -> anyhow::Result<KeyProvisioning> {
        let Some((workspaces, grants)) = Self::workspace_projection(credentials) else {
            return Ok(KeyProvisioning::Ready);
        };
        let active_grants = grants
            .into_iter()
            .filter(|grant| grant.is_active)
            .map(|grant| (grant.workspace_id.clone(), grant))
            .collect::<HashMap<_, _>>();
        let shared_workspaces = workspaces
            .into_iter()
            .filter(|workspace| workspace.kind == "shared")
            .collect::<Vec<_>>();
        if shared_workspaces.is_empty() {
            return Ok(KeyProvisioning::Ready);
        }

        let api_url = Self::api_url()?;
        let secrets = GpuiE2eeSecrets {
            app_id: self.app_id.clone(),
        };
        let mut provisioned = false;
        let mut waiting = false;
        for workspace in shared_workspaces {
            let active_grant = active_grants.get(&workspace.id);
            if workspace.role != "owner" && workspace.role != "admin" {
                waiting |= active_grant.is_none();
                continue;
            }

            let mut recipients_url = url::Url::parse(api_url)?;
            recipients_url
                .path_segments_mut()
                .map_err(|_| anyhow!("API URL cannot contain path segments"))?
                .extend(["sync", "e2ee", "workspaces", &workspace.id, "recipients"]);
            let response = self
                .http
                .get(recipients_url)
                .bearer_auth(access_token)
                .send()
                .await?;
            if !response.status().is_success() {
                return Err(anyhow!("workspace E2EE recipients are unavailable"));
            }
            let value = response.json::<Value>().await?;
            let (recipients, waiting_for_identity, all_granted) =
                parse_recipients(value, account_user_id, active_grant)?;
            if active_grant.is_some() && all_granted {
                continue;
            }
            if waiting_for_identity {
                waiting = true;
                continue;
            }

            let sealed =
                self.runtime
                    .seal_workspace_e2ee_key_for_recipients(
                        &secrets,
                        account_user_id,
                        &workspace.id,
                        recipients
                            .iter()
                            .map(|recipient| {
                                Ok(anlg_desktop_db_runtime::WorkspaceE2eeKeyRecipient {
                                    user_id: recipient.user_id.clone(),
                                    public_key: recipient.public_key.clone().ok_or_else(|| {
                                        anyhow!("workspace E2EE identity is missing")
                                    })?,
                                })
                            })
                            .collect::<anyhow::Result<Vec<_>>>()?,
                        active_grant.is_none(),
                        active_grant.map(|grant| grant.clone().into()),
                    )
                    .await
                    .map_err(anyhow::Error::msg)?;

            let mut publication_url = url::Url::parse(api_url)?;
            publication_url
                .path_segments_mut()
                .map_err(|_| anyhow!("API URL cannot contain path segments"))?
                .extend(["sync", "e2ee", "workspaces", &workspace.id, "key"]);
            let response = self
                .http
                .put(publication_url)
                .bearer_auth(access_token)
                .json(&sealed)
                .send()
                .await?;
            if !response.status().is_success() {
                return Err(anyhow!("workspace E2EE key publication failed"));
            }
            let publication = response.json::<Value>().await?;
            if publication.get("keyId").and_then(Value::as_str) != Some(sealed.key_id.as_str()) {
                return Err(anyhow!(
                    "workspace E2EE key publication response is invalid"
                ));
            }
            provisioned = true;
        }

        Ok(if provisioned {
            KeyProvisioning::Provisioned
        } else if waiting {
            KeyProvisioning::Waiting
        } else {
            KeyProvisioning::Ready
        })
    }

    pub async fn finish_e2ee_setup(
        self: &Arc<Self>,
        code: &str,
        enabled: bool,
    ) -> anyhow::Result<()> {
        let session = self
            .auth
            .session()
            .map_err(anyhow::Error::msg)?
            .ok_or_else(|| anyhow!("authentication is required"))?;
        let Some(user) = session.user.as_ref() else {
            anyhow::bail!("authenticated session has no user");
        };
        let key_id = inspect_e2ee_recovery_key(code).map_err(anyhow::Error::msg)?;
        self.claim_e2ee_identity(&key_id).await?;
        let secrets = GpuiE2eeSecrets {
            app_id: self.app_id.clone(),
        };
        import_e2ee_recovery_key(&secrets, &user.id, code)
            .await
            .map_err(anyhow::Error::msg)?;
        self.activate_with_enabled(enabled).await
    }

    pub async fn request_credentials(
        &self,
        access_token: &str,
        encryption_key_id: &str,
        member_public_key: &str,
    ) -> anyhow::Result<CredentialResponse> {
        let api_url = Self::api_url()?;
        let device_name =
            sysinfo::System::host_name().and_then(|name| sanitize_device_name(Some(&name)));
        let response = self
            .credential_request(
                format!("{api_url}/sync/token"),
                access_token,
                encryption_key_id,
                member_public_key,
                device_name.as_deref(),
            )
            .send()
            .await?;
        let response = if response.status() == reqwest::StatusCode::NOT_FOUND {
            self.credential_request(
                format!("{api_url}/sync/replica/credentials"),
                access_token,
                encryption_key_id,
                member_public_key,
                device_name.as_deref(),
            )
            .send()
            .await?
        } else {
            response
        };
        match response.status() {
            reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::NOT_IMPLEMENTED => {
                return Err(anyhow::Error::new(CredentialBlockError(
                    CredentialBlock::Unavailable,
                )));
            }
            reqwest::StatusCode::UNAUTHORIZED => {
                return Err(anyhow::Error::new(CredentialBlockError(
                    CredentialBlock::ReauthRequired,
                )));
            }
            reqwest::StatusCode::FORBIDDEN => {
                let code = response
                    .json::<serde_json::Value>()
                    .await
                    .ok()
                    .and_then(|value| value["error"]["code"].as_str().map(str::to_string));
                return Err(anyhow::Error::new(CredentialBlockError(
                    forbidden_credential_block(code.as_deref()),
                )));
            }
            status if !status.is_success() => {
                return Err(anyhow!("CloudSync credential exchange returned {status}"));
            }
            _ => {}
        }
        Ok(response.json().await?)
    }

    pub async fn activate_with_enabled(self: &Arc<Self>, enabled: bool) -> anyhow::Result<()> {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        {
            let _ops = self.ops.lock().await;
            self.cancel_timer_locked();
        }
        let Some(session) = self.auth.session().map_err(anyhow::Error::msg)? else {
            self.suspend_and_set_state(
                generation,
                true,
                State {
                    status: CloudsyncStatus::Off,
                    block: None,
                },
            )
            .await?;
            return Ok(());
        };
        if !enabled {
            self.suspend_and_set_state(
                generation,
                false,
                State {
                    status: CloudsyncStatus::Off,
                    block: None,
                },
            )
            .await?;
            return Ok(());
        }
        let Some(user) = session.user.as_ref() else {
            anyhow::bail!("authenticated session has no user");
        };
        {
            let _ops = self.ops.lock().await;
            if self.generation.load(Ordering::SeqCst) != generation {
                return Ok(());
            }
            match self.runtime.bind_cloudsync_account(user.id.clone()).await {
                Ok(true) => {}
                Ok(false) => {
                    if self.generation.load(Ordering::SeqCst) != generation {
                        return Ok(());
                    }
                    self.state_tx.send_replace(State {
                        status: CloudsyncStatus::Blocked,
                        block: Some(CredentialBlock::IdentityMismatch),
                    });
                    return Ok(());
                }
                Err(error) => {
                    self.schedule_locked(
                        generation,
                        Duration::from_millis(RETRY_DELAY_MS),
                        enabled,
                    );
                    tracing::warn!(%error, "failed to bind CloudSync account");
                    return Ok(());
                }
            }
        }
        let api_url = Self::api_url()?;
        let secrets = GpuiE2eeSecrets {
            app_id: self.app_id.clone(),
        };
        let recovery = load_e2ee_recovery_key(&secrets, &user.id)
            .await
            .map_err(anyhow::Error::msg)?;
        let Some(recovery) = recovery else {
            self.suspend_and_set_state(
                generation,
                false,
                State {
                    status: CloudsyncStatus::Blocked,
                    block: Some(CredentialBlock::SetupRequired),
                },
            )
            .await?;
            return Ok(());
        };
        let member_public_key = recovery.member_identity_key()?.public_key();
        let credentials = match self
            .request_credentials(
                &session.access_token,
                &recovery.key_id(),
                &member_public_key,
            )
            .await
        {
            Ok(credentials) => credentials,
            Err(error) => {
                if let Some(block) = error
                    .downcast_ref::<CredentialBlockError>()
                    .map(|error| error.0)
                {
                    self.suspend_and_set_state(
                        generation,
                        false,
                        State {
                            status: CloudsyncStatus::Blocked,
                            block: Some(block),
                        },
                    )
                    .await?;
                    return Ok(());
                }
                self.schedule_if_current(
                    generation,
                    Duration::from_millis(RETRY_DELAY_MS),
                    enabled,
                )
                .await;
                return Ok(());
            }
        };
        if generation != self.generation.load(Ordering::SeqCst) {
            return Ok(());
        }
        if credentials.encryption_key_id() != recovery.key_id() {
            self.suspend_and_set_state(
                generation,
                false,
                State {
                    status: CloudsyncStatus::Blocked,
                    block: Some(CredentialBlock::IdentityMismatch),
                },
            )
            .await?;
            return Ok(());
        }
        let expires_at_ms = match chrono::DateTime::parse_from_rfc3339(credentials.expires_at())
            .ok()
            .and_then(|date| date.timestamp_millis().try_into().ok())
        {
            Some(expires_at_ms) => expires_at_ms,
            None => {
                self.schedule_if_current(
                    generation,
                    Duration::from_millis(RETRY_DELAY_MS),
                    enabled,
                )
                .await;
                return Ok(());
            }
        };
        let now_ms = chrono::Utc::now()
            .timestamp_millis()
            .try_into()
            .unwrap_or(0);
        if credentials_expired(expires_at_ms, now_ms) {
            self.suspend_and_set_state(
                generation,
                false,
                State {
                    status: CloudsyncStatus::Blocked,
                    block: Some(CredentialBlock::ClockSkew),
                },
            )
            .await?;
            self.schedule_if_current(generation, Duration::from_millis(RETRY_DELAY_MS), enabled)
                .await;
            return Ok(());
        }
        if credentials.account_user_id() != user.id {
            self.suspend_and_set_state(
                generation,
                false,
                State {
                    status: CloudsyncStatus::Blocked,
                    block: Some(CredentialBlock::IdentityMismatch),
                },
            )
            .await?;
            return Ok(());
        }
        if Self::workspace_projection(&credentials).is_some() {
            let provisioning = match self
                .provision_missing_workspace_keys(
                    &credentials,
                    &session.access_token,
                    credentials.account_user_id(),
                )
                .await
            {
                Ok(provisioning) => provisioning,
                Err(error) => {
                    if generation != self.generation.load(Ordering::SeqCst) {
                        return Ok(());
                    }
                    self.schedule_if_current(
                        generation,
                        Duration::from_millis(RETRY_DELAY_MS),
                        enabled,
                    )
                    .await;
                    tracing::warn!(%error, "CloudSync workspace key provisioning failed");
                    return Ok(());
                }
            };
            if generation != self.generation.load(Ordering::SeqCst) {
                return Ok(());
            }
            match provisioning {
                KeyProvisioning::Provisioned => {
                    self.schedule_if_current(
                        generation,
                        Duration::from_millis(MIN_REFRESH_DELAY_MS),
                        enabled,
                    )
                    .await;
                    return Ok(());
                }
                KeyProvisioning::Waiting => {
                    self.schedule_if_current(
                        generation,
                        Duration::from_millis(RETRY_DELAY_MS),
                        enabled,
                    )
                    .await;
                    return Ok(());
                }
                KeyProvisioning::Ready => {}
            }
        }
        let workspace_projection = match &credentials {
            CredentialResponse::Replica(credentials) => credentials
                .personal_workspace_id
                .clone()
                .map(|personal_workspace_id| CloudsyncWorkspaceProjection {
                    account_user_id: credentials.account_user_id.clone(),
                    personal_workspace_id,
                    workspaces: credentials
                        .workspaces
                        .clone()
                        .unwrap_or_default()
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                }),
            CredentialResponse::E2ee(credentials) => Some(CloudsyncWorkspaceProjection {
                account_user_id: credentials.account_user_id.clone(),
                personal_workspace_id: credentials.personal_workspace_id.clone(),
                workspaces: credentials
                    .workspaces
                    .clone()
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            }),
            CredentialResponse::Legacy(_) => None,
        };
        let workspace_key_grants = match &credentials {
            CredentialResponse::Replica(credentials) => credentials
                .workspace_key_grants
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            CredentialResponse::E2ee(credentials) => credentials
                .workspace_key_grants
                .clone()
                .into_iter()
                .map(Into::into)
                .collect(),
            CredentialResponse::Legacy(_) => Vec::new(),
        };
        let personal_workspace_id = match &credentials {
            CredentialResponse::Replica(credentials) => credentials
                .personal_workspace_id
                .clone()
                .unwrap_or_else(|| credentials.workspace_id.clone()),
            CredentialResponse::E2ee(credentials) => credentials.personal_workspace_id.clone(),
            CredentialResponse::Legacy(credentials) => credentials.workspace_id.clone(),
        };
        let keyrings = open_shared_workspace_keyrings(
            &recovery,
            &user.id,
            shared_workspace_ids(workspace_projection.as_ref()),
            workspace_key_grants,
        )
        .map_err(anyhow::Error::msg)?;
        let mut workspace_keys = Some(E2eeWorkspaceKeyConfiguration::new(
            personal_workspace_id,
            recovery,
            keyrings,
        ));
        {
            let _ops = self.ops.lock().await;
            if self.generation.load(Ordering::SeqCst) != generation {
                return Ok(());
            }
            let result = match &credentials {
                CredentialResponse::Replica(credentials) => {
                    let witness_workspace_id = credentials
                        .personal_workspace_id
                        .clone()
                        .unwrap_or_else(|| credentials.workspace_id.clone());
                    let runtime_generation = self.runtime.begin_cloudsync_auth_configuration();
                    self.runtime
                        .configure_replica_transport_at_generation(
                            credentials.workspace_id.clone(),
                            CloudsyncE2eeWitness {
                                endpoint: format!(
                                    "{api}/sync/e2ee/witness/{id}",
                                    api = api_url,
                                    id = witness_workspace_id
                                ),
                                access_token: session.access_token.clone(),
                            },
                            workspace_keys
                                .take()
                                .ok_or_else(|| anyhow!("E2EE workspace keys are unavailable"))?,
                            workspace_projection.clone().map(Into::into),
                            runtime_generation,
                        )
                        .await
                        .map_err(|error| error.to_string())
                }
                CredentialResponse::E2ee(credentials) => {
                    let runtime_generation = self.runtime.begin_cloudsync_auth_configuration();
                    self.runtime
                        .configure_cloudsync_token_with_projection_at_generation(
                            CloudsyncTokenConfiguration::new(
                                credentials.database_id.clone(),
                                credentials.token.clone(),
                                credentials.account_user_id.clone(),
                                workspace_projection.clone().map(Into::into),
                                CloudsyncE2eeWitness {
                                    endpoint: format!(
                                        "{api}/sync/e2ee/witness/{id}",
                                        api = api_url,
                                        id = credentials.personal_workspace_id
                                    ),
                                    access_token: session.access_token.clone(),
                                },
                            ),
                            workspace_keys.take(),
                            runtime_generation,
                        )
                        .await
                        .map_err(|error| error.to_string())
                }
                CredentialResponse::Legacy(credentials) => {
                    let runtime_generation = self.runtime.begin_cloudsync_auth_configuration();
                    self.runtime
                        .configure_cloudsync_token_with_projection_at_generation(
                            CloudsyncTokenConfiguration::new(
                                credentials.database_id.clone(),
                                credentials.token.clone(),
                                credentials.workspace_id.clone(),
                                None,
                                CloudsyncE2eeWitness {
                                    endpoint: format!(
                                        "{api}/sync/e2ee/witness/{id}",
                                        api = api_url,
                                        id = credentials.workspace_id
                                    ),
                                    access_token: session.access_token.clone(),
                                },
                            ),
                            workspace_keys.take(),
                            runtime_generation,
                        )
                        .await
                        .map_err(|error| error.to_string())
                }
            };
            let configuration_step = match credentials {
                CredentialResponse::Replica(_) => "configure_replica",
                CredentialResponse::E2ee(_) | CredentialResponse::Legacy(_) => "configure_token",
            };
            self.runtime
                .record_cloudsync_configuration_result(configuration_step, &result);
            match result {
                Err(error) => {
                    self.schedule_locked(
                        generation,
                        Duration::from_millis(RETRY_DELAY_MS),
                        enabled,
                    );
                    return Err(anyhow::Error::msg(error));
                }
                Ok(anlg_desktop_db_runtime::CloudsyncTokenConfigurationResult::AccountMismatch) => {
                    if self.generation.load(Ordering::SeqCst) != generation {
                        return Ok(());
                    }
                    self.state_tx.send_replace(State {
                        status: CloudsyncStatus::Blocked,
                        block: Some(CredentialBlock::IdentityMismatch),
                    });
                    return Ok(());
                }
                Ok(anlg_desktop_db_runtime::CloudsyncTokenConfigurationResult::Configured) => {}
            }
            if self.generation.load(Ordering::SeqCst) != generation {
                if let Err(error) = self.runtime.suspend_cloudsync().await {
                    tracing::debug!(%error, "stale_cloudsync_configuration_suspend_failed");
                }
                tracing::debug!("stale_cloudsync_configuration");
                return Ok(());
            }
            if let Err(error) = self.runtime.start_cloudsync().await {
                self.schedule_locked(generation, Duration::from_millis(RETRY_DELAY_MS), enabled);
                return Err(error.into());
            }
            if self.generation.load(Ordering::SeqCst) != generation {
                if let Err(error) = self.runtime.suspend_cloudsync().await {
                    tracing::debug!(%error, "stale_cloudsync_start_suspend_failed");
                }
                tracing::debug!("stale_cloudsync_start");
                return Ok(());
            }
            self.state_tx.send_replace(State {
                status: CloudsyncStatus::Syncing,
                block: None,
            });
            self.schedule_locked(generation, refresh_delay(expires_at_ms, now_ms), enabled);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_device_names() {
        assert_eq!(
            sanitize_device_name(Some(" \u{1f600} Mac \n")),
            Some("Mac".into())
        );
        assert_eq!(sanitize_device_name(Some(" \u{0} ")), None);
        assert_eq!(
            sanitize_device_name(Some(&"x".repeat(129))).unwrap().len(),
            128
        );
    }

    #[test]
    fn computes_refresh_delay() {
        assert_eq!(
            refresh_delay(1_000, 900),
            Duration::from_millis(MIN_REFRESH_DELAY_MS)
        );
        assert_eq!(
            refresh_delay(2_000_000, 900_000),
            Duration::from_millis(980_000)
        );
    }

    #[test]
    fn expired_credentials_include_the_current_timestamp() {
        assert!(credentials_expired(10, 10));
        assert!(credentials_expired(9, 10));
        assert!(!credentials_expired(11, 10));
    }

    #[test]
    fn recipient_validation_rejects_invalid_shapes_duplicates_and_missing_issuer() {
        let grant = Grant {
            workspace_id: "workspace".into(),
            key_id: "abcdefghijklmnopqrstuv".into(),
            ephemeral_public_key: "key".into(),
            nonce: "nonce".into(),
            ciphertext: "ciphertext".into(),
            is_active: true,
        };
        assert!(parse_recipients(serde_json::json!({}), "issuer", Some(&grant)).is_err());
        assert!(
            parse_recipients(
                serde_json::json!([
                    {"userId": "issuer", "publicKey": null, "grantedKeyIds": []},
                    {"userId": "issuer", "publicKey": null, "grantedKeyIds": []}
                ]),
                "issuer",
                Some(&grant)
            )
            .is_err()
        );
        assert!(
            parse_recipients(
                serde_json::json!([
                    {"userId": "member", "publicKey": null, "grantedKeyIds": []}
                ]),
                "issuer",
                Some(&grant)
            )
            .is_err()
        );
    }

    #[test]
    fn recipient_validation_waits_for_null_identity() {
        let (recipients, waiting, all_granted) = parse_recipients(
            serde_json::json!([
                {
                    "userId": "issuer",
                    "publicKey": null,
                    "grantedKeyIds": []
                }
            ]),
            "issuer",
            None,
        )
        .unwrap();
        assert_eq!(recipients.len(), 1);
        assert!(waiting);
        assert!(all_granted);
    }

    #[test]
    fn recipient_validation_skips_when_everyone_has_the_active_key() {
        let grant = Grant {
            workspace_id: "workspace".into(),
            key_id: "abcdefghijklmnopqrstuv".into(),
            ephemeral_public_key: "key".into(),
            nonce: "nonce".into(),
            ciphertext: "ciphertext".into(),
            is_active: true,
        };
        let (_, waiting, all_granted) = parse_recipients(
            serde_json::json!([
                {
                    "userId": "issuer",
                    "publicKey": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                    "grantedKeyIds": ["abcdefghijklmnopqrstuv"]
                }
            ]),
            "issuer",
            Some(&grant),
        )
        .unwrap();
        assert!(!waiting);
        assert!(all_granted);
    }

    #[test]
    fn maps_credential_blocks() {
        assert_eq!(
            forbidden_credential_block(Some("sync_device_limit_reached")),
            CredentialBlock::DeviceLimit
        );
        assert_eq!(
            forbidden_credential_block(Some("unknown")),
            CredentialBlock::NotEntitled
        );
    }

    #[test]
    fn deserializes_all_credential_variants() {
        let legacy = serde_json::from_str::<CredentialResponse>(
            r#"{"encryptionVersion":2,"encryptionKeyId":"key","databaseId":"db","token":"token","expiresAt":"2025-01-01T00:00:00Z","workspaceId":"workspace"}"#,
        )
        .unwrap();
        assert!(matches!(legacy, CredentialResponse::Legacy(_)));
        let e2ee = serde_json::from_str::<CredentialResponse>(
            r#"{"encryptionVersion":2,"encryptionKeyId":"key","databaseId":"db","token":"token","expiresAt":"2025-01-01T00:00:00Z","workspaceId":"workspace","accountUserId":"user","personalWorkspaceId":"personal","workspaces":[],"workspaceKeyGrants":[]}"#,
        )
        .unwrap();
        assert!(matches!(e2ee, CredentialResponse::E2ee(_)));
        let replica = serde_json::from_str::<CredentialResponse>(
            r#"{"transport":"replica","encryptionVersion":2,"encryptionKeyId":"key","expiresAt":"2025-01-01T00:00:00Z","workspaceId":"workspace","accountUserId":"user"}"#,
        )
        .unwrap();
        assert!(matches!(replica, CredentialResponse::Replica(_)));
    }
}
