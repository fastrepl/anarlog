use crate::StoreKey;
use tauri_plugin_store2::{ScopedStore, Store2PluginExt};
pub trait AppExt<R: tauri::Runtime> {
    fn desktop_store(&self) -> Result<ScopedStore<R, crate::StoreKey>, String>;

    fn get_onboarding_needed(&self) -> Result<bool, String>;
    fn set_onboarding_needed(&self, v: bool) -> Result<(), String>;

    fn get_dismissed_toasts(&self) -> Result<Vec<String>, String>;
    fn set_dismissed_toasts(&self, v: Vec<String>) -> Result<(), String>;

    fn get_tinybase_values(&self) -> Result<Option<String>, String>;
    fn set_tinybase_values(&self, v: String) -> Result<(), String>;

    fn get_pinned_tabs(&self) -> Result<Option<String>, String>;
    fn set_pinned_tabs(&self, v: String) -> Result<(), String>;

    fn get_recently_opened_sessions(&self) -> Result<Option<String>, String>;
    fn set_recently_opened_sessions(&self, v: String) -> Result<(), String>;

    fn get_app_open_count(&self) -> Result<u32, String>;
    fn set_app_open_count(&self, v: u32) -> Result<(), String>;
    fn increment_app_open_count(&self) -> Result<u32, String>;

    fn get_survey_dismissed(&self) -> Result<bool, String>;
    fn set_survey_dismissed(&self, v: bool) -> Result<(), String>;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> AppExt<R> for T {
    #[tracing::instrument(skip_all)]
    fn desktop_store(&self) -> Result<ScopedStore<R, crate::StoreKey>, String> {
        self.store2()
            .scoped_store("desktop")
            .map_err(|e| e.to_string())
    }

    #[tracing::instrument(skip_all)]
    fn get_onboarding_needed(&self) -> Result<bool, String> {
        let store = self.desktop_store()?;
        store
            .get(StoreKey::OnboardingNeeded2)
            .map(|opt| opt.unwrap_or(true))
            .map_err(|e| e.to_string())
    }

    #[tracing::instrument(skip_all)]
    fn set_onboarding_needed(&self, v: bool) -> Result<(), String> {
        let store = self.desktop_store()?;
        store
            .set(StoreKey::OnboardingNeeded2, v)
            .map_err(|e| e.to_string())?;
        store.save().map_err(|e| e.to_string())
    }

    #[tracing::instrument(skip_all)]
    fn get_dismissed_toasts(&self) -> Result<Vec<String>, String> {
        let store = self.desktop_store()?;
        store
            .get(StoreKey::DismissedToasts)
            .map(|opt| opt.unwrap_or_default())
            .map_err(|e| e.to_string())
    }

    #[tracing::instrument(skip_all)]
    fn set_dismissed_toasts(&self, v: Vec<String>) -> Result<(), String> {
        let store = self.desktop_store()?;
        store
            .set(StoreKey::DismissedToasts, v)
            .map_err(|e| e.to_string())?;
        store.save().map_err(|e| e.to_string())
    }

    #[tracing::instrument(skip_all)]
    fn get_tinybase_values(&self) -> Result<Option<String>, String> {
        let store = self.desktop_store()?;
        store
            .get(StoreKey::TinybaseValues)
            .map_err(|e| e.to_string())
    }

    #[tracing::instrument(skip_all)]
    fn set_tinybase_values(&self, v: String) -> Result<(), String> {
        let store = self.desktop_store()?;
        store
            .set(StoreKey::TinybaseValues, v)
            .map_err(|e| e.to_string())?;
        store.save().map_err(|e| e.to_string())
    }

    #[tracing::instrument(skip_all)]
    fn get_pinned_tabs(&self) -> Result<Option<String>, String> {
        let store = self.desktop_store()?;
        store.get(StoreKey::PinnedTabs).map_err(|e| e.to_string())
    }

    #[tracing::instrument(skip_all)]
    fn set_pinned_tabs(&self, v: String) -> Result<(), String> {
        let store = self.desktop_store()?;
        store
            .set(StoreKey::PinnedTabs, v)
            .map_err(|e| e.to_string())?;
        store.save().map_err(|e| e.to_string())
    }

    #[tracing::instrument(skip_all)]
    fn get_recently_opened_sessions(&self) -> Result<Option<String>, String> {
        let store = self.desktop_store()?;
        store
            .get(StoreKey::RecentlyOpenedSessions)
            .map_err(|e| e.to_string())
    }

    #[tracing::instrument(skip_all)]
    fn set_recently_opened_sessions(&self, v: String) -> Result<(), String> {
        let store = self.desktop_store()?;
        store
            .set(StoreKey::RecentlyOpenedSessions, v)
            .map_err(|e| e.to_string())?;
        store.save().map_err(|e| e.to_string())
    }

    #[tracing::instrument(skip_all)]
    fn get_app_open_count(&self) -> Result<u32, String> {
        let store = self.desktop_store()?;
        store
            .get(StoreKey::AppOpenCount)
            .map(|opt: Option<u32>| opt.unwrap_or(0))
            .map_err(|e| e.to_string())
    }

    #[tracing::instrument(skip_all)]
    fn set_app_open_count(&self, v: u32) -> Result<(), String> {
        let store = self.desktop_store()?;
        store
            .set(StoreKey::AppOpenCount, v)
            .map_err(|e| e.to_string())?;
        store.save().map_err(|e| e.to_string())
    }

    #[tracing::instrument(skip_all)]
    fn increment_app_open_count(&self) -> Result<u32, String> {
        let current = self.get_app_open_count()?;
        let new_count = current + 1;
        let store = self.desktop_store()?;
        store
            .set(StoreKey::AppOpenCount, new_count)
            .map_err(|e| e.to_string())?;
        store.save().map_err(|e| e.to_string())?;
        Ok(new_count)
    }

    #[tracing::instrument(skip_all)]
    fn get_survey_dismissed(&self) -> Result<bool, String> {
        let store = self.desktop_store()?;
        store
            .get(StoreKey::SurveyDismissed)
            .map(|opt| opt.unwrap_or(false))
            .map_err(|e| e.to_string())
    }

    #[tracing::instrument(skip_all)]
    fn set_survey_dismissed(&self, v: bool) -> Result<(), String> {
        let store = self.desktop_store()?;
        store
            .set(StoreKey::SurveyDismissed, v)
            .map_err(|e| e.to_string())?;
        store.save().map_err(|e| e.to_string())
    }
}
