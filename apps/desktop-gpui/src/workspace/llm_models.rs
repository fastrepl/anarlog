//! The Intelligence page's model catalogue (`llm/select.tsx`): each
//! configured provider's `listModels` result cached by its config, fetched
//! through `ai_models::list_llm_models`, and `getDefaultLlmSelection` when
//! nothing usable is selected.

use std::collections::HashMap;

use gpui::Context;

use super::Workspace;
use super::ai_settings::ProviderKind;
use crate::ai_health::Health;
use crate::ai_models::{ListModelsResult, list_llm_models};
use crate::ai_providers::Provider;

pub(super) enum LlmModels {
    Loading,
    Loaded(ListModelsResult),
}

/// `["models", provider, listModels]`: one cache entry per provider config.
fn cache_key(provider_id: &str, base_url: &str, api_key: &str) -> String {
    format!("{provider_id}\u{1}{base_url}\u{1}{api_key}")
}

/// `getPreferredProviderModel`
pub(super) fn preferred_provider_model(
    saved: Option<&str>,
    models: &[String],
    allow_saved_without_choices: bool,
) -> String {
    if let Some(saved) = saved
        && models.iter().any(|model| model == saved)
    {
        return saved.to_string();
    }
    if let Some(first) = models.first() {
        return first.clone();
    }
    if allow_saved_without_choices {
        return saved.unwrap_or_default().to_string();
    }
    String::new()
}

impl Workspace {
    /// The cached catalogue for a provider's current config.
    pub(super) fn llm_models_for(&self, provider: &Provider) -> Option<&LlmModels> {
        let (base_url, api_key) = self.ai_provider_config(ProviderKind::Llm, provider);
        self.llm_models
            .get(&cache_key(provider.id, &base_url, &api_key))
    }

    /// The configured LLM providers with the selected one first
    /// (`getConfiguredProviderIds`).
    fn configured_llm_providers(&self) -> Vec<&'static Provider> {
        let current = self
            .provider_settings
            .string_setting("current_llm_provider", &["ai", "current_llm_provider"]);
        let mut providers: Vec<&'static Provider> = ProviderKind::Llm
            .providers()
            .iter()
            .filter(|provider| {
                !provider.disabled && self.ai_provider_configured(ProviderKind::Llm, provider)
            })
            .collect();
        if let Some(current) = current
            && let Some(index) = providers.iter().position(|provider| provider.id == current)
        {
            let preferred = providers.remove(index);
            providers.insert(0, preferred);
        }
        providers
    }

    /// Start (or with `force`, restart) the catalogue fetches the page needs:
    /// the selected provider's, plus every configured provider's while a
    /// default selection is pending.
    pub(super) fn ensure_llm_models(&mut self, force: bool, cx: &mut Context<Self>) {
        if !self.settings_open() && !force {
            return;
        }
        let providers = self.configured_llm_providers();
        let needs_default = self.needs_default_llm_selection();
        for (index, provider) in providers.iter().enumerate() {
            if index > 0 && !needs_default {
                break;
            }
            let (base_url, api_key) = self.ai_provider_config(ProviderKind::Llm, provider);
            let key = cache_key(provider.id, &base_url, &api_key);
            if !force && self.llm_models.contains_key(&key) {
                continue;
            }
            self.llm_models.insert(key.clone(), LlmModels::Loading);
            let provider_id = provider.id;
            let task = self
                .store
                .runtime()
                .spawn(async move { list_llm_models(provider_id, &base_url, &api_key).await });
            cx.spawn(async move |this, cx| {
                let result = task.await.unwrap_or_default();
                this.update(cx, |this, cx| {
                    this.llm_models.insert(key, LlmModels::Loaded(result));
                    this.apply_default_llm_selection(cx);
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        self.apply_default_llm_selection(cx);
        self.ensure_llm_health(force, cx);
    }

    /// The selected, configured provider with its config and model.
    fn selected_llm(&self) -> Option<(&'static Provider, String, String, String)> {
        let provider_id = self
            .provider_settings
            .string_setting("current_llm_provider", &["ai", "current_llm_provider"])?;
        let model = self
            .provider_settings
            .string_setting("current_llm_model", &["ai", "current_llm_model"])
            .filter(|model| !model.is_empty())?;
        let provider = ProviderKind::Llm
            .providers()
            .iter()
            .find(|provider| provider.id == provider_id)
            .filter(|provider| self.ai_provider_configured(ProviderKind::Llm, provider))?;
        let (base_url, api_key) = self.ai_provider_config(ProviderKind::Llm, provider);
        Some((provider, base_url, api_key, model))
    }

    /// `useConnectionHealth`'s status for the selected model.
    pub(super) fn llm_health_status(&self) -> Option<&Health> {
        let (provider, base_url, api_key, model) = self.selected_llm()?;
        self.llm_health.get(&cache_key(
            provider.id,
            &base_url,
            &format!("{api_key}\u{1}{model}"),
        ))
    }

    /// `useQuery(["llm-health-check", model])`: probe the selected model
    /// once per config (again with `force`, like the page's remount refetch).
    pub(super) fn ensure_llm_health(&mut self, force: bool, cx: &mut Context<Self>) {
        let Some((provider, base_url, api_key, model)) = self.selected_llm() else {
            return;
        };
        if !crate::ai_health::can_probe(provider.id, &base_url) {
            return;
        }
        let key = cache_key(provider.id, &base_url, &format!("{api_key}\u{1}{model}"));
        if !force && self.llm_health.contains_key(&key) {
            return;
        }
        self.llm_health.insert(key.clone(), Health::Pending);
        let provider_id = provider.id;
        let task = self.store.runtime().spawn(async move {
            crate::ai_health::check(provider_id, &base_url, &api_key, &model).await
        });
        cx.spawn(async move |this, cx| {
            let health = task
                .await
                .unwrap_or_else(|_| Health::Error("Connection failed: Unknown error".into()));
            this.update(cx, |this, cx| {
                this.llm_health.insert(key, health);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// `handleProviderChange`: the provider switches at once with the model
    /// it last had, the cached catalogue's preferred one, or nothing until
    /// the catalogue lands and the default selection fills it in. Effort
    /// support differs per model, so the level resets like
    /// `persistSelection` does.
    pub(super) fn change_llm_provider(&mut self, provider_id: String, cx: &mut Context<Self>) {
        let current_provider = self
            .provider_settings
            .string_setting("current_llm_provider", &["ai", "current_llm_provider"]);
        let current_model = self
            .provider_settings
            .string_setting("current_llm_model", &["ai", "current_llm_model"]);
        if let (Some(provider), Some(model)) = (current_provider, current_model)
            && !model.is_empty()
        {
            self.last_llm_models.insert(provider, model);
        }
        let cached: Vec<String> = ProviderKind::Llm
            .providers()
            .iter()
            .find(|provider| provider.id == provider_id)
            .and_then(|provider| match self.llm_models_for(provider) {
                Some(LlmModels::Loaded(result)) => Some(result.models.clone()),
                _ => None,
            })
            .unwrap_or_default();
        let model = preferred_provider_model(
            self.last_llm_models.get(&provider_id).map(String::as_str),
            &cached,
            provider_id == "custom",
        );
        if !model.is_empty() {
            self.last_llm_models
                .insert(provider_id.clone(), model.clone());
        }
        self.applying_llm_default = true;
        self.set_setting(
            "current_llm_provider",
            serde_json::Value::String(provider_id),
            cx,
        );
        self.set_setting("current_llm_model", serde_json::Value::String(model), cx);
        self.set_setting(
            "current_llm_reasoning_effort",
            serde_json::Value::String("default".to_string()),
            cx,
        );
        self.applying_llm_default = false;
        self.ensure_llm_models(false, cx);
    }

    /// `needsDefaultSelection`: no configured provider with a model chosen.
    fn needs_default_llm_selection(&self) -> bool {
        let provider = self
            .provider_settings
            .string_setting("current_llm_provider", &["ai", "current_llm_provider"]);
        let model = self
            .provider_settings
            .string_setting("current_llm_model", &["ai", "current_llm_model"])
            .unwrap_or_default();
        let configured = provider.as_deref().is_some_and(|id| {
            ProviderKind::Llm.providers().iter().any(|provider| {
                provider.id == id && self.ai_provider_configured(ProviderKind::Llm, provider)
            })
        });
        !configured || model.is_empty()
    }

    /// `getDefaultLlmSelection` + `PersistAiSelection`: the first configured
    /// provider (selected one first) whose catalogue offers a model, the
    /// saved model kept when it is listed.
    fn apply_default_llm_selection(&mut self, cx: &mut Context<Self>) {
        // `set_setting` re-enters `ensure_llm_models`; the two writes below
        // are one `PersistAiSelection`.
        if self.applying_llm_default || !self.needs_default_llm_selection() {
            return;
        }
        let current_provider = self
            .provider_settings
            .string_setting("current_llm_provider", &["ai", "current_llm_provider"]);
        let current_model = self
            .provider_settings
            .string_setting("current_llm_model", &["ai", "current_llm_model"]);
        for provider in self.configured_llm_providers() {
            let Some(LlmModels::Loaded(result)) = self.llm_models_for(provider) else {
                // Still loading (or not requested): decide once it lands.
                return;
            };
            let saved = (current_provider.as_deref() == Some(provider.id))
                .then_some(current_model.as_deref())
                .flatten();
            let model = preferred_provider_model(saved, &result.models, provider.id == "custom");
            if !model.is_empty() {
                let provider_id = provider.id.to_string();
                self.applying_llm_default = true;
                self.set_setting(
                    "current_llm_provider",
                    serde_json::Value::String(provider_id),
                    cx,
                );
                self.set_setting("current_llm_model", serde_json::Value::String(model), cx);
                self.applying_llm_default = false;
                return;
            }
        }
    }
}

pub(super) type LlmModelsCache = HashMap<String, LlmModels>;
pub(super) type LlmHealthCache = HashMap<String, Health>;
/// `lastSelectedModelsRef`: the model each provider last had.
pub(super) type LastLlmModels = HashMap<String, String>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferred_model_keeps_the_saved_one_when_listed() {
        let models = vec!["a".to_string(), "b".to_string()];
        assert_eq!(preferred_provider_model(Some("b"), &models, false), "b");
        assert_eq!(preferred_provider_model(Some("zzz"), &models, false), "a");
        assert_eq!(preferred_provider_model(None, &models, false), "a");
        assert_eq!(preferred_provider_model(Some("mine"), &[], false), "");
        // `allowSavedModelWithoutChoices` for the custom provider.
        assert_eq!(preferred_provider_model(Some("mine"), &[], true), "mine");
        assert_eq!(preferred_provider_model(None, &[], true), "");
    }
}
