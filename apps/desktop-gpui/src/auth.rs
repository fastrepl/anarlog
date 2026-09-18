use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use anlg_deeplink_core::AuthCallbackSearch;
use anlg_desktop_auth::{AccountInfo, Persistence, SessionManager, paths, storage_key};
#[cfg(target_os = "linux")]
use anlg_desktop_auth::{LinuxSecurePersistence, SecretStore};
use tokio::sync::watch;

#[cfg(target_os = "linux")]
const AUTH_SCOPE: &str = "auth";
#[cfg(target_os = "linux")]
const AUTH_KEY: &str = "supabase-storage";
const SUPABASE_URL: Option<&str> = option_env!("VITE_SUPABASE_URL");
const SUPABASE_ANON_KEY: Option<&str> = option_env!("VITE_SUPABASE_ANON_KEY");

pub struct Auth {
    session: Arc<SessionManager>,
    callbacks: Mutex<CallbackDeduper>,
    signed_in: watch::Sender<bool>,
    refresh_lock: tokio::sync::Mutex<()>,
}

impl Auth {
    pub fn new(identifier: &str) -> Self {
        let key = SUPABASE_URL.map_or_else(|| "sb-auth-auth-token".to_string(), storage_key);
        let client = SUPABASE_URL
            .zip(SUPABASE_ANON_KEY)
            .map(|(url, anon_key)| anlg_supabase_auth::refresh::AuthClient::new(url, anon_key));
        let persistence = persistence(identifier);
        let session =
            SessionManager::new(key.clone(), persistence, client.clone()).unwrap_or_else(|error| {
                tracing::warn!(%error, "failed to load desktop auth persistence");
                SessionManager::in_memory(key, HashMap::new(), client)
            });
        Self::from_manager(session)
    }

    pub(crate) fn from_manager(session: SessionManager) -> Self {
        let signed_in = session.session().ok().flatten().is_some();
        let (signed_in_tx, _) = watch::channel(signed_in);
        Self {
            session: Arc::new(session),
            callbacks: Mutex::new(CallbackDeduper::default()),
            signed_in: signed_in_tx,
            refresh_lock: tokio::sync::Mutex::new(()),
        }
    }

    pub fn start(identifier: &str, runtime: &tokio::runtime::Handle) -> Arc<Self> {
        let auth = Arc::new(Self::new(identifier));
        let refresh_auth = auth.clone();
        runtime.spawn(async move {
            loop {
                refresh_auth.refresh().await;
                tokio::time::sleep(Duration::from_secs(300)).await;
            }
        });
        auth
    }

    pub fn signed_in(&self) -> bool {
        *self.signed_in.borrow()
    }

    pub fn subscribe(&self) -> watch::Receiver<bool> {
        self.signed_in.subscribe()
    }

    pub fn account_info(&self) -> Option<AccountInfo> {
        self.session.account_info().ok().flatten()
    }

    pub fn session(&self) -> Result<Option<anlg_supabase_auth::session::Session>, String> {
        self.session.session().map_err(|error| error.to_string())
    }

    pub fn sign_out(&self) -> Result<(), String> {
        let result = self.session.sign_out().map_err(|error| error.to_string());
        self.signed_in.send_replace(false);
        result
    }

    pub fn claims(&self) -> Option<anlg_supabase_auth::Claims> {
        session_claims(&self.session().ok().flatten()?)
    }

    pub fn cloud_auth(self: &Arc<Self>) -> Option<CloudAuth> {
        let claims = self.claims().filter(anlg_supabase_auth::Claims::is_paid)?;
        Some(CloudAuth {
            auth: self.clone(),
            user_id: claims.sub,
        })
    }

    pub async fn refresh(&self) {
        if let Err(error) = self.refresh_session(false).await {
            tracing::warn!(%error, "failed to refresh desktop auth session");
        }
    }

    async fn refresh_session(
        &self,
        force: bool,
    ) -> Result<Option<anlg_supabase_auth::session::Session>, String> {
        let _guard = self.refresh_lock.lock().await;
        let before = self.session()?;
        let result = if force {
            let Some(session) = before.as_ref() else {
                return Ok(None);
            };
            let refresh_token = session
                .refresh_token()
                .ok_or_else(|| "Sign in again to use Anarlog Cloud.".to_string())?;
            self.session
                .install_tokens(&session.access_token, refresh_token)
                .await
                .map(Some)
        } else {
            self.session.ensure_fresh(Duration::from_secs(60)).await
        };
        if let Err(error) = result {
            if matches!(&error, anlg_desktop_auth::Error::Refresh(error) if error.is_fatal())
                && self.session()? == before
                && let Err(sign_out_error) = self.sign_out()
            {
                tracing::error!(error = %sign_out_error, "failed to sign out after fatal auth refresh error");
            }
            return Err(error.to_string());
        }
        let current = self.session()?;
        if before != current {
            self.signed_in.send_replace(current.is_some());
        }
        Ok(current)
    }

    pub async fn handle_callback(
        &self,
        callback: AuthCallbackSearch,
    ) -> Result<CallbackOutcome, String> {
        if !has_auth_tokens(&callback) {
            return Ok(CallbackOutcome::Ignored);
        }
        let fingerprint = format!("{}:{}", callback.access_token, callback.refresh_token);
        if !self
            .callbacks
            .lock()
            .unwrap()
            .begin(&fingerprint, SystemTime::now())
        {
            return Ok(CallbackOutcome::Duplicate);
        }
        let result = self
            .session
            .install_tokens(&callback.access_token, &callback.refresh_token)
            .await
            .map(|_| {
                self.signed_in.send_replace(true);
                CallbackOutcome::Installed
            })
            .map_err(|error| error.to_string());
        self.callbacks
            .lock()
            .unwrap()
            .finish(&fingerprint, SystemTime::now(), result.is_ok());
        result
    }
}

#[derive(Clone)]
pub struct CloudAuth {
    auth: Arc<Auth>,
    user_id: String,
}

impl std::fmt::Debug for CloudAuth {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("CloudAuth").finish_non_exhaustive()
    }
}

impl PartialEq for CloudAuth {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.auth, &other.auth) && self.user_id == other.user_id
    }
}

impl Eq for CloudAuth {}

impl CloudAuth {
    pub async fn access_token(&self, force_refresh: bool) -> Result<String, String> {
        let belongs_to_account = |session: &anlg_supabase_auth::session::Session| {
            session
                .user
                .as_ref()
                .is_some_and(|user| user.id == self.user_id)
        };
        if !self
            .auth
            .session()?
            .as_ref()
            .is_some_and(belongs_to_account)
        {
            return Err("Sign in again to use Anarlog Cloud.".to_string());
        }
        let session = self
            .auth
            .refresh_session(force_refresh)
            .await?
            .filter(belongs_to_account)
            .ok_or_else(|| "Sign in again to use Anarlog Cloud.".to_string())?;
        if !session_claims(&session).is_some_and(|claims| claims.is_paid()) {
            return Err("Anarlog Cloud requires an active paid plan or trial.".to_string());
        }
        Ok(session.access_token)
    }
}

fn session_claims(
    session: &anlg_supabase_auth::session::Session,
) -> Option<anlg_supabase_auth::Claims> {
    if session.expires_soon(SystemTime::now(), Duration::ZERO) {
        return None;
    }
    let claims = anlg_supabase_auth::Claims::decode_insecure(&session.access_token).ok()?;
    session
        .user
        .as_ref()
        .filter(|user| user.id == claims.sub)
        .map(|_| claims)
}

pub fn cloud_endpoint(path: &str) -> Option<String> {
    url::Url::parse(option_env!("VITE_API_URL")?)
        .ok()?
        .join(path)
        .ok()
        .map(Into::into)
}

pub enum CallbackOutcome {
    Installed,
    Duplicate,
    Ignored,
}

fn has_auth_tokens(callback: &AuthCallbackSearch) -> bool {
    !callback.access_token.is_empty() && !callback.refresh_token.is_empty()
}

#[derive(Default)]
struct CallbackDeduper {
    recent: Option<(String, SystemTime)>,
    in_flight: Option<String>,
}

impl CallbackDeduper {
    fn begin(&mut self, fingerprint: &str, now: SystemTime) -> bool {
        if self.in_flight.as_deref() == Some(fingerprint) {
            return false;
        }
        if self.recent.as_ref().is_some_and(|(value, at)| {
            value == fingerprint
                && now.duration_since(*at).unwrap_or_default() < Duration::from_secs(5)
        }) {
            return false;
        }
        self.in_flight = Some(fingerprint.to_string());
        true
    }

    fn finish(&mut self, fingerprint: &str, now: SystemTime, succeeded: bool) {
        if self.in_flight.as_deref() == Some(fingerprint) {
            self.in_flight = None;
            if succeeded {
                self.recent = Some((fingerprint.to_string(), now));
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
struct FilePersistence {
    path: PathBuf,
}

#[cfg(not(target_os = "linux"))]
impl Persistence for FilePersistence {
    fn load(&self) -> anlg_desktop_auth::Result<HashMap<String, String>> {
        match std::fs::read_to_string(&self.path) {
            Ok(content) => Ok(serde_json::from_str(&content).unwrap_or_default()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
            Err(error) => Err(anlg_desktop_auth::Error::Persistence(error.to_string())),
        }
    }

    fn save(&self, data: &HashMap<String, String>) -> anlg_desktop_auth::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| anlg_desktop_auth::Error::Persistence(error.to_string()))?;
        }
        let content = serde_json::to_string(data)?;
        anlg_storage::fs::atomic_write(&self.path, &content)
            .map_err(|error| anlg_desktop_auth::Error::Persistence(error.to_string()))
    }

    fn clear(&self) -> anlg_desktop_auth::Result<()> {
        if self.path.exists() {
            std::fs::remove_file(&self.path)
                .map_err(|error| anlg_desktop_auth::Error::Persistence(error.to_string()))?;
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
struct GpuiSecretStore {
    app_id: String,
}

#[cfg(target_os = "windows")]
struct WindowsPersistence {
    secure_path: PathBuf,
    fallback: FilePersistence,
}

#[cfg(target_os = "windows")]
impl Persistence for WindowsPersistence {
    fn load(&self) -> anlg_desktop_auth::Result<HashMap<String, String>> {
        if self.secure_path.is_file() {
            return anlg_storage::windows_auth::load(&self.secure_path)
                .map_err(|error| anlg_desktop_auth::Error::Persistence(error.to_string()));
        }
        self.fallback.load()
    }

    fn save(&self, data: &HashMap<String, String>) -> anlg_desktop_auth::Result<()> {
        anlg_storage::windows_auth::persist(&self.secure_path, data)
            .map_err(|error| anlg_desktop_auth::Error::Persistence(error.to_string()))
    }

    fn clear(&self) -> anlg_desktop_auth::Result<()> {
        let secure = anlg_storage::windows_auth::clear(&self.secure_path);
        let fallback = self.fallback.clear();
        match (secure, fallback) {
            (Err(error), _) => Err(anlg_desktop_auth::Error::Persistence(error.to_string())),
            (Ok(()), Err(error)) => Err(error),
            (Ok(()), Ok(())) => Ok(()),
        }
    }
}

#[cfg(target_os = "linux")]
impl SecretStore for GpuiSecretStore {
    fn read(&self) -> std::result::Result<Option<String>, String> {
        crate::secrets::read(&self.app_id, AUTH_SCOPE, AUTH_KEY)
    }

    fn write(&self, value: &str) -> std::result::Result<(), String> {
        crate::secrets::write(&self.app_id, AUTH_SCOPE, AUTH_KEY, value)
    }

    fn delete(&self) -> std::result::Result<(), String> {
        crate::secrets::delete(&self.app_id, AUTH_SCOPE, AUTH_KEY)
    }
}

#[cfg(target_os = "linux")]
fn linux_persistence(identifier: &str, path: PathBuf) -> LinuxSecurePersistence {
    LinuxSecurePersistence::new(
        Box::new(GpuiSecretStore {
            app_id: identifier.to_string(),
        }),
        path,
    )
}

#[cfg(target_os = "linux")]
fn persistence(identifier: &str) -> Box<dyn Persistence> {
    let data_dir = dirs::data_dir().unwrap_or_else(|| Path::new(".").to_path_buf());
    let local_dir = dirs::data_local_dir().unwrap_or_else(|| data_dir.clone());
    let new_path = local_dir.join(identifier).join(paths::FILENAME);
    let legacy_base = anlg_storage::global::compute_default_base(identifier)
        .unwrap_or_else(|| data_dir.join(identifier));
    let legacy_path = legacy_base.join(paths::FILENAME);
    let store_path = legacy_base.join("store.json");
    let path = paths::resolve_auth_path_from_paths(&legacy_path, &store_path, &new_path);
    Box::new(linux_persistence(identifier, path))
}

#[cfg(not(target_os = "linux"))]
fn persistence(identifier: &str) -> Box<dyn Persistence> {
    let data_dir = dirs::data_dir().unwrap_or_else(|| Path::new(".").to_path_buf());
    let local_dir = dirs::data_local_dir().unwrap_or_else(|| data_dir.clone());
    let new_path = local_dir.join(identifier).join(paths::FILENAME);
    let legacy_base = anlg_storage::global::compute_default_base(identifier)
        .unwrap_or_else(|| data_dir.join(identifier));
    let legacy_path = legacy_base.join(paths::FILENAME);
    let store_path = legacy_base.join("store.json");
    let path = paths::resolve_auth_path_from_paths(&legacy_path, &store_path, &new_path);
    #[cfg(target_os = "windows")]
    {
        Box::new(WindowsPersistence {
            secure_path: anlg_storage::windows_auth::secure_path(&path),
            fallback: FilePersistence { path },
        })
    }
    #[cfg(not(target_os = "windows"))]
    {
        Box::new(FilePersistence { path })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use anlg_supabase_auth::session::Session;
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use serde_json::{Value, json};
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;

    pub fn test_session(mut claims: Value) -> Session {
        let now = chrono::Utc::now().timestamp();
        let object = claims.as_object_mut().unwrap();
        object.entry("sub").or_insert(json!("account"));
        object.entry("exp").or_insert(json!(now + 3600));
        let token = format!("e30.{}.test", URL_SAFE_NO_PAD.encode(claims.to_string()));
        serde_json::from_value(json!({
            "access_token": token,
            "refresh_token": "test-refresh",
            "token_type": "bearer",
            "expires_at": claims["exp"],
            "expires_in": 3600,
            "user": { "id": claims["sub"] },
        }))
        .unwrap()
    }

    pub fn test_auth(session: Option<&Session>, server: Option<&str>) -> Arc<Auth> {
        let data = session
            .map(|session| {
                HashMap::from([(
                    "session".to_string(),
                    serde_json::to_string(session).unwrap(),
                )])
            })
            .unwrap_or_default();
        Arc::new(Auth::from_manager(SessionManager::in_memory(
            "session",
            data,
            server.map(|url| anlg_supabase_auth::refresh::AuthClient::new(url, "test-anon")),
        )))
    }

    pub async fn mock_server(
        responses: Vec<(u16, String)>,
    ) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            tokio::time::timeout(Duration::from_secs(10), async move {
                let mut requests = Vec::new();
                for (status, body) in responses {
                    let (stream, _) = listener.accept().await.unwrap();
                    let mut reader = BufReader::new(stream);
                    let mut headers = String::new();
                    let mut content_length = 0;
                    loop {
                        let mut line = String::new();
                        assert!(reader.read_line(&mut line).await.unwrap() > 0);
                        if line == "\r\n" { break; }
                        if let Some(length) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                            content_length = length.trim().parse::<usize>().unwrap();
                        }
                        headers.push_str(&line);
                    }
                    let mut request_body = vec![0; content_length];
                    reader.read_exact(&mut request_body).await.unwrap();
                    requests.push(format!("{headers}\r\n{}", String::from_utf8(request_body).unwrap()));
                    let response = format!(
                        "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len(),
                    );
                    reader.get_mut().write_all(response.as_bytes()).await.unwrap();
                }
                requests
            }).await.expect("mock requests timed out")
        });
        (base_url, task)
    }

    #[test]
    fn billing_uses_shared_trial_and_entitlement_rules() {
        let now = chrono::Utc::now().timestamp();
        for (claims, pro, paid) in [
            (json!({"entitlements": ["hyprnote_pro"]}), true, true),
            (json!({"entitlements": ["hyprnote_lite"]}), false, true),
            (
                json!({"subscription_status": "trialing", "trial_end": now + 3600}),
                true,
                true,
            ),
            (
                json!({"subscription_status": "trialing", "trial_end": now - 1, "entitlements": ["hyprnote_pro"]}),
                false,
                false,
            ),
            (
                json!({"subscription_status": "paused", "entitlements": ["hyprnote_pro"]}),
                false,
                false,
            ),
            (json!({"subscription_status": "active"}), false, false),
        ] {
            let auth = test_auth(Some(&test_session(claims)), None);
            let claims = auth.claims().unwrap();
            assert_eq!(claims.is_pro(), pro);
            assert_eq!(claims.is_paid(), paid);
            assert_eq!(auth.cloud_auth().is_some(), paid);
        }
    }

    #[test]
    fn missing_invalid_expired_and_mismatched_sessions_have_no_billing_access() {
        assert!(test_auth(None, None).claims().is_none());
        let mut invalid = test_session(json!({"entitlements": ["hyprnote_pro"]}));
        invalid.access_token = "invalid".to_string();
        let mut mismatched = test_session(json!({"entitlements": ["hyprnote_pro"]}));
        mismatched.user.as_mut().unwrap().id = "another-account".to_string();
        let expired = test_session(json!({"exp": 1, "entitlements": ["hyprnote_pro"]}));
        for session in [invalid, mismatched, expired] {
            let auth = test_auth(Some(&session), None);
            assert!(auth.claims().is_none());
            assert!(auth.cloud_auth().is_none());
        }
    }

    #[tokio::test]
    async fn cached_cloud_auth_uses_refreshed_tokens_and_rejects_sign_out() {
        let original = test_session(json!({"entitlements": ["hyprnote_pro"], "revision": 1}));
        let renewed = test_session(json!({"entitlements": ["hyprnote_lite"], "revision": 2}));
        let (url, requests) =
            mock_server(vec![(200, serde_json::to_string(&renewed).unwrap())]).await;
        let auth = test_auth(Some(&original), Some(&url));
        let cloud = auth.cloud_auth().unwrap();
        let mut changes = auth.subscribe();
        assert_eq!(
            cloud.access_token(false).await.unwrap(),
            original.access_token
        );
        assert!(!changes.has_changed().unwrap());
        assert_eq!(
            cloud.access_token(true).await.unwrap(),
            renewed.access_token
        );
        changes.changed().await.unwrap();
        assert_eq!(
            cloud.access_token(false).await.unwrap(),
            renewed.access_token
        );
        assert_eq!(requests.await.unwrap().len(), 1);
        auth.sign_out().unwrap();
        assert!(auth.claims().is_none());
        assert!(cloud.access_token(false).await.is_err());
    }

    #[tokio::test]
    async fn cached_cloud_auth_rejects_account_switches_and_revoked_access() {
        for claims in [
            json!({"sub": "other", "entitlements": ["hyprnote_pro"]}),
            json!({}),
        ] {
            let original = test_session(json!({"entitlements": ["hyprnote_pro"]}));
            let renewed = test_session(claims);
            let (url, requests) =
                mock_server(vec![(200, serde_json::to_string(&renewed).unwrap())]).await;
            let auth = test_auth(Some(&original), Some(&url));
            let cloud = auth.cloud_auth().unwrap();
            assert!(cloud.access_token(true).await.is_err());
            assert!(cloud.access_token(false).await.is_err());
            assert_eq!(requests.await.unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn concurrent_cloud_requests_share_a_proactive_refresh() {
        let original = test_session(json!({
            "exp": chrono::Utc::now().timestamp() + 30,
            "entitlements": ["hyprnote_pro"],
        }));
        let renewed = test_session(json!({"entitlements": ["hyprnote_pro"]}));
        let (url, requests) =
            mock_server(vec![(200, serde_json::to_string(&renewed).unwrap())]).await;
        let auth = test_auth(Some(&original), Some(&url));
        let cloud = auth.cloud_auth().unwrap();
        let (first, second) = tokio::join!(cloud.access_token(false), cloud.access_token(false));
        assert_eq!(first.unwrap(), renewed.access_token);
        assert_eq!(second.unwrap(), renewed.access_token);
        assert_eq!(requests.await.unwrap().len(), 1);
    }

    #[test]
    fn dedupes_for_five_seconds() {
        let mut deduper = CallbackDeduper::default();
        let now = SystemTime::UNIX_EPOCH;
        assert!(deduper.begin("token", now));
        assert!(!deduper.begin("token", now + Duration::from_secs(1)));
        deduper.finish("token", now, true);
        assert!(!deduper.begin("token", now + Duration::from_secs(4)));
        assert!(deduper.begin("token", now + Duration::from_secs(5)));
    }

    #[test]
    fn failed_callback_can_retry_within_five_seconds() {
        let mut deduper = CallbackDeduper::default();
        let now = SystemTime::UNIX_EPOCH;
        assert!(deduper.begin("token", now));
        deduper.finish("token", now, false);
        assert!(deduper.begin("token", now + Duration::from_secs(1)));
    }

    #[test]
    fn successful_callback_is_recent() {
        let mut deduper = CallbackDeduper::default();
        let now = SystemTime::UNIX_EPOCH;
        assert!(deduper.begin("token", now));
        deduper.finish("token", now, true);
        assert!(!deduper.begin("token", now + Duration::from_secs(1)));
    }

    #[test]
    fn code_only_callback_is_not_an_auth_callback() {
        let callback = AuthCallbackSearch {
            code: Some("subscription-code".into()),
            ..AuthCallbackSearch::default()
        };
        assert!(!has_auth_tokens(&callback));
    }
}
