use std::collections::HashMap;

use hypr_supabase_auth::client::{session::find_session, store::AuthStore};
use hypr_template_support::AccountInfo;

pub(crate) fn parse_account_info(
    data: &HashMap<String, String>,
) -> Result<Option<AccountInfo>, crate::Error> {
    let Some(session) = find_session(data)? else {
        return Ok(None);
    };
    let metadata = session.user.user_metadata;
    Ok(Some(AccountInfo {
        user_id: session.user.id,
        email: session.user.email,
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
        self.state::<AuthStore>().set(key, value)
    }

    fn remove_item(&self, key: String) -> Result<(), crate::Error> {
        self.state::<AuthStore>().remove(&key)
    }

    fn clear_auth(&self) -> Result<(), crate::Error> {
        self.state::<AuthStore>().clear()
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
