use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;

use anlg_desktop_db_runtime::{
    CloudsyncE2eeWitness, CloudsyncWorkspaceKeyGrant, CloudsyncWorkspaceProjection,
    CloudsyncWorkspaceProjectionEntry, DesktopDbRuntime, QueryEventSink,
    cloudsync_config::{
        E2eeSecretReader, E2eeSecretWriter, create_e2ee_recovery_code, import_e2ee_recovery_key,
        inspect_e2ee_recovery_key, load_e2ee_recovery_key,
    },
};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};

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
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
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
    state: Mutex<State>,
    generation: AtomicU64,
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
        Self {
            runtime,
            auth,
            http: reqwest::Client::new(),
            state: Mutex::new(State {
                status: CloudsyncStatus::Off,
                block: None,
            }),
            generation: AtomicU64::new(0),
            timer: Mutex::new(None),
            handle,
            app_id: app_id.into(),
        }
    }

    pub fn state(&self) -> State {
        self.state.lock().expect("cloudsync state poisoned").clone()
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

    fn cancel_timer(&self) {
        if let Some(timer) = self.timer.lock().expect("cloudsync timer poisoned").take() {
            timer.abort();
        }
    }

    fn schedule(self: &Arc<Self>, generation: u64, delay: Duration, enabled: bool) {
        let service = Arc::clone(self);
        let task = self.handle.spawn(async move {
            tokio::time::sleep(delay).await;
            if service.generation.load(Ordering::SeqCst) == generation {
                service
                    .timer
                    .lock()
                    .expect("cloudsync timer poisoned")
                    .take();
                let _ = service.activate_with_enabled(enabled).await;
            }
        });
        *self.timer.lock().expect("cloudsync timer poisoned") = Some(task);
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

    pub async fn finish_e2ee_setup(self: &Arc<Self>, code: &str) -> anyhow::Result<()> {
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
        self.activate().await
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

    pub async fn activate(self: &Arc<Self>) -> anyhow::Result<()> {
        self.activate_with_enabled(true).await
    }

    pub async fn activate_with_enabled(self: &Arc<Self>, enabled: bool) -> anyhow::Result<()> {
        self.cancel_timer();
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let Some(session) = self.auth.session().map_err(anyhow::Error::msg)? else {
            self.runtime.suspend_cloudsync_for_sign_out().await?;
            *self.state.lock().expect("cloudsync state poisoned") = State {
                status: CloudsyncStatus::Off,
                block: None,
            };
            return Ok(());
        };
        if !enabled {
            self.runtime.suspend_cloudsync().await?;
            *self.state.lock().expect("cloudsync state poisoned") = State {
                status: CloudsyncStatus::Off,
                block: None,
            };
            return Ok(());
        }
        let api_url = Self::api_url()?;
        let Some(user) = session.user.as_ref() else {
            anyhow::bail!("authenticated session has no user");
        };
        let secrets = GpuiE2eeSecrets {
            app_id: self.app_id.clone(),
        };
        let recovery = load_e2ee_recovery_key(&secrets, &user.id)
            .await
            .map_err(anyhow::Error::msg)?;
        let Some(recovery) = recovery else {
            self.runtime.suspend_cloudsync().await?;
            *self.state.lock().expect("cloudsync state poisoned") = State {
                status: CloudsyncStatus::Blocked,
                block: Some(CredentialBlock::SetupRequired),
            };
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
                    self.runtime.suspend_cloudsync().await?;
                    *self.state.lock().expect("cloudsync state poisoned") = State {
                        status: CloudsyncStatus::Blocked,
                        block: Some(block),
                    };
                    return Ok(());
                }
                self.schedule(generation, Duration::from_millis(RETRY_DELAY_MS), enabled);
                return Ok(());
            }
        };
        if generation != self.generation.load(Ordering::SeqCst) {
            return Ok(());
        }
        if credentials.encryption_key_id() != recovery.key_id() {
            self.runtime.suspend_cloudsync().await?;
            *self.state.lock().expect("cloudsync state poisoned") = State {
                status: CloudsyncStatus::Blocked,
                block: Some(CredentialBlock::IdentityMismatch),
            };
            return Ok(());
        }
        let expires_at_ms = match chrono::DateTime::parse_from_rfc3339(credentials.expires_at())
            .ok()
            .and_then(|date| date.timestamp_millis().try_into().ok())
        {
            Some(expires_at_ms) => expires_at_ms,
            None => {
                self.schedule(generation, Duration::from_millis(RETRY_DELAY_MS), enabled);
                return Ok(());
            }
        };
        if credentials.account_user_id() != user.id {
            self.runtime.suspend_cloudsync().await?;
            *self.state.lock().expect("cloudsync state poisoned") = State {
                status: CloudsyncStatus::Blocked,
                block: Some(CredentialBlock::IdentityMismatch),
            };
            return Ok(());
        }
        let result = match credentials {
            CredentialResponse::Replica(credentials) => {
                let witness_workspace_id = credentials
                    .personal_workspace_id
                    .clone()
                    .unwrap_or_else(|| credentials.workspace_id.clone());
                let projection = credentials.personal_workspace_id.map(|personal| {
                    CloudsyncWorkspaceProjection {
                        account_user_id: credentials.account_user_id.clone(),
                        personal_workspace_id: personal,
                        workspaces: credentials
                            .workspaces
                            .unwrap_or_default()
                            .into_iter()
                            .map(Into::into)
                            .collect(),
                    }
                });
                self.runtime
                    .configure_e2ee_replica_with_keys(
                        &secrets,
                        credentials.workspace_id,
                        CloudsyncE2eeWitness {
                            endpoint: format!(
                                "{api}/sync/e2ee/witness/{id}",
                                api = api_url,
                                id = witness_workspace_id
                            ),
                            access_token: session.access_token.clone(),
                        },
                        projection,
                        Some(
                            credentials
                                .workspace_key_grants
                                .unwrap_or_default()
                                .into_iter()
                                .map(Into::into)
                                .collect(),
                        ),
                    )
                    .await
            }
            CredentialResponse::E2ee(credentials) => {
                let projection = CloudsyncWorkspaceProjection {
                    account_user_id: credentials.account_user_id.clone(),
                    personal_workspace_id: credentials.personal_workspace_id.clone(),
                    workspaces: credentials.workspaces.into_iter().map(Into::into).collect(),
                };
                self.runtime
                    .configure_cloudsync_token_with_keys(
                        &secrets,
                        credentials.database_id,
                        credentials.token,
                        credentials.account_user_id,
                        Some(projection),
                        Some(
                            credentials
                                .workspace_key_grants
                                .into_iter()
                                .map(Into::into)
                                .collect(),
                        ),
                        CloudsyncE2eeWitness {
                            endpoint: format!(
                                "{api}/sync/e2ee/witness/{id}",
                                api = api_url,
                                id = credentials.personal_workspace_id
                            ),
                            access_token: session.access_token.clone(),
                        },
                    )
                    .await
            }
            CredentialResponse::Legacy(credentials) => {
                self.runtime
                    .configure_cloudsync_token_with_keys(
                        &secrets,
                        credentials.database_id,
                        credentials.token,
                        credentials.workspace_id.clone(),
                        None,
                        None,
                        CloudsyncE2eeWitness {
                            endpoint: format!(
                                "{api}/sync/e2ee/witness/{id}",
                                api = api_url,
                                id = credentials.workspace_id
                            ),
                            access_token: session.access_token.clone(),
                        },
                    )
                    .await
            }
        };
        if let Err(error) = result {
            self.schedule(generation, Duration::from_millis(RETRY_DELAY_MS), enabled);
            return Err(anyhow::Error::msg(error));
        }
        if let Err(error) = self.runtime.start_cloudsync().await {
            self.schedule(generation, Duration::from_millis(RETRY_DELAY_MS), enabled);
            return Err(error.into());
        }
        *self.state.lock().expect("cloudsync state poisoned") = State {
            status: CloudsyncStatus::Syncing,
            block: None,
        };
        let now_ms = chrono::Utc::now()
            .timestamp_millis()
            .try_into()
            .unwrap_or(0);
        self.schedule(generation, refresh_delay(expires_at_ms, now_ms), enabled);
        Ok(())
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
