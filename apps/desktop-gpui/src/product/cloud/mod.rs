//! Native cloud services. Construct and call through the desktop runtime worker.

pub mod auth;
pub mod host;
mod integration;
mod management;
mod panels;
pub mod shared_view;
pub use management::ManagementCommand;
mod review;
mod sharing;
mod sync;
mod teams;
mod transport;
pub mod view;
pub use review::{ConflictDecision, ConflictReview};

use std::sync::Arc;

use desktop_runtime::{Result, RuntimeHandle, ServiceError, Services, SessionId};
use futures::future::BoxFuture;
use reqwest::{Method, Url};
use serde_json::Value;
use tokio::sync::Mutex;
use zeroize::Zeroizing;

use super::services::{Action, Mutation, Outcome, Panel, ProductServices, Request, Scope, Surface};
use auth::{Auth, AuthLease, Identity, SecureAuthProvider, SecureStore};
pub use transport::CloudConfig;
use transport::{Transport, failure};

pub trait CloudHost: Send + Sync {
    fn open_url(&self, url: Url) -> BoxFuture<'static, Result<()>>;
    fn copy_text(&self, text: Zeroizing<String>) -> BoxFuture<'static, Result<()>>;
    fn export_recovery(&self, code: Zeroizing<String>) -> BoxFuture<'static, Result<()>>;
    fn flush_editor(&self, session: SessionId) -> BoxFuture<'static, Result<()>>;
}

struct Core {
    auth: Arc<Auth>,
    host: Arc<dyn CloudHost>,
    sync: sync::Sync,
    mutations: Mutex<()>,
    notice: Mutex<Option<String>>,
}

#[derive(Clone)]
pub struct CloudServices {
    runtime: RuntimeHandle,
    core: Arc<Core>,
}

impl CloudServices {
    pub async fn new(
        runtime: RuntimeHandle,
        config: CloudConfig,
        store: Arc<dyn SecureStore>,
        host: Arc<dyn CloudHost>,
    ) -> Result<Self> {
        let core = runtime
            .service(move |services| async move {
                let auth = Auth::new(Transport::new(config)?, store).await?;
                let sync = sync::Sync::default();
                services.db.set_cloudsync_sync_hook(sync.hook.clone());
                Ok(Arc::new(Core {
                    auth,
                    host,
                    sync,
                    mutations: Mutex::new(()),
                    notice: Mutex::new(None),
                }))
            })?
            .receive()
            .await?;
        let shutdown = core.clone();
        runtime
            .register_shutdown(
                desktop_runtime::ShutdownPhase::ApplicationState,
                Box::new(move || {
                    Box::pin(async move {
                        shutdown.sync.stop().await;
                        Ok(())
                    })
                }),
            )?
            .receive()
            .await?;
        Ok(Self { runtime, core })
    }

    pub fn auth_provider(&self) -> Arc<dyn SecureAuthProvider> {
        Arc::new(self.clone())
    }

    pub fn scope(&self) -> Scope {
        Scope {
            account_id: self.core.auth.identity().account_id,
            ..Scope::default()
        }
    }

    pub fn handle_deep_link(&self, url: Url) -> BoxFuture<'static, Result<Identity>> {
        let this = self.clone();
        Box::pin(async move {
            this.runtime
                .service(move |_| async move {
                    let _guard = this.core.mutations.lock().await;
                    this.core.sync.stop().await;
                    this.core.auth.deep_link(url).await
                })?
                .receive()
                .await
        })
    }

    /// Call every 60 seconds and after browser checkout/focus; the worker renews credentials.
    pub fn refresh(&self) -> BoxFuture<'static, Result<()>> {
        let this = self.clone();
        Box::pin(async move {
            this.runtime
                .service(move |services| async move {
                    let _guard = this.core.mutations.lock().await;
                    let Some(account) = this.core.auth.identity().account_id else {
                        return Ok(());
                    };
                    match this.core.auth.lease(&account, false).await {
                        Ok(lease) => {
                            if this.core.sync.enabled(&this.core.auth, &account).await? {
                                this.core.sync.start(&this.core, &services, &lease).await?;
                            }
                            Ok(())
                        }
                        Err(error) => {
                            this.core.sync.stop().await;
                            Err(error)
                        }
                    }
                })?
                .receive()
                .await
        })
    }

    pub fn shutdown(&self) -> BoxFuture<'static, Result<()>> {
        let core = self.core.clone();
        Box::pin(async move {
            core.sync.stop().await;
            Ok(())
        })
    }
}

impl SecureAuthProvider for CloudServices {
    fn identity(&self) -> Identity {
        self.core.auth.identity()
    }
    fn identities(&self) -> tokio::sync::watch::Receiver<Identity> {
        self.core.auth.identities()
    }
    fn credential(&self, account: Arc<str>) -> BoxFuture<'static, Result<AuthLease>> {
        let this = self.clone();
        Box::pin(async move {
            this.runtime
                .service(move |_| async move { this.core.auth.lease(&account, false).await })?
                .receive()
                .await
        })
    }
}

impl CloudServices {
    pub fn load_page(
        &self,
        surface: Surface,
        request: Request,
        page: usize,
    ) -> BoxFuture<'static, Result<Panel>> {
        let this = self.clone();
        Box::pin(async move {
            this.runtime
                .service(move |services| async move {
                    let operation = this.core.load(surface, &request, &services, page);
                    tokio::select! {
                        _ = request.cancel.cancelled() => Err(ServiceError::Cancelled),
                        result = operation => result,
                    }
                })?
                .receive()
                .await
        })
    }
}

impl ProductServices for CloudServices {
    fn load(&self, surface: Surface, request: Request) -> BoxFuture<'static, Result<Panel>> {
        self.load_page(surface, request, 0)
    }
    fn perform(
        &self,
        surface: Surface,
        request: Request,
        mutation: Mutation,
    ) -> BoxFuture<'static, Result<Outcome>> {
        let this = self.clone();
        Box::pin(async move {
            this.runtime
                .service(move |services| async move {
                    let _guard = this
                        .core
                        .mutations
                        .try_lock()
                        .map_err(|_| ServiceError::Busy)?;
                    if request.cancel.is_cancelled() {
                        return Err(ServiceError::Cancelled);
                    }
                    this.core
                        .perform(surface, &request, mutation, &services)
                        .await
                })?
                .receive()
                .await
        })
    }
}

impl Core {
    async fn lease(&self, scope: &Scope) -> Result<AuthLease> {
        let account = scope
            .account_id
            .as_deref()
            .ok_or_else(|| failure("Sign in to continue"))?;
        self.auth.lease(account, false).await
    }

    async fn rpc(&self, lease: &AuthLease, name: &str, body: Value) -> Result<Value> {
        if name.starts_with("list_") && name != "list_session_share_comments" {
            let mut rows = Vec::new();
            let mut bytes = 0usize;
            loop {
                let value = self.rpc_page(lease, name, &body, Some(rows.len())).await?;
                let batch = value
                    .as_array()
                    .filter(|rows| rows.len() <= 128)
                    .ok_or_else(|| failure("Invalid cloud directory page"))?;
                transport::check_directory_size(rows.len(), batch, &mut bytes)?;
                rows.extend(batch.iter().cloned());
                if batch.len() < 128 {
                    return Ok(Value::Array(rows));
                }
            }
        }
        self.rpc_page(lease, name, &body, None).await
    }

    async fn rpc_page(
        &self,
        lease: &AuthLease,
        name: &str,
        body: &Value,
        offset: Option<usize>,
    ) -> Result<Value> {
        let mut url = self
            .auth
            .transport
            .config
            .supabase
            .join(&format!("/rest/v1/rpc/{name}"))
            .map_err(|_| failure("Invalid RPC route"))?;
        if let Some(offset) = offset {
            url.query_pairs_mut()
                .append_pair("limit", "128")
                .append_pair("offset", &offset.to_string());
        }
        let response = self
            .auth
            .transport
            .send(
                Method::POST,
                url,
                Some(lease.access_token()?),
                Some(body),
                &[("apikey", self.auth.transport.config.anon_key.clone())],
            )
            .await?;
        lease.check()?;
        Transport::checked(response)
    }

    async fn api(
        &self,
        lease: &AuthLease,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value> {
        let url = self
            .auth
            .transport
            .config
            .api
            .join(path)
            .map_err(|_| failure("Invalid API route"))?;
        let response = self
            .auth
            .transport
            .send(
                method,
                url,
                Some(lease.access_token()?),
                body,
                &[
                    (
                        "x-device-fingerprint",
                        self.auth.transport.config.device_fingerprint.clone(),
                    ),
                    (
                        "x-anarlog-device-name",
                        self.auth.transport.config.device_name.clone(),
                    ),
                ],
            )
            .await?;
        lease.check()?;
        Transport::checked(response)
    }

    async fn perform(
        &self,
        surface: Surface,
        request: &Request,
        mutation: Mutation,
        services: &Services,
    ) -> Result<Outcome> {
        *self.notice.lock().await = None;
        if !panels::allowed(surface, &mutation.operation.action) {
            return Err(failure("Action does not belong to this page"));
        }
        if mutation.fields.len() > 16
            || mutation
                .fields
                .iter()
                .any(|(key, value)| key.len() > 128 || value.len() > 4096)
        {
            return Err(failure("Invalid form fields"));
        }
        let action = &mutation.operation.action;
        if request.scope.account_id != self.auth.identity().account_id {
            return Err(ServiceError::Cancelled);
        }
        if *action == Action::SignIn {
            let url = self
                .auth
                .begin_login(optional_field(&mutation, "provider").unwrap_or("google"))
                .await?;
            self.host.open_url(url).await?;
            return Ok(Outcome::Refresh);
        }
        if matches!(action, Action::SignOut | Action::LogoutLibrary) {
            self.sync.stop().await;
            self.auth.sign_out().await?;
            return Ok(Outcome::IdentityChanged(Scope::default()));
        }
        let lease = self.lease(&request.scope).await?;
        match surface {
            Surface::Account => match action {
                Action::RefreshAccount => {
                    self.auth.lease(account(&lease)?, true).await?;
                }
                _ => return Err(failure("Invalid account action")),
            },
            Surface::Billing => match action {
                Action::StartTrial => {
                    let interval = choice(&mutation, "interval", &["monthly", "yearly"])?;
                    let response = self
                        .api(
                            &lease,
                            Method::POST,
                            &format!("/subscription/start-trial?interval={interval}"),
                            None,
                        )
                        .await?;
                    if response.get("started").and_then(Value::as_bool) != Some(true) {
                        return Err(failure(
                            "Trial was not started. Refresh billing to check eligibility",
                        ));
                    }
                    self.auth.lease(account(&lease)?, true).await?;
                }
                Action::Checkout | Action::BillingPortal => {
                    let path = if *action == Action::Checkout {
                        "/app/checkout"
                    } else {
                        "/app/portal"
                    };
                    let mut url = self.auth.transport.config.web_flow(path)?;
                    if *action == Action::Checkout {
                        url.query_pairs_mut()
                            .append_pair(
                                "period",
                                choice(&mutation, "interval", &["monthly", "yearly"])?,
                            )
                            .append_pair("plan", "pro")
                            .append_pair("source", "settings");
                    }
                    self.host.open_url(url).await?;
                }
                Action::RefreshAccount => {
                    self.auth.lease(account(&lease)?, true).await?;
                }
                _ => return Err(failure("Invalid billing action")),
            },
            Surface::Teams => self.team_action(&lease, &request.scope, &mutation).await?,
            Surface::CloudSync => self.sync.action(self, services, &lease, &mutation).await?,
            Surface::Sharing => {
                self.share_action(services, &lease, &request.scope, &mutation)
                    .await?
            }
            _ => return Err(failure("Route this surface to its product service")),
        }
        lease.check()?;
        Ok(Outcome::Refresh)
    }
}

fn account(lease: &AuthLease) -> Result<&str> {
    lease
        .identity
        .account_id
        .as_deref()
        .ok_or_else(|| failure("Sign in to continue"))
}

fn optional_field<'a>(mutation: &'a Mutation, key: &str) -> Option<&'a str> {
    mutation
        .fields
        .iter()
        .find(|(name, _)| name.as_ref() == key)
        .map(|(_, value)| value.trim())
}

fn field<'a>(mutation: &'a Mutation, key: &str) -> Result<&'a str> {
    optional_field(mutation, key)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| failure(&format!("Enter {key}")))
}

fn choice<'a>(mutation: &'a Mutation, key: &str, choices: &[&str]) -> Result<&'a str> {
    let value = field(mutation, key)?;
    if choices.contains(&value) {
        Ok(value)
    } else {
        Err(failure(&format!("Invalid {key}")))
    }
}

fn target(mutation: &Mutation) -> Result<&str> {
    mutation
        .operation
        .target_id
        .as_deref()
        .or_else(|| optional_field(mutation, "target"))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| failure("Select a target"))
}

#[cfg(test)]
mod tests;
