use crate::StoreKey;
use tauri_plugin_store2::{ScopedStore, Store2PluginExt};

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(default, rename_all = "camelCase")]
pub struct OnboardingSurveyState {
    pub launch_count: u32,
    pub done: bool,
}

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

    fn get_onboarding_survey_state(&self) -> Result<OnboardingSurveyState, String>;
    fn set_onboarding_survey_state(&self, state: OnboardingSurveyState) -> Result<(), String>;
    fn record_onboarding_survey_launch(&self) -> Result<OnboardingSurveyState, String>;
    fn finish_onboarding_survey(&self) -> Result<OnboardingSurveyState, String>;
    fn reset_onboarding_survey(&self) -> Result<OnboardingSurveyState, String>;

    fn get_char_v1p1_preview(&self) -> Result<bool, String>;
    fn set_char_v1p1_preview(&self, v: bool) -> Result<(), String>;
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
    fn get_onboarding_survey_state(&self) -> Result<OnboardingSurveyState, String> {
        let store = self.desktop_store()?;
        store
            .get(StoreKey::OnboardingSurvey)
            .map(|opt| opt.unwrap_or_default())
            .map_err(|e| e.to_string())
    }

    #[tracing::instrument(skip_all)]
    fn set_onboarding_survey_state(&self, state: OnboardingSurveyState) -> Result<(), String> {
        let store = self.desktop_store()?;
        store
            .set(StoreKey::OnboardingSurvey, state)
            .map_err(|e| e.to_string())?;
        store.save().map_err(|e| e.to_string())
    }

    #[tracing::instrument(skip_all)]
    fn record_onboarding_survey_launch(&self) -> Result<OnboardingSurveyState, String> {
        let mut state = self.get_onboarding_survey_state()?;
        state.launch_count = state.launch_count.saturating_add(1);
        self.set_onboarding_survey_state(state.clone())?;
        Ok(state)
    }

    #[tracing::instrument(skip_all)]
    fn finish_onboarding_survey(&self) -> Result<OnboardingSurveyState, String> {
        let mut state = self.get_onboarding_survey_state()?;
        state.done = true;
        self.set_onboarding_survey_state(state.clone())?;
        Ok(state)
    }

    #[tracing::instrument(skip_all)]
    fn reset_onboarding_survey(&self) -> Result<OnboardingSurveyState, String> {
        let state = OnboardingSurveyState::default();
        self.set_onboarding_survey_state(state.clone())?;
        Ok(state)
    }

    #[tracing::instrument(skip_all)]
    fn get_char_v1p1_preview(&self) -> Result<bool, String> {
        if cfg!(feature = "new") {
            return Ok(true);
        }
        let store = self.desktop_store()?;
        store
            .get(StoreKey::CharV1p1Preview)
            .map(|opt| opt.unwrap_or(false))
            .map_err(|e| e.to_string())
    }

    #[tracing::instrument(skip_all)]
    fn set_char_v1p1_preview(&self, v: bool) -> Result<(), String> {
        let store = self.desktop_store()?;
        store
            .set(StoreKey::CharV1p1Preview, v)
            .map_err(|e| e.to_string())?;
        store.save().map_err(|e| e.to_string())
    }
}
