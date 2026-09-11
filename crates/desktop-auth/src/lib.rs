use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anlg_supabase_auth::client::store::AuthStore;
use anlg_supabase_auth::refresh::AuthClient;
use anlg_supabase_auth::session::{Session, find_session};

pub mod paths;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{LinuxSecurePersistence, SecretStore};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub user_id: String,
    pub email: Option<String>,
    pub full_name: Option<String>,
    pub avatar_url: Option<String>,
    pub stripe_customer_id: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    AuthStore(#[from] anlg_supabase_auth::client::Error),
    #[error(transparent)]
    Refresh(#[from] anlg_supabase_auth::refresh::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("invalid auth session: {0}")]
    InvalidSession(String),
    #[error("auth persistence failed: {0}")]
    Persistence(String),
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn account_info(data: &HashMap<String, String>) -> Result<Option<AccountInfo>> {
    let Some(session) = find_session(data)? else {
        return Ok(None);
    };
    let Some(user) = session.user else {
        return Ok(None);
    };
    let metadata = user.user_metadata;
    Ok(Some(AccountInfo {
        user_id: user.id,
        email: user.email,
        full_name: metadata.as_ref().and_then(|m| m.full_name.clone()),
        avatar_url: metadata.as_ref().and_then(|m| m.avatar_url.clone()),
        stripe_customer_id: metadata.as_ref().and_then(|m| m.stripe_customer_id.clone()),
    }))
}

pub fn access_token(data: &HashMap<String, String>) -> Result<Option<String>> {
    Ok(find_session(data)?.map(|session| session.access_token))
}

pub fn storage_key(supabase_url: &str) -> String {
    let authority = supabase_url
        .split_once("://")
        .map_or(supabase_url, |(_, rest)| rest)
        .split('/')
        .next()
        .unwrap_or_default()
        .split('@')
        .next_back()
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or_default();
    let label = authority.split('.').next().unwrap_or_default();
    format!("sb-{label}-auth-token")
}

pub trait Persistence: Send + Sync {
    fn load(&self) -> Result<HashMap<String, String>>;
    fn save(&self, data: &HashMap<String, String>) -> Result<()>;
    fn clear(&self) -> Result<()>;
}

pub struct SessionManager {
    pub(crate) store: AuthStore,
    pub(crate) key: String,
    pub(crate) client: Option<AuthClient>,
    persistence: Arc<dyn Persistence>,
}

impl SessionManager {
    pub fn new(
        key: impl Into<String>,
        persistence: Box<dyn Persistence>,
        client: Option<AuthClient>,
    ) -> Result<Self> {
        let persistence: Arc<dyn Persistence> = persistence.into();
        let data = persistence.load()?;
        Ok(Self {
            store: AuthStore::in_memory(data),
            key: key.into(),
            client,
            persistence,
        })
    }

    pub fn in_memory(
        key: impl Into<String>,
        data: HashMap<String, String>,
        client: Option<AuthClient>,
    ) -> Self {
        Self::new(key, Box::new(InMemoryPersistence::new(data)), client)
            .expect("in-memory persistence cannot fail")
    }

    pub fn session(&self) -> Result<Option<Session>> {
        let Some(value) = self.store.get(&self.key) else {
            return Ok(None);
        };
        Ok(Some(serde_json::from_str(&value)?))
    }

    pub async fn install_tokens(
        &self,
        _access_token: &str,
        refresh_token: &str,
    ) -> Result<Session> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| Error::Persistence("refresh client is not configured".into()))?;
        let session = client.refresh_session(refresh_token).await?;
        if session.access_token.is_empty() {
            return Err(Error::InvalidSession(
                "refresh returned no access token".to_string(),
            ));
        }
        self.save_session(&session)?;
        Ok(session)
    }

    pub async fn ensure_fresh(&self, skew: Duration) -> Result<Option<Session>> {
        let Some(session) = self.session()? else {
            return Ok(None);
        };
        if !session.requires_refresh(SystemTime::now(), skew) {
            return Ok(Some(session));
        }
        let Some(refresh_token) = session.refresh_token() else {
            return Ok(Some(session));
        };
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| Error::Persistence("refresh client is not configured".into()))?;
        let refreshed = client.refresh_session(refresh_token).await?;
        self.save_session(&refreshed)?;
        Ok(Some(refreshed))
    }

    pub fn sign_out(&self) -> Result<()> {
        let _ = self.store.clear();
        self.persistence.clear()
    }

    pub fn account_info(&self) -> Result<Option<AccountInfo>> {
        account_info(&self.store.snapshot())
    }

    pub fn access_token(&self) -> Result<Option<String>> {
        access_token(&self.store.snapshot())
    }

    fn save_session(&self, session: &Session) -> Result<()> {
        let value = serde_json::to_string(session)?;
        let mut data = self.store.snapshot();
        data.insert(self.key.clone(), value);
        self.store.replace(data.clone());
        self.persistence.save(&data)
    }
}

struct InMemoryPersistence {
    data: std::sync::Mutex<HashMap<String, String>>,
}

impl InMemoryPersistence {
    fn new(data: HashMap<String, String>) -> Self {
        Self {
            data: std::sync::Mutex::new(data),
        }
    }
}

impl Persistence for InMemoryPersistence {
    fn load(&self) -> Result<HashMap<String, String>> {
        Ok(self.data.lock().unwrap().clone())
    }

    fn save(&self, data: &HashMap<String, String>) -> Result<()> {
        *self.data.lock().unwrap() = data.clone();
        Ok(())
    }

    fn clear(&self) -> Result<()> {
        self.data.lock().unwrap().clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_key_matches_web_client() {
        assert_eq!(
            storage_key("https://project.supabase.co"),
            "sb-project-auth-token"
        );
    }

    #[test]
    fn failed_persistence_does_not_rollback_memory() {
        struct Failing;
        impl Persistence for Failing {
            fn load(&self) -> Result<HashMap<String, String>> {
                Ok(HashMap::new())
            }
            fn save(&self, _: &HashMap<String, String>) -> Result<()> {
                Err(Error::Persistence("nope".into()))
            }
            fn clear(&self) -> Result<()> {
                Ok(())
            }
        }

        let manager = SessionManager::new("session", Box::new(Failing), None).unwrap();
        let session =
            r#"{"access_token":"access","refresh_token":"refresh","token_type":"bearer"}"#;
        let mut data = HashMap::new();
        data.insert("session".to_string(), session.to_string());
        manager.store.replace(data);
        let result = manager.save_session(&serde_json::from_str(session).unwrap());
        assert!(result.is_err());
        assert_eq!(manager.session().unwrap().unwrap().access_token, "access");
    }
}
