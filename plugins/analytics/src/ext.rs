use std::time::SystemTime;

use tauri_plugin_misc::MiscPluginExt;
use tauri_plugin_store2::Store2PluginExt;

pub struct Analytics<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Analytics<'a, R, M> {
    pub async fn event(
        &self,
        mut payload: hypr_analytics::AnalyticsPayload,
    ) -> Result<(), crate::Error> {
        if self.is_disabled().unwrap_or(true) {
            return Ok(());
        }

        Self::enrich_payload(self.manager, &mut payload);

        let machine_id = hypr_host::fingerprint();
        let state = self.manager.state::<crate::ManagedState>();
        state
            .client
            .event(machine_id, payload)
            .await
            .map_err(crate::Error::HyprAnalytics)?;

        Ok(())
    }

    pub fn event_fire_and_forget(&self, mut payload: hypr_analytics::AnalyticsPayload) {
        if self.is_disabled().unwrap_or(true) {
            return;
        }

        Self::enrich_payload(self.manager, &mut payload);

        let machine_id = hypr_host::fingerprint();
        let client = self.manager.state::<crate::ManagedState>().client.clone();

        tauri::async_runtime::spawn(async move {
            let _ = client.event(machine_id, payload).await;
        });
    }

    fn session_id(manager: &M) -> String {
        let state = manager.state::<crate::ManagedState>();
        let mut session = state
            .session
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        session.session_id(SystemTime::now())
    }

    fn enrich_payload(manager: &M, payload: &mut hypr_analytics::AnalyticsPayload) {
        let app_version = env!("APP_VERSION");
        let app_identifier = manager.config().identifier.clone();
        let git_hash = manager.misc().get_git_hash();
        let bundle_id = manager.config().identifier.clone();
        let session_id = Self::session_id(manager);
        let groups = {
            let state = manager.state::<crate::ManagedState>();
            state
                .groups
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone()
        };

        payload
            .props
            .insert("$session_id".into(), session_id.into());

        for (group_type, group_key) in groups {
            payload
                .groups
                .get_or_insert_default()
                .entry(group_type)
                .or_insert(group_key);
        }

        payload
            .props
            .entry("app_version".into())
            .or_insert(app_version.into());

        payload
            .props
            .entry("surface".into())
            .or_insert("desktop".into());

        payload
            .props
            .entry("analytics_schema_version".into())
            .or_insert(1.into());

        payload
            .props
            .entry("app_identifier".into())
            .or_insert(app_identifier.into());

        payload
            .props
            .entry("git_hash".into())
            .or_insert(git_hash.into());

        payload
            .props
            .entry("bundle_id".into())
            .or_insert(bundle_id.into());

        payload.props.entry("$set".into()).or_insert_with(|| {
            serde_json::json!({
                "app_version": app_version
            })
        });
    }

    pub fn set_disabled(&self, disabled: bool) -> Result<(), crate::Error> {
        {
            let store = self.manager.store2().scoped_store(crate::PLUGIN_NAME)?;
            store.set(crate::StoreKey::Disabled, disabled)?;
        }
        Ok(())
    }

    pub fn is_disabled(&self) -> Result<bool, crate::Error> {
        let store = self.manager.store2().scoped_store(crate::PLUGIN_NAME)?;
        let v = store.get(crate::StoreKey::Disabled)?.unwrap_or(false);
        Ok(v)
    }

    pub async fn set_properties(
        &self,
        payload: hypr_analytics::PropertiesPayload,
    ) -> Result<(), crate::Error> {
        if !self.is_disabled()? {
            let machine_id = hypr_host::fingerprint();

            let state = self.manager.state::<crate::ManagedState>();
            state
                .client
                .set_properties(machine_id, payload)
                .await
                .map_err(crate::Error::HyprAnalytics)?;
        }

        Ok(())
    }

    pub async fn identify(
        &self,
        user_id: impl Into<String>,
        payload: hypr_analytics::PropertiesPayload,
    ) -> Result<(), crate::Error> {
        if !self.is_disabled()? {
            let machine_id = hypr_host::fingerprint();
            let user_id = user_id.into();

            let state = self.manager.state::<crate::ManagedState>();
            if let Some(group) = &payload.group {
                state
                    .groups
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .insert(group.r#type.clone(), group.key.clone());
            }
            state
                .client
                .identify(user_id, machine_id, payload)
                .await
                .map_err(crate::Error::HyprAnalytics)?;
        }

        Ok(())
    }

    pub fn clear_groups(&self) {
        self.manager
            .state::<crate::ManagedState>()
            .groups
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
    }
}

pub trait AnalyticsPluginExt<R: tauri::Runtime> {
    fn analytics(&self) -> Analytics<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> AnalyticsPluginExt<R> for T {
    fn analytics(&self) -> Analytics<'_, R, Self>
    where
        Self: Sized,
    {
        Analytics {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
