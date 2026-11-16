use std::{
    fmt,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anlg_supabase_auth::{Claims, session::Session};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use desktop_runtime::{CancellationToken, Result, ServiceError};
use futures::future::BoxFuture;
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, watch};
use zeroize::Zeroizing;

use super::transport::{Transport, failure};

pub trait SecureStore: Send + Sync {
    fn read(&self, key: String) -> BoxFuture<'static, Result<Option<Zeroizing<String>>>>;
    fn write(&self, key: String, value: Zeroizing<String>) -> BoxFuture<'static, Result<()>>;
    fn remove(&self, key: String) -> BoxFuture<'static, Result<()>>;
}

pub struct KeyringStore {
    service: Arc<str>,
}

impl KeyringStore {
    pub fn new(service: impl Into<Arc<str>>) -> Self {
        Self {
            service: service.into(),
        }
    }
}

impl SecureStore for KeyringStore {
    fn read(&self, key: String) -> BoxFuture<'static, Result<Option<Zeroizing<String>>>> {
        let service = self.service.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let entry = keyring::Entry::new(&service, &key).map_err(|_| storage_error())?;
                match entry.get_password() {
                    Ok(value) => Ok(Some(Zeroizing::new(value))),
                    Err(keyring::Error::NoEntry) => Ok(None),
                    Err(_) => Err(storage_error()),
                }
            })
            .await
            .map_err(|_| storage_error())?
        })
    }

    fn write(&self, key: String, value: Zeroizing<String>) -> BoxFuture<'static, Result<()>> {
        let service = self.service.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                keyring::Entry::new(&service, &key)
                    .map_err(|_| storage_error())?
                    .set_password(&value)
                    .map_err(|_| storage_error())
            })
            .await
            .map_err(|_| storage_error())?
        })
    }

    fn remove(&self, key: String) -> BoxFuture<'static, Result<()>> {
        let service = self.service.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let entry = keyring::Entry::new(&service, &key).map_err(|_| storage_error())?;
                match entry.delete_credential() {
                    Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                    Err(_) => Err(storage_error()),
                }
            })
            .await
            .map_err(|_| storage_error())?
        })
    }
}

fn storage_error() -> ServiceError {
    failure("Secure credential storage is locked or unavailable. Unlock it and retry")
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Identity {
    pub account_id: Option<Arc<str>>,
    pub email: Option<Arc<str>>,
    pub generation: u64,
}

pub struct AuthLease {
    pub identity: Identity,
    pub cancelled: CancellationToken,
    token: Zeroizing<String>,
    expires_at: u64,
}

impl fmt::Debug for AuthLease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthLease")
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl AuthLease {
    pub fn authorize(&self, request: reqwest::RequestBuilder) -> Result<reqwest::RequestBuilder> {
        self.check()?;
        Ok(request.bearer_auth(self.token.as_str()))
    }

    pub fn access_token(&self) -> Result<&str> {
        self.check()?;
        Ok(self.token.as_str())
    }

    pub fn check(&self) -> Result<()> {
        if self.cancelled.is_cancelled() || now() >= self.expires_at {
            Err(ServiceError::Cancelled)
        } else {
            Ok(())
        }
    }
}

pub trait SecureAuthProvider: Send + Sync {
    fn identity(&self) -> Identity;
    fn identities(&self) -> watch::Receiver<Identity>;
    fn credential(&self, account: Arc<str>) -> BoxFuture<'static, Result<AuthLease>>;
}

struct State {
    session: Option<Session>,
    cancel: CancellationToken,
}

#[derive(Serialize, Deserialize)]
struct PendingLogin {
    verifier: String,
    state: String,
    started: u64,
}

pub(crate) struct Auth {
    pub store: Arc<dyn SecureStore>,
    pub transport: Transport,
    state: Mutex<State>,
    identity: watch::Sender<Identity>,
}

impl Auth {
    pub async fn new(transport: Transport, store: Arc<dyn SecureStore>) -> Result<Arc<Self>> {
        let (identity, _) = watch::channel(Identity::default());
        let this = Arc::new(Self {
            store,
            transport,
            identity,
            state: Mutex::new(State {
                session: None,
                cancel: CancellationToken::new(),
            }),
        });
        if let Some(value) = this.store.read(this.key("session")).await? {
            let session: Session = serde_json::from_str(&value)
                .map_err(|_| failure("Stored session is invalid. Sign out to clear it"))?;
            this.validate_session(&session)?;
            let mut state = this.state.lock().await;
            this.announce(&mut state, Some(&session));
            state.session = Some(session);
        }
        Ok(this)
    }

    pub fn key(&self, suffix: &str) -> String {
        format!("{}:{suffix}", self.transport.config.credential_namespace)
    }

    pub fn identity(&self) -> Identity {
        self.identity.borrow().clone()
    }

    pub fn identities(&self) -> watch::Receiver<Identity> {
        self.identity.subscribe()
    }

    fn announce(&self, state: &mut State, session: Option<&Session>) {
        state.cancel.cancel();
        state.cancel = CancellationToken::new();
        let generation = self.identity.borrow().generation + 1;
        let user = session.and_then(|session| session.user.as_ref());
        self.identity.send_replace(Identity {
            account_id: user.map(|user| user.id.as_str().into()),
            email: user.and_then(|user| user.email.as_deref()).map(Into::into),
            generation,
        });
    }

    fn validate_session(&self, session: &Session) -> Result<()> {
        let user = session
            .user
            .as_ref()
            .ok_or_else(|| failure("Session has no user"))?;
        super::transport::uuid(&user.id)?;
        if session.access_token.is_empty() || user.is_anonymous == Some(true) {
            return Err(failure("A registered account is required"));
        }
        Ok(())
    }

    pub async fn lease(&self, account: &str, force: bool) -> Result<AuthLease> {
        let mut state = self.state.lock().await;
        if self.identity.borrow().account_id.as_deref() != Some(account) {
            return Err(ServiceError::Conflict);
        }
        let session = state
            .session
            .as_ref()
            .ok_or_else(|| failure("Sign in to continue"))?;
        if force || session.requires_refresh(SystemTime::now(), Duration::from_secs(90)) {
            let refresh_token = session
                .refresh_token()
                .ok_or_else(|| failure("Sign in again to renew this session"))?
                .to_owned();
            let url = self
                .transport
                .config
                .supabase
                .join("/auth/v1/token?grant_type=refresh_token")
                .map_err(|_| failure("Invalid auth route"))?;
            let response = self
                .transport
                .send(
                    Method::POST,
                    url,
                    None,
                    Some(&json!({"refresh_token": refresh_token})),
                    &[("apikey", self.transport.config.anon_key.clone())],
                )
                .await?;
            let mut renewed: Session = serde_json::from_value(Transport::checked(response)?)
                .map_err(|_| failure("Invalid refreshed session"))?;
            self.validate_session(&renewed)?;
            if renewed.user.as_ref().map(|user| user.id.as_str()) != Some(account) {
                return Err(ServiceError::Conflict);
            }
            normalize_expiry(&mut renewed);
            self.persist(&renewed).await?;
            state.session = Some(renewed);
        }
        let session = state
            .session
            .as_ref()
            .ok_or_else(|| failure("Sign in to continue"))?;
        if session.expires_soon(SystemTime::now(), Duration::ZERO) {
            return Err(failure("Session expired"));
        }
        Ok(AuthLease {
            identity: self.identity(),
            cancelled: state.cancel.clone(),
            token: Zeroizing::new(session.access_token.clone()),
            expires_at: session.expires_at.unwrap_or(0),
        })
    }

    async fn persist(&self, session: &Session) -> Result<()> {
        let value =
            serde_json::to_string(session).map_err(|_| failure("Could not persist session"))?;
        self.store
            .write(self.key("session"), Zeroizing::new(value))
            .await
    }

    pub async fn begin_login(&self, provider: &str) -> Result<Url> {
        if !matches!(provider, "google" | "github" | "apple" | "azure") {
            return Err(failure("Unknown OAuth provider"));
        }
        let _state = self.state.lock().await;
        let verifier = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let nonce = uuid::Uuid::new_v4().to_string();
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let pending = PendingLogin {
            verifier,
            state: nonce.clone(),
            started: now(),
        };
        self.store
            .write(
                self.key("pkce"),
                Zeroizing::new(
                    serde_json::to_string(&pending)
                        .map_err(|_| failure("Could not start sign in"))?,
                ),
            )
            .await?;
        let mut callback = self.transport.config.callback.clone();
        callback.query_pairs_mut().append_pair("state", &nonce);
        let mut url = self
            .transport
            .config
            .supabase
            .join("/auth/v1/authorize")
            .map_err(|_| failure("Invalid auth endpoint"))?;
        url.query_pairs_mut()
            .append_pair("provider", provider)
            .append_pair("redirect_to", callback.as_str())
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "s256");
        Ok(url)
    }

    pub async fn deep_link(&self, url: Url) -> Result<Identity> {
        let callback = &self.transport.config.callback;
        if url.scheme() != callback.scheme()
            || url.host_str() != callback.host_str()
            || url.path() != callback.path()
            || url.fragment().is_some()
            || !url.username().is_empty()
        {
            return Err(failure("Unexpected sign-in callback"));
        }
        let mut state = self.state.lock().await;
        let pending = self
            .store
            .read(self.key("pkce"))
            .await?
            .ok_or_else(|| failure("No sign-in request is pending"))?;
        let pending: PendingLogin =
            serde_json::from_str(&pending).map_err(|_| failure("Invalid pending login"))?;
        let pairs = url.query_pairs().collect::<Vec<_>>();
        let parameter = |name| -> Result<String> {
            let values = pairs
                .iter()
                .filter(|(key, _)| key == name)
                .collect::<Vec<_>>();
            if values.len() != 1 {
                return Err(failure("Invalid sign-in callback"));
            }
            Ok(values[0].1.to_string())
        };
        if parameter("state")? != pending.state || now().saturating_sub(pending.started) > 600 {
            return Err(failure(
                "Sign-in request expired or belongs to another device",
            ));
        }
        let code = parameter("code")?;
        let response = self
            .transport
            .send(
                Method::POST,
                self.transport
                    .config
                    .supabase
                    .join("/auth/v1/token?grant_type=pkce")
                    .map_err(|_| failure("Invalid auth endpoint"))?,
                None,
                Some(&json!({"auth_code": code, "code_verifier": pending.verifier})),
                &[("apikey", self.transport.config.anon_key.clone())],
            )
            .await?;
        let mut session: Session = serde_json::from_value(Transport::checked(response)?)
            .map_err(|_| failure("Invalid session response"))?;
        normalize_expiry(&mut session);
        self.validate_session(&session)?;
        self.persist(&session).await?;
        self.store.remove(self.key("pkce")).await?;
        self.announce(&mut state, Some(&session));
        state.session = Some(session);
        Ok(self.identity())
    }

    pub async fn sign_out(&self) -> Result<()> {
        let mut state = self.state.lock().await;
        let old = state.session.take();
        self.announce(&mut state, None);
        let removed = self.store.remove(self.key("session")).await;
        let pending_removed = self.store.remove(self.key("pkce")).await;
        if let Some(session) = old {
            // Local erasure must not depend on reachability of the logout endpoint.
            let _ = self
                .transport
                .send(
                    Method::POST,
                    self.transport
                        .config
                        .supabase
                        .join("/auth/v1/logout?scope=local")
                        .map_err(|_| failure("Invalid auth endpoint"))?,
                    Some(&session.access_token),
                    None,
                    &[("apikey", self.transport.config.anon_key.clone())],
                )
                .await;
        }
        removed?;
        pending_removed
    }

    pub async fn claims(&self, account: &str) -> Result<Claims> {
        let lease = self.lease(account, false).await?;
        Claims::decode_insecure(lease.access_token()?)
            .map_err(|_| failure("Invalid account claims"))
    }
}

fn normalize_expiry(session: &mut Session) {
    if session.expires_at.is_none() {
        session.expires_at = session
            .expires_in
            .map(|duration| now().saturating_add(duration));
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
