use std::collections::HashMap;
use std::sync::Arc;

use rand_core::OsRng;
use ssh_key::{Algorithm, LineEnding, PrivateKey};
use tokio::sync::Mutex;

use crate::Error;
use crate::naming::UserId;

/// Per-user SSH keypair as stored by the caller.
///
/// The `private_pem` is in OpenSSH-ASCII format (`-----BEGIN OPENSSH PRIVATE KEY-----`).
/// `public_openssh` is the single-line authorized-keys format that exe.dev's
/// `ssh-key add` expects (e.g. `ssh-ed25519 AAAA... label`).
#[derive(Debug, Clone)]
pub struct UserSshKey {
    pub private_pem: String,
    pub public_openssh: String,
    pub fingerprint: String,
}

/// Abstraction over per-user SSH key storage.
///
/// Implementations MUST:
/// - return the *same* keypair for the same `UserId` on repeat calls, or
///   callers will lose the ability to mint tokens the server recognizes;
/// - be safe to call concurrently for the same user (the manager calls this
///   from provision/resume paths that may race).
pub trait UserKeyring: Send + Sync + 'static {
    fn get_or_create(
        &self,
        user: &UserId,
    ) -> impl std::future::Future<Output = Result<UserSshKey, Error>> + Send;

    fn remove(&self, user: &UserId) -> impl std::future::Future<Output = Result<(), Error>> + Send;
}

/// Generate a fresh Ed25519 keypair. Exposed so storage backends can implement
/// `get_or_create` by calling into this helper when a miss occurs.
pub fn generate_ssh_key(label: &str) -> Result<UserSshKey, Error> {
    let key = PrivateKey::random(&mut OsRng, Algorithm::Ed25519)
        .map_err(|e| Error::KeyGen(e.to_string()))?;
    let private_pem = key
        .to_openssh(LineEnding::LF)
        .map_err(|e| Error::KeyGen(e.to_string()))?
        .to_string();
    let mut public_openssh = key
        .public_key()
        .to_openssh()
        .map_err(|e| Error::KeyGen(e.to_string()))?;
    if !label.is_empty() {
        public_openssh.push(' ');
        public_openssh.push_str(label);
    }
    let fingerprint = key
        .public_key()
        .fingerprint(ssh_key::HashAlg::Sha256)
        .to_string();
    Ok(UserSshKey {
        private_pem,
        public_openssh,
        fingerprint,
    })
}

/// In-memory keyring for tests and local dev. Not durable.
#[derive(Default)]
pub struct InMemoryKeyring {
    inner: Arc<Mutex<HashMap<UserId, UserSshKey>>>,
}

impl InMemoryKeyring {
    pub fn new() -> Self {
        Self::default()
    }
}

impl UserKeyring for InMemoryKeyring {
    async fn get_or_create(&self, user: &UserId) -> Result<UserSshKey, Error> {
        let mut guard = self.inner.lock().await;
        if let Some(existing) = guard.get(user) {
            return Ok(existing.clone());
        }
        let label = format!("api-claw:{user}");
        let key = generate_ssh_key(&label)?;
        guard.insert(user.clone(), key.clone());
        Ok(key)
    }

    async fn remove(&self, user: &UserId) -> Result<(), Error> {
        self.inner.lock().await.remove(user);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn keyring_is_deterministic_per_user() {
        let kr = InMemoryKeyring::new();
        let u = UserId::new("u1");
        let a = kr.get_or_create(&u).await.unwrap();
        let b = kr.get_or_create(&u).await.unwrap();
        assert_eq!(a.public_openssh, b.public_openssh);
        assert_eq!(a.fingerprint, b.fingerprint);
    }

    #[tokio::test]
    async fn different_users_get_different_keys() {
        let kr = InMemoryKeyring::new();
        let a = kr.get_or_create(&UserId::new("a")).await.unwrap();
        let b = kr.get_or_create(&UserId::new("b")).await.unwrap();
        assert_ne!(a.public_openssh, b.public_openssh);
    }

    #[test]
    fn generate_emits_openssh_pem_and_pubkey() {
        let k = generate_ssh_key("label").unwrap();
        assert!(
            k.private_pem.contains("BEGIN OPENSSH PRIVATE KEY"),
            "bad pem: {}",
            k.private_pem
        );
        assert!(k.public_openssh.starts_with("ssh-ed25519 "));
        assert!(k.public_openssh.ends_with(" label"));
        assert!(k.fingerprint.starts_with("SHA256:"));
    }
}
