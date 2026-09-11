//! The Transcription page's model selection (`stt/select.tsx`,
//! `stt/model-selection.ts`): each configured provider's model rows, the
//! default the page persists when nothing usable is selected
//! (`getDefaultSttSelection` + `PersistAiSelection`), and
//! `handleProviderChange`.

use gpui::Context;

use super::Workspace;
use super::ai_settings::ProviderKind;
use super::llm_models::preferred_provider_model;
use crate::ai_providers::Provider;

/// `DEFAULT_EXTERNAL_STT_MODELS`: the model a provider starts on when it
/// lists none to pick from.
const DEFAULT_EXTERNAL_STT_MODELS: &[(&str, &str)] = &[
    ("local_file", "local-file"),
    ("deepgram", "nova-3-general"),
    ("assemblyai", "universal-3-5-pro"),
    ("openai", "gpt-live-transcribe"),
    ("openrouter", "openai/gpt-transcribe"),
    ("cartesia", "ink-2"),
    ("cloudflare_workers_ai", "nova-3"),
    ("gladia", "solaria-1"),
    ("soniox", "stt-rt-v5"),
    ("elevenlabs", "scribe_v2"),
    ("mistral", "voxtral-mini-2602"),
    ("meta", "muse-voice-transcribe-1.0"),
    ("pyannote", "parakeet-tdt-0.6b-v3"),
    ("aquavoice", "avalon-v1.5"),
    ("cohere", "cohere-transcribe-03-2026"),
    ("dashscope", "qwen3-asr-flash-realtime"),
    ("zai", "glm-asr-2512"),
    ("siliconflow", "FunAudioLLM/SenseVoiceSmall"),
    ("fireworks", "whisper-v3-turbo"),
    ("groq", "whisper-large-v3-turbo"),
    ("xai", "xai-stt"),
    ("smallestai", "pulse"),
    ("together", "openai/whisper-large-v3"),
    ("speechmatics", "enhanced"),
    ("azure_speech", "fast-transcription"),
    ("google_cloud", "latest_long"),
    ("google_generative_ai", "gemini-3.5-transcribe-live"),
    ("aws_transcribe", "amazon-transcribe"),
    ("revai", "machine"),
];

/// `getDefaultSttModel`
pub(super) fn default_stt_model(provider: &str) -> Option<&'static str> {
    DEFAULT_EXTERNAL_STT_MODELS
        .iter()
        .find(|(id, _)| *id == provider)
        .map(|(_, model)| *model)
}

/// `normalizeStoredSttModel`: the ids the providers retired, mapped onto
/// their successors before the saved selection is read.
pub(super) fn normalize_stored_stt_model(provider: &str, model: &str) -> String {
    match (provider, model) {
        ("assemblyai", "universal" | "universal-3-pro") => "universal-3-5-pro",
        ("assemblyai", "u3-rt-pro") => "universal-3-5-pro-realtime",
        ("aquavoice", "avalon-v1-en") => "avalon-v1.5",
        ("soniox", _)
            if model
                .strip_prefix("stt-")
                .map(|rest| rest.trim_start_matches("async-").trim_start_matches("rt-"))
                .is_some_and(|rest| matches!(rest, "v3" | "v4" | "v5")) =>
        {
            "stt-rt-v5"
        }
        ("openai", "gpt-4o-transcribe" | "gpt-4o-mini-transcribe") => "gpt-transcribe",
        ("openrouter", "openai/gpt-4o-transcribe" | "openai/gpt-4o-mini-transcribe") => {
            "openai/gpt-transcribe"
        }
        _ => model,
    }
    .to_string()
}

/// `isConfiguredSttModel`: the saved model belongs to the saved provider.
pub(super) fn is_configured_stt_model(provider: &Provider, model: &str) -> bool {
    if model.is_empty() {
        return false;
    }
    match provider.id {
        "anarlog" => model == "cloud" || crate::db::is_on_device_stt_model("anarlog", model),
        "soniqo" => model.starts_with("soniqo-"),
        "apple_speech" => model == "apple-speech",
        "local_file" => model == "local-file",
        _ => true,
    }
}

/// `HealthStatus` of `stt/health.tsx`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SttHealth {
    Pending,
    Success,
    Error(String),
}

impl Workspace {
    /// `useConnectionHealth` for the Transcription page: the selected model's
    /// state — a managed provider's stale model, an unconfigured provider,
    /// Deepgram's `GET /v1/projects` key check (pending / failed / passed),
    /// and every other cloud provider as healthy. `None` without a selection.
    pub(super) fn stt_health_status(&self) -> Option<SttHealth> {
        let provider = self.provider_settings.stt_provider.clone()?;
        let model = self.provider_settings.stt_model.clone().unwrap_or_default();
        if model.is_empty() {
            return None;
        }
        let managed = matches!(
            provider.as_str(),
            "anarlog" | "soniqo" | "apple_speech" | "local_file"
        );
        let local = crate::db::is_on_device_stt_model(&provider, &model)
            || crate::db::is_local_file_stt_model(&provider, &model);
        let cloud = crate::db::is_anarlog_cloud_stt_model(&provider, &model) || !managed;
        if managed && !cloud && !local {
            return Some(SttHealth::Error(
                "Selected model is no longer available.".to_string(),
            ));
        }
        if local {
            // The on-device servers exist on Apple Silicon only.
            return Some(SttHealth::Error(
                "Could not connect to the local speech-to-text model.".to_string(),
            ));
        }
        let entry = ProviderKind::Stt
            .providers()
            .iter()
            .find(|entry| entry.id == provider)?;
        if !self.ai_provider_configured(ProviderKind::Stt, entry) {
            return Some(SttHealth::Error("Provider not configured.".to_string()));
        }
        if provider == "deepgram" {
            let (_, api_key) = self.ai_provider_config(ProviderKind::Stt, entry);
            return Some(match self.deepgram_health.get(&api_key) {
                None | Some(SttHealth::Pending) => SttHealth::Pending,
                Some(status) => status.clone(),
            });
        }
        Some(SttHealth::Success)
    }

    /// `useDeepgramHealth`: `GET https://api.deepgram.com/v1/projects` with
    /// the key, `retry: 3` at 200ms, once per key.
    pub(super) fn ensure_deepgram_health(&mut self, cx: &mut Context<Self>) {
        if self.provider_settings.stt_provider.as_deref() != Some("deepgram") {
            return;
        }
        let Some(entry) = ProviderKind::Stt
            .providers()
            .iter()
            .find(|entry| entry.id == "deepgram")
        else {
            return;
        };
        if !self.ai_provider_configured(ProviderKind::Stt, entry) {
            return;
        }
        let (_, api_key) = self.ai_provider_config(ProviderKind::Stt, entry);
        if self.deepgram_health.contains_key(&api_key) {
            return;
        }
        self.deepgram_health
            .insert(api_key.clone(), SttHealth::Pending);
        let key = api_key.clone();
        let task = self.store.runtime().spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .map_err(|error| error.to_string())?;
            let mut last = String::new();
            for attempt in 0..4 {
                if attempt > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
                match client
                    .get("https://api.deepgram.com/v1/projects")
                    .header("Authorization", format!("Token {api_key}"))
                    .send()
                    .await
                {
                    Ok(response) if response.status().is_success() => return Ok(()),
                    Ok(response) => {
                        last = format!(
                            "{} {}",
                            response.status().as_u16(),
                            response.status().canonical_reason().unwrap_or_default()
                        );
                    }
                    Err(error) => last = error.to_string(),
                }
            }
            Err(last)
        });
        cx.spawn(async move |this, cx| {
            let status = match task.await {
                Ok(Ok(())) => SttHealth::Success,
                Ok(Err(message)) => {
                    SttHealth::Error(format!("API key verification failed: {message}"))
                }
                Err(error) => SttHealth::Error(format!("API key verification failed: {error}")),
            };
            this.update(cx, |this, cx| {
                this.deepgram_health.insert(key, status);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// `useConfiguredMapping`'s models of a configured provider that the page
    /// can select (`isDownloaded`): the cloud model needs a paid plan, the
    /// registry's list otherwise, none for `custom`.
    pub(super) fn selectable_stt_models(&self, provider: &Provider) -> Vec<String> {
        match provider.id {
            "anarlog" => {
                if self.is_pro() {
                    vec!["cloud".to_string()]
                } else {
                    Vec::new()
                }
            }
            "custom" => Vec::new(),
            _ => provider
                .models
                .iter()
                .map(|model| model.to_string())
                .collect(),
        }
    }

    /// `getConfiguredProviderIds`: the configured STT providers, the selected
    /// one first.
    fn configured_stt_providers(&self) -> Vec<&'static Provider> {
        let current = self
            .provider_settings
            .string_setting("current_stt_provider", &["ai", "current_stt_provider"]);
        let mut providers: Vec<&'static Provider> = ProviderKind::Stt
            .providers()
            .iter()
            .filter(|provider| {
                !provider.disabled && self.ai_provider_configured(ProviderKind::Stt, provider)
            })
            .collect();
        if let Some(current) = current
            && let Some(index) = providers.iter().position(|provider| provider.id == current)
        {
            let selected = providers.remove(index);
            providers.insert(0, selected);
        }
        providers
    }

    /// `!visibleSelection.model`: no configured provider with one of its
    /// models chosen.
    fn needs_default_stt_selection(&self) -> bool {
        let provider = self
            .provider_settings
            .string_setting("current_stt_provider", &["ai", "current_stt_provider"]);
        let model = self
            .provider_settings
            .string_setting("current_stt_model", &["ai", "current_stt_model"])
            .unwrap_or_default();
        !provider.as_deref().is_some_and(|id| {
            ProviderKind::Stt.providers().iter().any(|provider| {
                provider.id == id
                    && self.ai_provider_configured(ProviderKind::Stt, provider)
                    && is_configured_stt_model(provider, &model)
            })
        })
    }

    /// `getDefaultSttSelection` + `PersistAiSelection`: the first configured
    /// provider (selected one first) with a selectable model — the saved one
    /// when it is listed, else the first — written as the selection.
    pub(super) fn apply_default_stt_selection(&mut self, cx: &mut Context<Self>) {
        // `defaultSelection && !pendingProvider`.
        if self.applying_stt_default
            || self.pending_stt_provider.is_some()
            || !self.needs_default_stt_selection()
        {
            return;
        }
        let current_provider = self
            .provider_settings
            .string_setting("current_stt_provider", &["ai", "current_stt_provider"]);
        let current_model = self
            .provider_settings
            .string_setting("current_stt_model", &["ai", "current_stt_model"]);
        for provider in self.configured_stt_providers() {
            let saved = (current_provider.as_deref() == Some(provider.id))
                .then(|| {
                    current_model
                        .as_deref()
                        .map(|model| normalize_stored_stt_model(provider.id, model))
                })
                .flatten();
            let model = preferred_provider_model(
                saved.as_deref(),
                &self.selectable_stt_models(provider),
                provider.id == "custom",
            );
            if !model.is_empty() {
                self.persist_stt_selection(provider.id.to_string(), model, cx);
                return;
            }
        }
    }

    /// `handleProviderChange`: the provider switches with the model it last
    /// had, the first it lists, or its `DEFAULT_EXTERNAL_STT_MODELS` entry;
    /// with none of those the provider shows with an empty model
    /// (`pendingProvider`).
    pub(super) fn change_stt_provider(&mut self, provider_id: String, cx: &mut Context<Self>) {
        let current_provider = self
            .provider_settings
            .string_setting("current_stt_provider", &["ai", "current_stt_provider"]);
        let current_model = self
            .provider_settings
            .string_setting("current_stt_model", &["ai", "current_stt_model"]);
        if let (Some(provider), Some(model)) = (current_provider, current_model)
            && !model.is_empty()
        {
            self.last_stt_models.insert(provider, model);
        }
        let models = ProviderKind::Stt
            .providers()
            .iter()
            .find(|provider| provider.id == provider_id)
            .map(|provider| self.selectable_stt_models(provider))
            .unwrap_or_default();
        let mut model = preferred_provider_model(
            self.last_stt_models.get(&provider_id).map(String::as_str),
            &models,
            provider_id == "custom",
        );
        if model.is_empty() {
            model = default_stt_model(&provider_id)
                .unwrap_or_default()
                .to_string();
        }
        if model.is_empty() {
            // `setPendingProvider(providerId)`: shown with an empty model,
            // nothing written until a model is picked.
            self.pending_stt_provider = Some(provider_id);
            cx.notify();
            return;
        }
        self.pending_stt_provider = None;
        self.last_stt_models
            .insert(provider_id.clone(), model.clone());
        self.persist_stt_selection(provider_id, model, cx);
    }

    /// `handleModelChange`
    pub(super) fn change_stt_model(&mut self, model: String, cx: &mut Context<Self>) {
        let Some(provider) = self.pending_stt_provider.take().or_else(|| {
            self.provider_settings
                .string_setting("current_stt_provider", &["ai", "current_stt_provider"])
        }) else {
            return;
        };
        self.last_stt_models.insert(provider.clone(), model.clone());
        self.persist_stt_selection(provider, model, cx);
    }

    /// One `setSelection({ current_stt_provider, current_stt_model })`.
    fn persist_stt_selection(&mut self, provider: String, model: String, cx: &mut Context<Self>) {
        self.applying_stt_default = true;
        self.set_setting(
            "current_stt_provider",
            serde_json::Value::String(provider),
            cx,
        );
        self.set_setting("current_stt_model", serde_json::Value::String(model), cx);
        self.applying_stt_default = false;
        self.ensure_deepgram_health(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retired_models_map_onto_their_successors() {
        assert_eq!(
            normalize_stored_stt_model("assemblyai", "universal"),
            "universal-3-5-pro"
        );
        assert_eq!(
            normalize_stored_stt_model("assemblyai", "u3-rt-pro"),
            "universal-3-5-pro-realtime"
        );
        assert_eq!(
            normalize_stored_stt_model("soniox", "stt-async-v4"),
            "stt-rt-v5"
        );
        assert_eq!(normalize_stored_stt_model("soniox", "stt-v3"), "stt-rt-v5");
        assert_eq!(
            normalize_stored_stt_model("soniox", "stt-rt-v5"),
            "stt-rt-v5"
        );
        assert_eq!(normalize_stored_stt_model("soniox", "stt-v6"), "stt-v6");
        assert_eq!(
            normalize_stored_stt_model("openai", "gpt-4o-mini-transcribe"),
            "gpt-transcribe"
        );
        assert_eq!(
            normalize_stored_stt_model("openrouter", "openai/gpt-4o-transcribe"),
            "openai/gpt-transcribe"
        );
        assert_eq!(normalize_stored_stt_model("deepgram", "nova-3"), "nova-3");
        assert_eq!(default_stt_model("openai"), Some("gpt-live-transcribe"));
        assert_eq!(default_stt_model("custom"), None);
    }
}
