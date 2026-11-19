//! The cloud calls the Teams and Sync settings pages make, as the signed-in account.
//! `CloudServices` sends them through the runtime worker; tests and the screens harness
//! answer them from memory.

use std::sync::Arc;

use desktop_runtime::{Result, ServiceError};
use futures::future::BoxFuture;
use gpui::{App, Global};
use reqwest::Method;
use serde_json::Value;
use zeroize::Zeroizing;

use super::{CloudServices, Transport, transport::failure};

pub trait CloudCalls: Send + Sync {
    /// A Supabase RPC; `list_*` directories are paged until complete.
    fn rpc(&self, name: &'static str, body: Value) -> BoxFuture<'static, Result<Value>>;
    /// An Anarlog API request.
    fn api(
        &self,
        method: Method,
        path: String,
        body: Option<Value>,
    ) -> BoxFuture<'static, Result<Value>>;
    /// A PostgREST table read (`/rest/v1/{table}`) under the account's row-level security.
    fn select(
        &self,
        table: &'static str,
        query: Vec<(&'static str, String)>,
    ) -> BoxFuture<'static, Result<Value>>;
    /// Opens an Anarlog web page in the browser.
    fn open_web(
        &self,
        path: &'static str,
        params: Vec<(&'static str, String)>,
        fragment: Option<&'static str>,
    ) -> BoxFuture<'static, Result<()>>;
    fn copy_text(&self, text: Zeroizing<String>) -> BoxFuture<'static, Result<()>>;
    /// The web link a workspace invitation email would carry.
    fn invitation_url(&self, invitation: &str, token: &str) -> Result<String>;
    fn sync_state(&self) -> BoxFuture<'static, Result<SyncState>>;
    /// Runs a Sync page command; creating a recovery key returns it.
    fn sync(&self, command: SyncCommand) -> BoxFuture<'static, Result<Option<Zeroizing<String>>>>;
    /// Reloads the app-wide account after membership changes, like shipping's session refresh.
    fn refresh_account(&self, cx: &mut App);
}

/// The installed cloud calls; absent while the app has no cloud configuration.
#[derive(Clone)]
pub struct AccountCloud(pub Arc<dyn CloudCalls>);

impl Global for AccountCloud {}

pub fn installed(cx: &App) -> Option<Arc<dyn CloudCalls>> {
    cx.try_global::<AccountCloud>().map(|cloud| cloud.0.clone())
}

/// Shipping's `CloudsyncCredentialBlock` states the native sync can reach.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncBlock {
    SetupRequired,
    KeychainAccess,
    DeviceLimit,
    ApprovalPending,
    UpdateRequired,
    ActivationFailed(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncStatus {
    pub configured: bool,
    pub running: bool,
    pub last_sync_at_ms: Option<u64>,
    pub has_unsent_changes: bool,
    pub consecutive_failures: u32,
    pub activity_paused: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncHealth {
    pub conflicted_notes: u32,
    pub pending_update: u32,
    pub too_large: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncState {
    /// The stored `sync-enabled` preference.
    pub enabled: bool,
    pub block: Option<SyncBlock>,
    pub status: SyncStatus,
    pub health: SyncHealth,
    /// This device's sync fingerprint.
    pub fingerprint: Arc<str>,
    /// Recent CloudSync entries, newest first.
    pub log: Vec<String>,
}

#[derive(Clone)]
pub enum SyncCommand {
    SetEnabled(bool),
    SyncNow,
    /// Generates (or reuses) this account's unsaved recovery key and returns it.
    CreateRecoveryKey,
    /// Claims `key` as the account's encryption identity and stores it on this device.
    UseRecoveryKey(Zeroizing<String>),
    DownloadRecoveryKey(Zeroizing<String>),
    RepairKeychain,
    RenameDevice {
        fingerprint: String,
        name: String,
    },
    RemoveDevice(String),
    ReplaceDevice(String),
}

impl CloudServices {
    fn worker<T: Send + 'static>(
        &self,
        run: impl FnOnce(
            CloudServices,
            desktop_runtime::Services,
            Arc<super::AuthLease>,
        ) -> BoxFuture<'static, Result<T>>
        + Send
        + 'static,
    ) -> BoxFuture<'static, Result<T>> {
        let this = self.clone();
        Box::pin(async move {
            this.runtime
                .clone()
                .service(move |services| async move {
                    let account = this
                        .core
                        .auth
                        .identity()
                        .account_id
                        .ok_or_else(|| failure("Sign in to continue"))?;
                    let lease = Arc::new(this.core.auth.lease(&account, false).await?);
                    let result = run(this, services, lease.clone()).await?;
                    lease.check()?;
                    Ok(result)
                })?
                .receive()
                .await
        })
    }

    /// Approves the account's devices waiting for approval, when this device syncs.
    pub fn approve_devices(&self) -> BoxFuture<'static, Result<()>> {
        self.worker(|this, _, lease| {
            Box::pin(async move { this.core.sync.approve_pending(&this.core, &lease).await })
        })
    }
}

impl CloudCalls for CloudServices {
    fn rpc(&self, name: &'static str, body: Value) -> BoxFuture<'static, Result<Value>> {
        self.worker(move |this, _, lease| {
            Box::pin(async move {
                this.core
                    .rpc_with(&lease, name, body, Transport::checked_reason)
                    .await
            })
        })
    }

    fn api(
        &self,
        method: Method,
        path: String,
        body: Option<Value>,
    ) -> BoxFuture<'static, Result<Value>> {
        self.worker(move |this, _, lease| {
            Box::pin(async move { this.core.api(&lease, method, &path, body.as_ref()).await })
        })
    }

    fn select(
        &self,
        table: &'static str,
        query: Vec<(&'static str, String)>,
    ) -> BoxFuture<'static, Result<Value>> {
        self.worker(move |this, _, lease| {
            Box::pin(async move {
                let transport = &this.core.auth.transport;
                let mut url = transport
                    .config
                    .supabase
                    .join(&format!("/rest/v1/{table}"))
                    .map_err(|_| failure("Invalid table route"))?;
                url.query_pairs_mut().extend_pairs(query);
                let response = transport
                    .send(
                        Method::GET,
                        url,
                        Some(lease.access_token()?),
                        None,
                        &[("apikey", transport.config.anon_key.clone())],
                    )
                    .await?;
                Transport::checked_reason(response)
            })
        })
    }

    fn open_web(
        &self,
        path: &'static str,
        params: Vec<(&'static str, String)>,
        fragment: Option<&'static str>,
    ) -> BoxFuture<'static, Result<()>> {
        CloudServices::open_web(self, path, params, fragment)
    }

    fn copy_text(&self, text: Zeroizing<String>) -> BoxFuture<'static, Result<()>> {
        self.core.host.copy_text(text)
    }

    fn invitation_url(&self, invitation: &str, token: &str) -> Result<String> {
        self.core
            .capability_url(&format!("/team/invite/{invitation}/"), token)
    }

    fn sync_state(&self) -> BoxFuture<'static, Result<SyncState>> {
        self.worker(move |this, services, lease| {
            Box::pin(async move { this.core.sync.state(&this.core, &services, &lease).await })
        })
    }

    fn sync(&self, command: SyncCommand) -> BoxFuture<'static, Result<Option<Zeroizing<String>>>> {
        self.worker(move |this, services, lease| {
            Box::pin(async move {
                let _guard = this
                    .core
                    .mutations
                    .try_lock()
                    .map_err(|_| ServiceError::Busy)?;
                this.core
                    .sync
                    .command(&this.core, &services, &lease, command)
                    .await
            })
        })
    }

    fn refresh_account(&self, cx: &mut App) {
        let this = self.clone();
        cx.spawn(async move |cx| {
            let _ = this.refresh_session().await;
            cx.update(|cx| this.publish_account(cx));
        })
        .detach();
    }
}

impl CloudServices {
    /// Renews the access token so membership-derived entitlements reach the claims.
    fn refresh_session(&self) -> BoxFuture<'static, Result<()>> {
        self.worker(move |this, _, lease| {
            Box::pin(async move {
                let account = super::account(&lease)?.to_owned();
                this.core.auth.lease(&account, true).await.map(|_| ())
            })
        })
    }
}
