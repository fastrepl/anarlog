use std::{
    collections::{HashMap, HashSet},
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
    field, target,
    transport::{Transport, failure, text, uuid},
};
use crate::product::services::{Action, Mutation};

#[derive(Default)]
pub(super) struct Sync {
    pub hook: Arc<E2eeSyncHook>,
    task: Mutex<Option<ReplicaSyncTask>>,
    watch: Mutex<Option<WitnessWatchTask>>,
}

impl Sync {
    pub async fn stop(&self) {
        self.hook.clear();
        self.task.lock().await.take();
        self.watch.lock().await.take();
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
            .await?
            .ok_or_else(|| {
                failure("Set up encryption, import your recovery key, or approve this device")
            })?;
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
        if result.is_err() {
            self.stop().await;
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
            Err(failure(
                "Approval requested. Approve this device on an enrolled device, then retry",
            ))
        }
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
