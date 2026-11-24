use std::{
    fmt,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anlg_supabase_auth::{Claims, session::Session};
use desktop_runtime::{CancellationToken, Result, ServiceError};
use futures::future::BoxFuture;
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};
use serde_json::json;
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

/// Where the signed-in session persists.
pub trait SessionStore: Send + Sync {
    fn load(&self) -> BoxFuture<'static, Result<Option<Session>>>;
    fn save(&self, session: &Session) -> BoxFuture<'static, Result<()>>;
    fn clear(&self) -> BoxFuture<'static, Result<()>>;
}

/// A sandbox profile's session: one item of this app's secure store.
pub struct KeychainSession {
    store: Arc<dyn SecureStore>,
    key: String,
}

impl KeychainSession {
    pub fn new(store: Arc<dyn SecureStore>, key: String) -> Self {
        Self { store, key }
    }
}

impl SessionStore for KeychainSession {
    fn load(&self) -> BoxFuture<'static, Result<Option<Session>>> {
        let read = self.store.read(self.key.clone());
        Box::pin(async move {
            read.await?
                .map(|value| {
                    serde_json::from_str(&value)
                        .map_err(|_| failure("Stored session is invalid. Sign out to clear it"))
                })
                .transpose()
        })
    }

    fn save(&self, session: &Session) -> BoxFuture<'static, Result<()>> {
        let value = serde_json::to_string(session);
        let store = self.store.clone();
        let key = self.key.clone();
        Box::pin(async move {
            let value = value.map_err(|_| failure("Could not persist session"))?;
            store.write(key, Zeroizing::new(value)).await
        })
    }

    fn clear(&self) -> BoxFuture<'static, Result<()>> {
        self.store.remove(self.key.clone())
    }
}

/// A channel run's session, kept where and how the shipping app keeps it: its supabase-js
/// storage map, which the Tauri app reads after a rollback and the bundled CLI shares.
pub struct ShippingSession {
    store: Arc<anlg_desktop_identity::session::SessionStore>,
    /// The key shipping's supabase-js client reads.
    key: String,
}

impl ShippingSession {
    pub fn new(store: anlg_desktop_identity::session::SessionStore, supabase: &Url) -> Self {
        Self {
            store: Arc::new(store),
            key: anlg_desktop_identity::session::storage_key(
                supabase.host_str().unwrap_or_default(),
            ),
        }
    }

    fn blocking<T: Send + 'static>(
        &self,
        work: impl FnOnce(&anlg_desktop_identity::session::SessionStore, &str) -> Result<T>
        + Send
        + 'static,
    ) -> BoxFuture<'static, Result<T>> {
        let store = self.store.clone();
        let key = self.key.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || work(&store, &key))
                .await
                .map_err(|_| storage_error())?
        })
    }
}

impl SessionStore for ShippingSession {
    fn load(&self) -> BoxFuture<'static, Result<Option<Session>>> {
        self.blocking(|store, key| {
            let map = store.load();
            // The desktop key first; the CLI may have saved its login under its own.
            let value = map.get(key).or_else(|| {
                map.iter()
                    .find(|(name, _)| {
                        name.ends_with(anlg_desktop_identity::session::SESSION_KEY_SUFFIX)
                    })
                    .map(|(_, value)| value)
            });
            // Like supabase-js, an unreadable session reads as signed out and stays stored.
            Ok(value.and_then(|value| match serde_json::from_str(value) {
                Ok(session) => Some(session),
                Err(error) => {
                    tracing::warn!(%error, "ignoring an unreadable stored session");
                    None
                }
            }))
        })
    }

    fn save(&self, session: &Session) -> BoxFuture<'static, Result<()>> {
        let value = serde_json::to_string(session);
        self.blocking(move |store, key| {
            let value = value.map_err(|_| failure("Could not persist session"))?;
            // Rewrite the stored map, other entries included, with this one session in it.
            let mut map = store.load();
            map.retain(|name, _| {
                !name.ends_with(anlg_desktop_identity::session::SESSION_KEY_SUFFIX)
            });
            map.insert(key.to_owned(), value);
            store.save(&map).map_err(|error| failure(&error))
        })
    }

    fn clear(&self) -> BoxFuture<'static, Result<()>> {
        self.blocking(|store, _| store.clear().map_err(|error| failure(&error)))
    }
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

    pub fn expires_at(&self) -> u64 {
        self.expires_at
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
    state: String,
    started: u64,
}

pub(crate) struct Auth {
    pub store: Arc<dyn SecureStore>,
    pub transport: Transport,
    session: Arc<dyn SessionStore>,
    state: Mutex<State>,
    identity: watch::Sender<Identity>,
}

impl Auth {
    /// Keeps a sandbox profile's session in `store`. A channel run must pass the shipping
    /// app's session store to `with_session`.
    pub async fn new(transport: Transport, store: Arc<dyn SecureStore>) -> Result<Arc<Self>> {
        let key = transport.config.secrets.session().ok_or_else(|| {
            failure("A channel run keeps its sign-in in the shipping app's session store")
        })?;
        let session = Arc::new(KeychainSession::new(store.clone(), key));
        Self::with_session(transport, store, session).await
    }

    pub async fn with_session(
        transport: Transport,
        store: Arc<dyn SecureStore>,
        session: Arc<dyn SessionStore>,
    ) -> Result<Arc<Self>> {
        let (identity, _) = watch::channel(Identity::default());
        let this = Arc::new(Self {
            store,
            transport,
            session,
            identity,
            state: Mutex::new(State {
                session: None,
                cancel: CancellationToken::new(),
            }),
        });
        if let Some(session) = this.session.load().await? {
            this.validate_session(&session)?;
            let mut state = this.state.lock().await;
            this.announce(&mut state, Some(&session));
            state.session = Some(session);
        }
        Ok(this)
    }

    pub fn secrets(&self) -> &super::SecretNames {
        &self.transport.config.secrets
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
        let needs_refresh = |session: &Session| {
            force || session.requires_refresh(SystemTime::now(), Duration::from_secs(90))
        };
        let session = state
            .session
            .as_ref()
            .ok_or_else(|| failure("Sign in to continue"))?;
        if needs_refresh(session) {
            // Another app sharing the session, such as the CLI, may have rotated it since it
            // was read; refreshing with the old token would fail and sign this app out.
            if let Some(stored) = self.session.load().await?
                && stored.user.as_ref().map(|user| user.id.as_str()) == Some(account)
            {
                state.session = Some(stored);
            }
        }
        let session = state
            .session
            .as_ref()
            .ok_or_else(|| failure("Sign in to continue"))?;
        if needs_refresh(session) {
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
        self.session.save(session).await
    }

    /// Shipping's browser hand-off: the web `/auth` page signs in and returns to the app with
    /// tokens and this request's `state`.
    pub async fn begin_sign_in(&self) -> Result<Url> {
        let _state = self.state.lock().await;
        let pending = PendingLogin {
            state: uuid::Uuid::new_v4().to_string(),
            started: now(),
        };
        self.store
            .write(
                self.secrets().pending_sign_in(),
                Zeroizing::new(
                    serde_json::to_string(&pending)
                        .map_err(|_| failure("Could not start sign in"))?,
                ),
            )
            .await?;
        let mut url = self.transport.config.web_flow("/auth")?;
        url.query_pairs_mut()
            .append_pair("desktop_state", &pending.state);
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
            .read(self.secrets().pending_sign_in())
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
            if values.len() != 1 || values[0].1.is_empty() {
                return Err(failure("Invalid sign-in callback"));
            }
            Ok(values[0].1.to_string())
        };
        if parameter("state")? != pending.state || now().saturating_sub(pending.started) > 900 {
            return Err(failure(
                "Sign-in request expired or belongs to another device",
            ));
        }
        parameter("access_token")?;
        // Exchanging the handed-off refresh token proves it and returns the account.
        let response = self
            .transport
            .send(
                Method::POST,
                self.transport
                    .config
                    .supabase
                    .join("/auth/v1/token?grant_type=refresh_token")
                    .map_err(|_| failure("Invalid auth endpoint"))?,
                None,
                Some(&json!({"refresh_token": parameter("refresh_token")?})),
                &[("apikey", self.transport.config.anon_key.clone())],
            )
            .await?;
        let mut session: Session = serde_json::from_value(Transport::checked(response)?)
            .map_err(|_| failure("Invalid session response"))?;
        normalize_expiry(&mut session);
        self.validate_session(&session)?;
        self.persist(&session).await?;
        self.store.remove(self.secrets().pending_sign_in()).await?;
        self.announce(&mut state, Some(&session));
        state.session = Some(session);
        Ok(self.identity())
    }

    pub async fn sign_out(&self) -> Result<()> {
        let mut state = self.state.lock().await;
        let old = state.session.take();
        self.announce(&mut state, None);
        let removed = self.session.clear().await;
        let pending_removed = self.store.remove(self.secrets().pending_sign_in()).await;
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
