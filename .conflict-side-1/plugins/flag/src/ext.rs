use crate::{Feature, FlagStrategy, ManagedState};

pub struct Flag<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Flag<'a, R, M> {
    pub async fn is_enabled(&self, feature: Feature) -> bool {
        match feature.strategy() {
            FlagStrategy::Debug => cfg!(debug_assertions),
            FlagStrategy::Hardcoded(v) => v,
            FlagStrategy::Posthog(key) => self.get_posthog_flag(key).await,
        }
    }

    async fn get_posthog_flag(&self, flag_key: &str) -> bool {
        let client = self.manager.state::<ManagedState>();
        let distinct_id = hypr_host::fingerprint();
        client
            .is_feature_enabled(flag_key, &distinct_id)
            .await
            .unwrap_or(false)
    }
}

pub trait FlagPluginExt<R: tauri::Runtime> {
    fn flag(&self) -> Flag<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> FlagPluginExt<R> for T {
    fn flag(&self) -> Flag<'_, R, Self>
    where
        Self: Sized,
    {
        Flag {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
