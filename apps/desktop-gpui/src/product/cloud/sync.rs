use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};

use anlg_db_app::{CloudsyncWorkspaceProjection, CloudsyncWorkspaceProjectionEntry};
use anlg_db_sync::{
    E2eeSyncHook, E2eeWitnessCancellation, E2eeWitnessClient, E2eeWitnessConfig, ReplicaSyncTask,
    WitnessWatchTask,
};
use anlg_e2ee::{
    DeviceEnrollmentKey, DeviceEnrollmentPackage, RecoveryKey, WorkspaceKey, WorkspaceKeyGrant,
    WorkspaceKeyring,
};
use desktop_runtime::{Result, Services};
use reqwest::Method;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use super::{
    Core, account,
    auth::{Auth, AuthLease},
    calls::{SyncBlock, SyncCommand, SyncHealth, SyncState, SyncStatus},
    field, target,
    transport::{Transport, failure, text, uuid},
};
use crate::product::services::{Action, Mutation};

const SETUP_REQUIRED: &str = "Set up encryption, import your recovery key, or approve this device";
const KEYCHAIN: &str = "macOS could not access your recovery key";
const DEVICE_LIMIT: &str =
    "Your plan's sync device limit is reached. Replace or remove another device to sync here.";
const UPDATE_REQUIRED: &str = "This version can no longer sync";
const APPROVAL: &str = "Approval requested. Approve this device on an enrolled device, then retry";
const LOG_LIMIT: usize = 200;

#[derive(Default)]
pub(super) struct Sync {
    pub hook: Arc<E2eeSyncHook>,
    task: Mutex<Option<ReplicaSyncTask>>,
    watch: Mutex<Option<WitnessWatchTask>>,
    /// Why sync last failed to start on this device, for the Sync page.
    block: std::sync::Mutex<Option<SyncBlock>>,
    log: std::sync::Mutex<VecDeque<String>>,
}

impl Sync {
    pub async fn stop(&self) {
        self.hook.clear();
        self.task.lock().await.take();
        self.watch.lock().await.take();
    }

    fn set_block(&self, block: Option<SyncBlock>) {
        *self.block.lock().unwrap_or_else(|error| error.into_inner()) = block;
    }

    fn record(&self, level: &str, message: &str) {
        let mut log = self.log.lock().unwrap_or_else(|error| error.into_inner());
        log.push_front(format!(
            "{} {level} cloudsync: {message}",
            chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        ));
        log.truncate(LOG_LIMIT);
    }

    pub async fn enabled(&self, auth: &Auth, account: &str) -> Result<bool> {
        Ok(auth
            .store
            .read(auth.key(&format!("{account}:sync-enabled")))
            .await?
            .as_deref()
            .map(String::as_str)
            == Some("true"))
    }

    async fn set_enabled(&self, auth: &Auth, account: &str, enabled: bool) -> Result<()> {
        auth.store
            .write(
                auth.key(&format!("{account}:sync-enabled")),
                zeroize::Zeroizing::new(enabled.to_string()),
            )
            .await
    }

    pub async fn recovery(&self, auth: &Auth, account: &str) -> Result<RecoveryKey> {
        let code = auth
            .store
            .read(auth.key(&format!("{account}:recovery")))
            .await
            .map_err(|_| failure(KEYCHAIN))?
            .ok_or_else(|| failure(SETUP_REQUIRED))?;
        RecoveryKey::parse(&code).map_err(|_| failure("Stored recovery key is invalid"))
    }

    async fn save_recovery(&self, auth: &Auth, account: &str, key: &RecoveryKey) -> Result<()> {
        let slot = auth.key(&format!("{account}:recovery"));
        if let Some(existing) = auth.store.read(slot.clone()).await? {
            let existing = RecoveryKey::parse(&existing).map_err(|_| {
                failure("Existing recovery key is damaged; export a backup before replacing")
            })?;
            if existing.key_id() != key.key_id() {
                return Err(failure(
                    "A different recovery key already exists. It was not replaced",
                ));
            }
        }
        auth.store.write(slot, key.expose_code()).await
    }

    async fn credentials(
        &self,
        core: &Core,
        lease: &AuthLease,
        recovery: &RecoveryKey,
    ) -> Result<Value> {
        let config = &core.auth.transport.config;
        let identity = recovery
            .member_identity_key()
            .map_err(|_| failure("Invalid encryption identity"))?;
        let response = core
            .auth
            .transport
            .send(
                Method::POST,
                config
                    .api
                    .join("/sync/replica/credentials")
                    .map_err(|_| failure("Invalid sync endpoint"))?,
                Some(lease.access_token()?),
                None,
                &[
                    ("x-anarlog-e2ee-key-id", recovery.key_id()),
                    ("x-anarlog-e2ee-member-public-key", identity.public_key()),
                    ("x-anarlog-cloudsync-transports", "replica".into()),
                    ("x-device-fingerprint", config.device_fingerprint.clone()),
                    ("x-anarlog-device-name", config.device_name.clone()),
                ],
            )
            .await?;
        lease.check()?;
        if response.status == reqwest::StatusCode::FORBIDDEN
            && response.body["error"]["code"] == "sync_device_limit_reached"
        {
            return Err(failure(DEVICE_LIMIT));
        }
        if response.status == reqwest::StatusCode::UPGRADE_REQUIRED {
            return Err(failure(UPDATE_REQUIRED));
        }
        let value = Transport::checked(response)?;
        if text(&value, "transport")? != "replica"
            || text(&value, "accountUserId")? != account(lease)?
            || text(&value, "workspaceId")? != account(lease)?
            || text(&value, "encryptionKeyId")? != recovery.key_id()
            || value["encryptionVersion"].as_u64() != Some(2)
        {
            return Err(failure(
                "Sync credentials do not match this account and encryption key",
            ));
        }
        Ok(value)
    }

    pub async fn start(&self, core: &Core, services: &Services, lease: &AuthLease) -> Result<()> {
        self.stop().await;
        let result = self.configure(core, services, lease).await;
        match &result {
            Ok(()) => {
                self.set_block(None);
                self.record("INFO", "encrypted sync started");
            }
            Err(desktop_runtime::ServiceError::Cancelled) => self.stop().await,
            Err(error) => {
                self.stop().await;
                self.set_block(Some(block_for(error)));
                self.record("WARN", &format!("sync could not start: {error}"));
            }
        }
        result
    }

    async fn configure(&self, core: &Core, services: &Services, lease: &AuthLease) -> Result<()> {
        let owner = account(lease)?;
        let recovery = self.recovery(&core.auth, owner).await?;
        let mut credentials = self.credentials(core, lease, &recovery).await?;
        if self
            .provision_keys(core, lease, &recovery, &credentials)
            .await?
        {
            credentials = self.credentials(core, lease, &recovery).await?;
        }
        let projection = projection(&credentials, owner)?;
        let shared = keyrings(&credentials, &recovery, &projection)?;
        anlg_db_app::claim_cloudsync_workspace_cancellable(services.db.pool(), owner, || {
            lease.cancelled.is_cancelled()
        })
        .await
        .map_err(|_| {
            failure("Library belongs to another account. Connect this local library explicitly")
        })?;
        lease.check()?;
        anlg_db_app::stage_cloudsync_workspace_reconciliation_cancellable(
            services.db.pool(),
            &projection,
            || lease.cancelled.is_cancelled(),
        )
        .await
        .map_err(|_| failure("Could not stage workspace changes; local data was retained"))?;
        anlg_db_app::commit_cloudsync_workspace_projection_cancellable(
            services.db.pool(),
            &projection,
            false,
            || lease.cancelled.is_cancelled(),
        )
        .await
        .map_err(|_| failure("Could not persist workspace membership"))?;
        self.hook
            .set_workspaces(owner, &recovery, shared)
            .map_err(|_| failure("Invalid workspace key"))?;
        let cancellation = E2eeWitnessCancellation::default();
        let operation = async {
            let keys = self.hook.snapshot();
            let mut witnesses = HashMap::new();
            for workspace in keys.keys() {
                let endpoint = core
                    .auth
                    .transport
                    .config
                    .api
                    .join(&format!("/sync/e2ee/witness/{workspace}"))
                    .map_err(|_| failure("Invalid witness endpoint"))?;
                let witness = E2eeWitnessClient::new(
                    E2eeWitnessConfig {
                        endpoint: endpoint.to_string(),
                        access_token: lease.access_token()?.to_owned(),
                    },
                    workspace,
                )
                .map_err(|_| failure("Could not configure encrypted sync"))?;
                witnesses.insert(workspace.clone(), witness);
            }
            self.hook
                .prepare_local_snapshot(services.db.pool(), &cancellation)
                .await
                .map_err(|_| failure("Could not prepare encrypted local snapshot"))?;
            for (workspace, keyring) in &keys {
                witnesses[workspace]
                    .initialize_keyring_cancellable(services.db.pool(), keyring, &cancellation)
                    .await
                    .map_err(|_| {
                        failure("Could not reconcile encrypted workspace; local changes retained")
                    })?;
            }
            lease.check()?;
            self.hook.set_replica_witnesses(witnesses);
            *self.task.lock().await = Some(anlg_db_sync::spawn_replica_sync(
                services.db.clone(),
                self.hook.clone(),
            ));
            *self.watch.lock().await = Some(anlg_db_sync::spawn_witness_watch(
                services.db.clone(),
                self.hook.clone(),
            ));
            self.hook.request_replica_sync();
            Ok(())
        };
        tokio::pin!(operation);
        tokio::select! {
            result = &mut operation => result,
            _ = lease.cancelled.cancelled() => {
                cancellation.cancel();
                let _ = operation.await;
                self.stop().await;
                Err(desktop_runtime::ServiceError::Cancelled)
            }
        }
    }

    async fn provision_keys(
        &self,
        core: &Core,
        lease: &AuthLease,
        recovery: &RecoveryKey,
        credentials: &Value,
    ) -> Result<bool> {
        let owner = account(lease)?;
        let identity = recovery
            .member_identity_key()
            .map_err(|_| failure("Invalid member identity"))?;
        let mut published = false;
        for workspace in credentials["workspaces"].as_array().into_iter().flatten() {
            if workspace["kind"] != "shared"
                || !matches!(workspace["role"].as_str(), Some("owner" | "admin"))
            {
                continue;
            }
            let id = uuid(text(workspace, "id")?)?;
            let recipients = core
                .api(
                    lease,
                    Method::GET,
                    &format!("/sync/e2ee/workspaces/{id}/recipients"),
                    None,
                )
                .await?;
            let recipients = recipients
                .as_array()
                .filter(|rows| !rows.is_empty())
                .ok_or_else(|| failure("Invalid key recipients"))?;
            if !recipients.iter().any(|row| row["userId"] == owner) {
                return Err(failure("Issuer missing from workspace"));
            }
            let active = credentials["workspaceKeyGrants"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|grant| grant["workspaceId"] == id && grant["isActive"] == true);
            if active.is_none()
                && recipients
                    .iter()
                    .any(|recipient| recipient["publicKey"].is_null())
            {
                return Err(failure(
                    "Waiting for workspace members to set up encryption",
                ));
            }
            if let Some(active) = active
                && recipients
                    .iter()
                    .filter(|row| !row["publicKey"].is_null())
                    .all(|row| {
                        row["grantedKeyIds"]
                            .as_array()
                            .is_some_and(|ids| ids.contains(&active["keyId"]))
                    })
            {
                continue;
            }
            let key = match active {
                Some(grant) => identity
                    .open_workspace_key(id, owner, &parse_grant(grant)?)
                    .map_err(|_| failure("Cannot open existing workspace key"))?,
                None => WorkspaceKey::generate()
                    .map_err(|_| failure("Cannot generate workspace key"))?,
            };
            let mut grants = Vec::new();
            for recipient in recipients {
                if recipient["publicKey"].is_null() {
                    continue;
                }
                let user = uuid(text(recipient, "userId")?)?;
                let grant = anlg_e2ee::seal_workspace_key_for_member(
                    &key,
                    text(recipient, "publicKey")?,
                    id,
                    user,
                )
                .map_err(|_| failure("Cannot encrypt workspace key for member"))?;
                grants.push(
                    json!({"userId": user, "ephemeralPublicKey": grant.ephemeral_public_key,
                    "nonce": grant.nonce, "ciphertext": grant.ciphertext}),
                );
            }
            let response = core
                .api(
                    lease,
                    Method::PUT,
                    &format!("/sync/e2ee/workspaces/{id}/key"),
                    Some(&json!({"keyId": key.key_id(), "grants": grants})),
                )
                .await?;
            if text(&response, "keyId")? != key.key_id() {
                return Err(failure("Workspace key publication mismatch"));
            }
            published = true;
        }
        Ok(published)
    }

    pub async fn action(
        &self,
        core: &Core,
        services: &Services,
        lease: &AuthLease,
        mutation: &Mutation,
    ) -> Result<()> {
        let owner = account(lease)?;
        match mutation.operation.action {
            Action::PauseSync => {
                self.stop().await;
                self.set_enabled(&core.auth, owner, false).await?;
            }
            Action::ResumeSync => {
                self.start(core, services, lease).await?;
                self.set_enabled(&core.auth, owner, true).await?;
            }
            Action::SyncNow => {
                if !self.hook.replica_transport_configured() {
                    self.start(core, services, lease).await?;
                }
                self.hook.request_replica_sync();
            }
            Action::SetupEncryption => {
                if core
                    .auth
                    .store
                    .read(core.auth.key(&format!("{owner}:recovery")))
                    .await?
                    .is_some()
                {
                    return Err(failure("Encryption is already configured"));
                }
                let slot = core.auth.key(&format!("{owner}:candidate-recovery"));
                let key = match core.auth.store.read(slot.clone()).await? {
                    Some(code) => RecoveryKey::parse(&code)
                        .map_err(|_| failure("Invalid pending recovery key"))?,
                    None => {
                        let key = RecoveryKey::generate()
                            .map_err(|_| failure("Secure randomness unavailable"))?;
                        core.auth
                            .store
                            .write(slot.clone(), key.expose_code())
                            .await?;
                        key
                    }
                };
                let identity = core
                    .api(
                        lease,
                        Method::PUT,
                        "/sync/e2ee/identity",
                        Some(&json!({"keyId": key.key_id()})),
                    )
                    .await?;
                if identity["keyId"] != key.key_id() {
                    return Err(failure(
                        "Account already has encryption. Import its recovery key or enroll this device",
                    ));
                }
                self.save_recovery(&core.auth, owner, &key).await?;
                core.auth.store.remove(slot).await?;
            }
            Action::ImportRecoveryKey => {
                let key = RecoveryKey::parse(field(mutation, "recovery_key")?)
                    .map_err(|_| failure("Invalid recovery key"))?;
                let identity = core
                    .api(
                        lease,
                        Method::PUT,
                        "/sync/e2ee/identity",
                        Some(&json!({"keyId": key.key_id()})),
                    )
                    .await?;
                if identity["keyId"] != key.key_id() {
                    return Err(failure("Recovery key does not match this account"));
                }
                self.save_recovery(&core.auth, owner, &key).await?;
            }
            Action::CopyRecoveryKey | Action::ExportRecoveryKey => {
                let key = self.recovery(&core.auth, owner).await?;
                if mutation.operation.action == Action::CopyRecoveryKey {
                    core.host.copy_text(key.expose_code()).await?;
                } else {
                    core.host.export_recovery(key.expose_code()).await?;
                }
            }
            Action::RepairKeychain => {
                self.recovery(&core.auth, owner).await?;
            }
            Action::ConnectLocalLibrary => {
                let binding = anlg_db_app::ensure_cloudsync_workspace_binding(services.db.pool())
                    .await
                    .map_err(|_| failure("Could not read library identity"))?;
                anlg_db_app::connect_local_library(services.db.pool(), owner, &binding)
                    .await
                    .map_err(|_| {
                        failure("Could not connect local library; existing notes retained")
                    })?;
            }
            Action::ReplaceDevice => {
                self.enroll(core, lease, mutation.operation.target_id.as_deref())
                    .await?
            }
            Action::ApproveDevice => {
                let id = uuid(target(mutation)?)?;
                let list = core.api(lease, Method::GET, "/sync/devices", None).await?;
                let device = list["pendingDevices"]
                    .as_array()
                    .and_then(|rows| rows.iter().find(|row| row["requestId"] == id))
                    .ok_or_else(|| failure("Pending device no longer exists"))?;
                let key = self.recovery(&core.auth, owner).await?;
                let package = anlg_e2ee::seal_recovery_key_for_device(
                    &key,
                    text(device, "publicKey")?,
                    owner,
                    id,
                )
                .map_err(|_| failure("Could not seal device approval"))?;
                core.api(
                    lease,
                    Method::POST,
                    &format!("/sync/e2ee/device-enrollments/{id}/seal"),
                    Some(
                        &serde_json::to_value(package)
                            .map_err(|_| failure("Invalid enrollment package"))?,
                    ),
                )
                .await?;
            }
            Action::RenameDevice | Action::RemoveDevice => {
                let fingerprint = target(mutation)?;
                if !(8..=128).contains(&fingerprint.len())
                    || !fingerprint
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                {
                    return Err(failure("Invalid device fingerprint"));
                }
                if mutation.operation.action == Action::RemoveDevice {
                    core.api(
                        lease,
                        Method::DELETE,
                        &format!("/sync/devices/{fingerprint}"),
                        None,
                    )
                    .await?;
                    if fingerprint == core.auth.transport.config.device_fingerprint {
                        self.stop().await;
                        self.set_enabled(&core.auth, owner, false).await?;
                    }
                } else {
                    core.api(
                        lease,
                        Method::PATCH,
                        &format!("/sync/devices/{fingerprint}"),
                        Some(&json!({"deviceName": field(mutation, "device_name")?})),
                    )
                    .await?;
                }
            }
            _ => return Err(failure("Invalid sync action")),
        }
        Ok(())
    }

    pub async fn enroll(
        &self,
        core: &Core,
        lease: &AuthLease,
        replace: Option<&str>,
    ) -> Result<()> {
        let owner = account(lease)?;
        let slot = core.auth.key(&format!("{owner}:enrollment"));
        let key = match core.auth.store.read(slot.clone()).await? {
            Some(code) => {
                DeviceEnrollmentKey::parse(&code).map_err(|_| failure("Invalid enrollment key"))?
            }
            None => {
                let key = DeviceEnrollmentKey::generate()
                    .map_err(|_| failure("Secure randomness unavailable"))?;
                core.auth
                    .store
                    .write(slot.clone(), key.expose_code())
                    .await?;
                key
            }
        };
        let result = core
            .api(
                lease,
                Method::POST,
                "/sync/e2ee/device-enrollments",
                Some(&json!({"publicKey": key.public_key(), "replaceFingerprint": replace})),
            )
            .await?;
        let id = uuid(text(&result, "requestId")?)?;
        if result["status"] == "sealed" {
            let package: DeviceEnrollmentPackage =
                serde_json::from_value(result["package"].clone())
                    .map_err(|_| failure("Invalid sealed enrollment"))?;
            let recovery = key
                .open_recovery_key(owner, id, &package)
                .map_err(|_| failure("Device approval failed authentication"))?;
            self.save_recovery(&core.auth, owner, &recovery).await?;
            core.api(
                lease,
                Method::POST,
                &format!("/sync/e2ee/device-enrollments/{id}/consume"),
                Some(&json!({"publicKey": key.public_key()})),
            )
            .await?;
            core.auth.store.remove(slot).await?;
            Ok(())
        } else {
            Err(failure(APPROVAL))
        }
    }

    /// The Sync page's view of this device: preference, block, status, health and log.
    pub async fn state(
        &self,
        core: &Core,
        services: &Services,
        lease: &AuthLease,
    ) -> Result<SyncState> {
        let mut block = self
            .block
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let enabled = match self.enabled(&core.auth, account(lease)?).await {
            Ok(enabled) => enabled,
            Err(_) => {
                block = Some(SyncBlock::KeychainAccess);
                true
            }
        };
        let replica = self.hook.replica_status();
        Ok(SyncState {
            enabled,
            block,
            status: SyncStatus {
                configured: self.hook.replica_transport_configured(),
                running: self.task.lock().await.is_some(),
                last_sync_at_ms: replica.last_sync_at_ms,
                has_unsent_changes: replica.pending_changes,
                consecutive_failures: replica.consecutive_failures,
                activity_paused: self.hook.activity_paused(),
            },
            health: health(services).await,
            fingerprint: core
                .auth
                .transport
                .config
                .device_fingerprint
                .as_str()
                .into(),
            log: self
                .log
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .iter()
                .cloned()
                .collect(),
        })
    }

    /// The Sync page's commands, with shipping's ordering: enabling checks the recovery key
    /// first, and a device change retries a blocked start.
    pub async fn command(
        &self,
        core: &Core,
        services: &Services,
        lease: &AuthLease,
        command: SyncCommand,
    ) -> Result<Option<zeroize::Zeroizing<String>>> {
        let owner = account(lease)?;
        match command {
            SyncCommand::SetEnabled(false) => {
                self.stop().await;
                self.set_enabled(&core.auth, owner, false).await?;
                self.set_block(None);
                self.record("INFO", "sync paused");
            }
            SyncCommand::SetEnabled(true) => {
                if let Err(error) = self.recovery(&core.auth, owner).await {
                    let block = block_for(&error);
                    self.set_block(Some(block.clone()));
                    // A missing key opens the setup dialog rather than failing the switch.
                    if block == SyncBlock::SetupRequired {
                        return Ok(None);
                    }
                    return Err(error);
                }
                self.start(core, services, lease).await?;
                self.set_enabled(&core.auth, owner, true).await?;
            }
            SyncCommand::SyncNow => {
                if !self.hook.replica_transport_configured() {
                    self.start(core, services, lease).await?;
                }
                self.hook.request_replica_sync();
                self.record("INFO", "manual sync requested");
            }
            SyncCommand::CreateRecoveryKey => {
                let slot = core.auth.key(&format!("{owner}:candidate-recovery"));
                let key = match core.auth.store.read(slot.clone()).await? {
                    Some(code) => RecoveryKey::parse(&code)
                        .map_err(|_| failure("Invalid pending recovery key"))?,
                    None => {
                        let key = RecoveryKey::generate()
                            .map_err(|_| failure("Secure randomness unavailable"))?;
                        core.auth.store.write(slot, key.expose_code()).await?;
                        key
                    }
                };
                return Ok(Some(key.expose_code()));
            }
            SyncCommand::UseRecoveryKey(code) => {
                let key =
                    RecoveryKey::parse(code.trim()).map_err(|_| failure("Invalid recovery key"))?;
                let claimed = core
                    .api(
                        lease,
                        Method::PUT,
                        "/sync/e2ee/identity",
                        Some(&json!({"keyId": key.key_id()})),
                    )
                    .await;
                let conflict = || {
                    failure(
                        "This account already uses another recovery key. Use the key from your first device.",
                    )
                };
                match claimed {
                    Ok(identity) if identity["keyId"] == key.key_id() => {}
                    Ok(_) | Err(desktop_runtime::ServiceError::Conflict) => return Err(conflict()),
                    Err(_) => return Err(failure("Could not protect this account. Try again.")),
                }
                self.save_recovery(&core.auth, owner, &key).await?;
                core.auth
                    .store
                    .remove(core.auth.key(&format!("{owner}:candidate-recovery")))
                    .await?;
                self.set_block(None);
                self.record("INFO", "recovery key saved on this device");
            }
            SyncCommand::DownloadRecoveryKey(code) => core.host.export_recovery(code).await?,
            SyncCommand::RepairKeychain => {
                self.recovery(&core.auth, owner).await?;
                self.set_block(None);
                if self.enabled(&core.auth, owner).await? {
                    self.start(core, services, lease).await?;
                }
            }
            SyncCommand::RenameDevice { fingerprint, name } => {
                let name = name.trim();
                if name.is_empty() || name.len() > 128 {
                    return Err(failure("Enter a device name up to 128 bytes"));
                }
                core.api(
                    lease,
                    Method::PATCH,
                    &format!("/sync/devices/{}", device(&fingerprint)?),
                    Some(&json!({"deviceName": name})),
                )
                .await?;
            }
            SyncCommand::RemoveDevice(fingerprint) => {
                core.api(
                    lease,
                    Method::DELETE,
                    &format!("/sync/devices/{}", device(&fingerprint)?),
                    None,
                )
                .await?;
                self.record("INFO", "device disconnected");
                if fingerprint == core.auth.transport.config.device_fingerprint {
                    self.stop().await;
                    self.set_enabled(&core.auth, owner, false).await?;
                } else if self.blocked_by(&SyncBlock::DeviceLimit) {
                    self.start(core, services, lease).await?;
                }
            }
            SyncCommand::ReplaceDevice(fingerprint) => {
                match self.enroll(core, lease, Some(device(&fingerprint)?)).await {
                    Ok(()) => {}
                    Err(error) => {
                        if error.to_string().contains(APPROVAL) {
                            self.set_block(Some(SyncBlock::ApprovalPending));
                        }
                        return Err(error);
                    }
                }
                self.start(core, services, lease).await?;
            }
        }
        Ok(None)
    }

    fn blocked_by(&self, block: &SyncBlock) -> bool {
        self.block
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            == Some(block)
    }
}

fn block_for(error: &desktop_runtime::ServiceError) -> SyncBlock {
    let message = error.to_string();
    if message.contains(SETUP_REQUIRED) {
        SyncBlock::SetupRequired
    } else if message.contains(KEYCHAIN) {
        SyncBlock::KeychainAccess
    } else if message.contains(DEVICE_LIMIT) {
        SyncBlock::DeviceLimit
    } else if message.contains(UPDATE_REQUIRED) {
        SyncBlock::UpdateRequired
    } else {
        SyncBlock::ActivationFailed(message)
    }
}

fn device(fingerprint: &str) -> Result<&str> {
    if !(8..=128).contains(&fingerprint.len())
        || !fingerprint
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(failure("Invalid device fingerprint"));
    }
    Ok(fingerprint)
}

/// `SyncHealthSection`'s counts: notes with an unresolved version from another device, and
/// encrypted records this build parked.
async fn health(services: &Services) -> SyncHealth {
    let pool = services.db.pool();
    let conflicted_notes = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(DISTINCT row_id) FROM e2ee_field_conflicts WHERE resolved_at IS NULL \
         AND ((table_name = 'session_documents' AND field_name = 'body') \
         OR (table_name = 'sessions' AND field_name = 'title'))",
    )
    .fetch_one(pool)
    .await
    .unwrap_or_default();
    let parked = sqlx::query_as::<_, (String, i64)>(
        "SELECT reason, COUNT(*) FROM e2ee_parked_records GROUP BY reason",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let count = |reasons: &[&str]| {
        parked
            .iter()
            .filter(|(reason, _)| reasons.contains(&reason.as_str()))
            .map(|(_, count)| *count)
            .sum::<i64>() as u32
    };
    SyncHealth {
        conflicted_notes: conflicted_notes as u32,
        pending_update: count(&["unknown_table", "unknown_field"]),
        too_large: count(&["too_large"]),
    }
}

fn projection(value: &Value, account: &str) -> Result<CloudsyncWorkspaceProjection> {
    if value["personalWorkspaceId"]
        .as_str()
        .is_some_and(|id| id != account)
    {
        return Err(failure("Invalid personal workspace"));
    }
    let mut workspaces = Vec::new();
    for row in value["workspaces"].as_array().into_iter().flatten() {
        workspaces.push(CloudsyncWorkspaceProjectionEntry {
            id: uuid(text(row, "id")?)?.into(),
            owner_user_id: uuid(text(row, "ownerUserId")?)?.into(),
            kind: text(row, "kind")?.into(),
            name: row["name"].as_str().unwrap_or_default().into(),
            membership_id: uuid(text(row, "membershipId")?)?.into(),
            role: text(row, "role")?.into(),
            membership_created_at: text(row, "membershipCreatedAt")?.into(),
            membership_updated_at: text(row, "membershipUpdatedAt")?.into(),
            created_at: text(row, "createdAt")?.into(),
            updated_at: text(row, "updatedAt")?.into(),
        });
    }
    let projection = CloudsyncWorkspaceProjection {
        account_user_id: account.into(),
        personal_workspace_id: account.into(),
        workspaces,
    };
    anlg_db_app::validate_cloudsync_workspace_projection(&projection)
        .map_err(|_| failure("Invalid workspace projection"))?;
    Ok(projection)
}

fn parse_grant(value: &Value) -> Result<WorkspaceKeyGrant> {
    Ok(WorkspaceKeyGrant {
        key_id: text(value, "keyId")?.into(),
        ephemeral_public_key: text(value, "ephemeralPublicKey")?.into(),
        nonce: text(value, "nonce")?.into(),
        ciphertext: text(value, "ciphertext")?.into(),
    })
}

fn keyrings(
    value: &Value,
    recovery: &RecoveryKey,
    projection: &CloudsyncWorkspaceProjection,
) -> Result<HashMap<String, WorkspaceKeyring>> {
    let identity = recovery
        .member_identity_key()
        .map_err(|_| failure("Invalid member identity"))?;
    let shared: HashSet<_> = projection
        .workspaces
        .iter()
        .filter(|workspace| workspace.kind == "shared")
        .map(|workspace| workspace.id.as_str())
        .collect();
    let mut generations: HashMap<String, Vec<(bool, WorkspaceKey)>> = HashMap::new();
    for grant in value["workspaceKeyGrants"].as_array().into_iter().flatten() {
        let workspace = text(grant, "workspaceId")?;
        if !shared.contains(workspace) {
            return Err(failure("Key grant targets unavailable workspace"));
        }
        let key = identity
            .open_workspace_key(workspace, &projection.account_user_id, &parse_grant(grant)?)
            .map_err(|_| failure("Workspace key grant failed authentication"))?;
        generations
            .entry(workspace.into())
            .or_default()
            .push((grant["isActive"] == true, key));
    }
    let mut result = HashMap::new();
    for workspace in shared {
        let mut keys = generations
            .remove(workspace)
            .ok_or_else(|| failure("Waiting for workspace encryption approval"))?;
        if keys.iter().filter(|(active, _)| *active).count() != 1 {
            return Err(failure("Invalid workspace key generations"));
        }
        let index = keys
            .iter()
            .position(|(active, _)| *active)
            .ok_or_else(|| failure("Missing active workspace key"))?;
        let mut ring = WorkspaceKeyring::new(keys.remove(index).1);
        for (_, key) in keys {
            ring.insert_retired(key);
        }
        result.insert(workspace.into(), ring);
    }
    Ok(result)
}
