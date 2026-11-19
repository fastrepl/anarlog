//! Where cloud secrets live. A sandbox profile keeps its own keychain namespace. A channel run
//! uses the shipping app's secure store and names, so an update finds the recovery key the
//! shipping app saved and never asks for it again.

use std::sync::Arc;

use anlg_desktop_identity::{SecretSlot, e2ee};
use desktop_runtime::Result;
use futures::future::BoxFuture;
use zeroize::Zeroizing;

use super::{auth::SecureStore, transport::failure};

/// The account names of this run's cloud secrets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SecretNames {
    /// A sandbox profile's namespace in the sandbox service.
    Sandbox(String),
    /// Shipping's `scope:key` accounts in the channel's service.
    Channel,
}

impl SecretNames {
    /// This app's sign-in secrets; shipping keeps its session elsewhere.
    pub fn auth(&self, name: &str) -> String {
        match self {
            Self::Sandbox(namespace) => format!("{namespace}:{name}"),
            Self::Channel => format!("desktop-auth:{name}"),
        }
    }

    /// The recovery key this device uses for the account.
    pub fn recovery(&self, account: &str) -> Result<String> {
        self.e2ee(account, "recovery", e2ee::recovery_key_name)
    }

    /// A recovery key shown during setup but not yet claimed on the server.
    pub fn pending_recovery(&self, account: &str) -> Result<String> {
        self.e2ee(
            account,
            "candidate-recovery",
            e2ee::pending_recovery_key_name,
        )
    }

    /// This device's key for receiving the recovery key from another device.
    pub fn enrollment(&self, account: &str) -> Result<String> {
        self.e2ee(account, "enrollment", e2ee::device_key_name)
    }

    pub fn share_link(&self, account: &str, share: &str) -> String {
        match self {
            Self::Sandbox(namespace) => format!("{namespace}:{account}:share:{share}:link"),
            Self::Channel => format!("share-links:{account}:{share}"),
        }
    }

    fn e2ee(
        &self,
        account: &str,
        sandbox: &str,
        shipping: fn(&str) -> std::result::Result<String, String>,
    ) -> Result<String> {
        match self {
            Self::Sandbox(namespace) => Ok(format!("{namespace}:{account}:{sandbox}")),
            Self::Channel => shipping(account)
                .map(|key| format!("{}:{key}", e2ee::SECRET_SCOPE))
                .map_err(|error| failure(&error)),
        }
    }
}

/// The channel's secure store behind this app's flat secret names: a name's first segment is
/// shipping's scope, so chunking, the dev prefix and legacy locations follow shipping.
pub struct ChannelStore<F> {
    store: Arc<anlg_desktop_identity::SecureStore<F>>,
}

impl ChannelStore<anlg_desktop_identity::secure_store::KeyringEntry> {
    pub fn keyring(identifier: &str) -> Self {
        Self::new(anlg_desktop_identity::SecureStore::keyring(identifier))
    }
}

impl<F, S> ChannelStore<F>
where
    F: Fn(&str, &str) -> keyring::Result<S> + Send + Sync + 'static,
    S: SecretSlot,
{
    pub fn new(store: anlg_desktop_identity::SecureStore<F>) -> Self {
        Self {
            store: Arc::new(store),
        }
    }

    fn run<T: Send + 'static>(
        &self,
        key: String,
        operation: impl FnOnce(
            &anlg_desktop_identity::SecureStore<F>,
            &str,
            &str,
        ) -> std::result::Result<T, String>
        + Send
        + 'static,
    ) -> BoxFuture<'static, Result<T>> {
        let store = self.store.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let (scope, key) = key
                    .split_once(':')
                    .ok_or_else(|| failure("Invalid secure-store name"))?;
                operation(&store, scope, key).map_err(|error| failure(&error))
            })
            .await
            .map_err(|_| failure("Secure credential storage is unavailable"))?
        })
    }
}

impl<F, S> SecureStore for ChannelStore<F>
where
    F: Fn(&str, &str) -> keyring::Result<S> + Send + Sync + 'static,
    S: SecretSlot,
{
    fn read(&self, key: String) -> BoxFuture<'static, Result<Option<Zeroizing<String>>>> {
        self.run(key, |store, scope, key| {
            Ok(store.read(scope, key)?.map(Zeroizing::new))
        })
    }

    fn write(&self, key: String, value: Zeroizing<String>) -> BoxFuture<'static, Result<()>> {
        self.run(key, move |store, scope, key| {
            store.write(scope, key, &value)
        })
    }

    fn remove(&self, key: String) -> BoxFuture<'static, Result<()>> {
        self.run(key, |store, scope, key| store.delete(scope, key))
    }
}

/// An in-memory keychain behind `SecretSlot`, rejecting values over Windows' limit like
/// shipping's tests do.
#[cfg(test)]
pub(crate) mod mock {
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
    };

    use anlg_desktop_identity::SecretSlot;

    const WINDOWS_MAX_UTF16_UNITS: usize = 1280;

    #[derive(Clone, Default)]
    pub struct Keychain(pub Arc<Mutex<BTreeMap<(String, String), String>>>);

    pub struct Item {
        keychain: Keychain,
        key: (String, String),
    }

    impl Keychain {
        pub fn entry(
            &self,
        ) -> impl Fn(&str, &str) -> keyring::Result<Item> + Send + Sync + 'static {
            let keychain = self.clone();
            move |service, account| {
                Ok(Item {
                    keychain: keychain.clone(),
                    key: (service.to_owned(), account.to_owned()),
                })
            }
        }

        pub fn insert(&self, service: &str, account: &str, value: &str) {
            self.0
                .lock()
                .unwrap()
                .insert((service.to_owned(), account.to_owned()), value.to_owned());
        }

        pub fn accounts(&self) -> Vec<(String, String)> {
            self.0.lock().unwrap().keys().cloned().collect()
        }
    }

    impl SecretSlot for Item {
        fn get(&self) -> keyring::Result<String> {
            let items = self.keychain.0.lock().unwrap();
            items.get(&self.key).cloned().ok_or(keyring::Error::NoEntry)
        }

        fn set(&self, value: &str) -> keyring::Result<()> {
            if value.encode_utf16().count() > WINDOWS_MAX_UTF16_UNITS {
                return Err(keyring::Error::TooLong("password".into(), 2560));
            }
            let mut items = self.keychain.0.lock().unwrap();
            items.insert(self.key.clone(), value.to_owned());
            Ok(())
        }

        fn delete(&self) -> keyring::Result<()> {
            let mut items = self.keychain.0.lock().unwrap();
            items
                .remove(&self.key)
                .map(|_| ())
                .ok_or(keyring::Error::NoEntry)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCOUNT: &str = "180DC97E-633A-4766-9184-799F89DB530B";

    #[test]
    fn channel_names_are_shippings_and_sandbox_names_stay_namespaced() {
        let account = ACCOUNT.to_lowercase();
        let channel = SecretNames::Channel;
        assert_eq!(
            channel.recovery(ACCOUNT).unwrap(),
            format!("e2ee:account:{account}:recovery-v1")
        );
        assert_eq!(
            channel.pending_recovery(ACCOUNT).unwrap(),
            format!("e2ee:account:{account}:recovery-v1:pending")
        );
        assert_eq!(
            channel.enrollment(ACCOUNT).unwrap(),
            format!("e2ee:account:{account}:device-enrollment-v1")
        );
        assert!(channel.recovery("not-an-account").is_err());

        let sandbox = SecretNames::Sandbox("gpui-device".into());
        assert_eq!(
            sandbox.recovery(ACCOUNT).unwrap(),
            format!("gpui-device:{ACCOUNT}:recovery")
        );
        assert_eq!(sandbox.auth("session"), "gpui-device:session");
    }

    /// The flat name maps to shipping's service, account and chunk layout per channel.
    #[tokio::test]
    async fn channel_store_reads_and_writes_shippings_keychain_items() {
        let keychain = mock::Keychain::default();
        let account = ACCOUNT.to_lowercase();
        let name = SecretNames::Channel.recovery(ACCOUNT).unwrap();
        keychain.insert(
            "com.anarlog.stable.secure-store",
            &format!("e2ee:account:{account}:recovery-v1"),
            "stable-code",
        );
        let stable = ChannelStore::new(anlg_desktop_identity::SecureStore::new(
            "com.hyprnote.stable",
            keychain.entry(),
        ));
        assert_eq!(
            stable
                .read(name.clone())
                .await
                .unwrap()
                .as_deref()
                .map(String::as_str),
            Some("stable-code")
        );

        let dev = ChannelStore::new(anlg_desktop_identity::SecureStore::new(
            "com.hyprnote.dev",
            keychain.entry(),
        ));
        dev.write(name.clone(), Zeroizing::new("x".repeat(3000)))
            .await
            .unwrap();
        assert_eq!(
            dev.read(name.clone())
                .await
                .unwrap()
                .map(|value| value.len()),
            Some(3000)
        );
        let dev_items = keychain
            .accounts()
            .into_iter()
            .filter(|(service, _)| service == "com.anarlog.dev.secure-store")
            .count();
        assert_eq!(dev_items, 4, "a manifest and three chunks");
        dev.remove(name.clone()).await.unwrap();
        assert_eq!(dev.read(name).await.unwrap(), None);
        assert_eq!(
            keychain.accounts(),
            [(
                "com.anarlog.stable.secure-store".to_owned(),
                format!("e2ee:account:{account}:recovery-v1")
            )]
        );
    }

    /// Long provider credentials (ChatGPT OAuth on Windows) are chunked like shipping writes
    /// them, under shipping's account for the channel.
    #[tokio::test]
    async fn provider_keys_use_shippings_chunked_accounts() {
        use crate::meeting::config::{ProviderKind, store_secrets};
        use crate::meeting::subscription::store_writer;
        let keychain = mock::Keychain::default();
        let store =
            || anlg_desktop_identity::SecureStore::new("com.hyprnote.stable", keychain.entry());
        let credential = "c".repeat(3000);
        store_writer(store())(ProviderKind::Llm, "openai".into(), credential.clone())
            .await
            .unwrap();
        assert_eq!(
            store_secrets(store())(ProviderKind::Llm, "openai".into())
                .await
                .unwrap(),
            Some(credential)
        );
        let accounts = keychain.accounts();
        assert_eq!(accounts.len(), 4, "a manifest and three chunks");
        assert!(accounts.iter().all(|(service, account)| {
            service == "com.anarlog.stable.secure-store"
                && account.starts_with("ai-provider-api-keys:llm:openai")
        }));
    }
}
