use std::collections::HashMap;

use anlg_supabase_auth::{client::store::AuthStore, session::find_session};

use crate::AccountInfo;

#[cfg(all(any(target_os = "linux", target_os = "windows"), not(test)))]
static SECURE_AUTH_WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) fn parse_account_info(
    data: &HashMap<String, String>,
) -> Result<Option<AccountInfo>, crate::Error> {
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

pub trait AuthPluginExt<R: tauri::Runtime> {
    fn get_item(&self, key: String) -> Result<Option<String>, crate::Error>;
    fn set_item(&self, key: String, value: String) -> Result<(), crate::Error>;
    fn remove_item(&self, key: String) -> Result<(), crate::Error>;
    fn clear_auth(&self) -> Result<(), crate::Error>;
    fn get_account_info(&self) -> Result<Option<AccountInfo>, crate::Error>;
    fn access_token(&self) -> Result<Option<String>, crate::Error>;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> AuthPluginExt<R> for T {
    fn get_item(&self, key: String) -> Result<Option<String>, crate::Error> {
        Ok(self.state::<AuthStore>().get(&key))
    }

    fn set_item(&self, key: String, value: String) -> Result<(), crate::Error> {
        let store = self.state::<AuthStore>();

        #[cfg(all(any(target_os = "linux", target_os = "windows"), not(test)))]
        {
            let _guard = SECURE_AUTH_WRITE_LOCK.lock().unwrap();
            store.set(key, value)?;

            #[cfg(target_os = "linux")]
            let result = crate::migrate::persist_linux_auth(self.app_handle(), &store.snapshot());

            #[cfg(target_os = "windows")]
            let result = crate::windows::persist_auth(self.app_handle(), &store.snapshot());

            result
        }

        #[cfg(any(not(any(target_os = "linux", target_os = "windows")), test))]
        {
            store.set(key, value).map_err(Into::into)
        }
    }

    fn remove_item(&self, key: String) -> Result<(), crate::Error> {
        let store = self.state::<AuthStore>();

        #[cfg(all(any(target_os = "linux", target_os = "windows"), not(test)))]
        {
            let _guard = SECURE_AUTH_WRITE_LOCK.lock().unwrap();
            let mut data = store.snapshot();
            data.remove(&key);
            // Apply the removal in memory even when the secure store refuses, so a
            // reported failure cannot leave the value readable via get_item.
            #[cfg(target_os = "linux")]
            let result = crate::migrate::persist_linux_auth(self.app_handle(), &data);

            #[cfg(target_os = "windows")]
            let result = crate::windows::persist_auth(self.app_handle(), &data);

            store.replace(data);
            result
        }

        #[cfg(any(not(any(target_os = "linux", target_os = "windows")), test))]
        {
            store.remove(&key).map_err(Into::into)
        }
    }

    fn clear_auth(&self) -> Result<(), crate::Error> {
        let store = self.state::<AuthStore>();

        #[cfg(all(any(target_os = "linux", target_os = "windows"), not(test)))]
        {
            let _guard = SECURE_AUTH_WRITE_LOCK.lock().unwrap();
            // Sign-out must drop the in-memory session even if secure persistence
            // fails; callers that ignore the error must not retain readable auth.
            #[cfg(target_os = "linux")]
            let result = crate::migrate::clear_linux_auth(self.app_handle());

            #[cfg(target_os = "windows")]
            let result = crate::windows::clear_auth(self.app_handle());

            store.replace(HashMap::new());
            result
        }

        #[cfg(any(not(any(target_os = "linux", target_os = "windows")), test))]
        {
            store.clear().map_err(Into::into)
        }
    }

    fn get_account_info(&self) -> Result<Option<AccountInfo>, crate::Error> {
        let data = self.state::<AuthStore>().snapshot();
        parse_account_info(&data)
    }

    fn access_token(&self) -> Result<Option<String>, crate::Error> {
        let Some(store) = self.try_state::<AuthStore>() else {
            return Ok(None);
        };
        Ok(find_session(&store.snapshot())?.map(|s| s.access_token))
    }
}
