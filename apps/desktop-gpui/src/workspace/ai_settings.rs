//! The Transcription and Intelligence settings pages:
//! `apps/desktop/src/settings/ai/{stt,llm}` (`SelectProviderAndModel`,
//! `ConfigureProviders`, `NonAnarlogProviderCard`, `ProviderSearch`).

use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Context, Div, Entity, Focusable as _, ImageSource, MouseButton,
    Resource, SharedString, Window, div, img, prelude::*, px,
};

use super::Workspace;
use super::settings::{SelectOption, SelectSpec};
use crate::ai_providers::{Icon, LLM_PROVIDERS, Provider, Requirement, STT_PROVIDERS};
use crate::text_input::{TextInput, TextInputEvent, TextInputStyle};
use crate::theme::alpha;
use crate::ui::{TailwindText as _, icon};

/// Which registry a page shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ProviderKind {
    Stt,
    Llm,
}

impl ProviderKind {
    pub fn credential_kind(self) -> crate::ai_verify::CredentialKind {
        match self {
            Self::Stt => crate::ai_verify::CredentialKind::Stt,
            Self::Llm => crate::ai_verify::CredentialKind::Llm,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Stt => "stt",
            Self::Llm => "llm",
        }
    }

    pub(super) fn providers(self) -> &'static [Provider] {
        match self {
            Self::Stt => STT_PROVIDERS,
            Self::Llm => LLM_PROVIDERS,
        }
    }

    fn provider_setting(self) -> &'static str {
        match self {
            Self::Stt => "current_stt_provider",
            Self::Llm => "current_llm_provider",
        }
    }

    fn model_setting(self) -> &'static str {
        match self {
            Self::Stt => "current_stt_model",
            Self::Llm => "current_llm_model",
        }
    }
}

/// The inputs of one expanded provider card.
pub(crate) struct ProviderForm {
    base_url: Entity<TextInput>,
    api_key: Entity<TextInput>,
}

/// Per-page state: the search box, the open accordion item, the card forms,
/// and the credential-store keys (`useAiProvidersState`).
pub(crate) struct AiSettings {
    search: Entity<TextInput>,
    open_provider: Option<&'static str>,
    forms: HashMap<&'static str, ProviderForm>,
    api_keys: HashMap<String, String>,
    /// The credential store's error, surfaced like the keychain alert toast.
    keychain_error: Option<String>,
    /// `providerMutation.error` per provider: a rejected key verification.
    form_errors: HashMap<String, String>,
}

impl Workspace {
    fn text_input_style(&self, masked: bool) -> TextInputStyle {
        TextInputStyle {
            text: self.theme.foreground,
            placeholder: self.theme.muted_foreground,
            selection: self.theme.selection,
            underline_when_focused: false,
            masked,
        }
    }

    /// Creates the page state on first open and loads the stored API keys.
    pub(crate) fn ensure_ai_settings(
        &mut self,
        kind: ProviderKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.ai_settings.contains_key(&kind) {
            self.ensure_ai_availability(kind, true, cx);
            if kind == ProviderKind::Stt {
                self.apply_default_stt_selection(cx);
            }
            return;
        }
        let style = self.text_input_style(false);
        let search = cx.new(|cx| TextInput::new("Search providers...", style, window, cx));
        cx.subscribe_in(
            &search,
            window,
            move |this, input, event: &TextInputEvent, window, cx| {
                match event {
                    TextInputEvent::Changed => cx.notify(),
                    // `onKeyDown: Escape -> onChange("")`
                    TextInputEvent::Escape => {
                        input.update(cx, |input, cx| input.set_text("", cx));
                        this.focus_handle.focus(window);
                    }
                    _ => {}
                }
            },
        )
        .detach();
        self.ai_settings.insert(
            kind,
            AiSettings {
                search,
                open_provider: None,
                forms: HashMap::new(),
                api_keys: HashMap::new(),
                keychain_error: None,
                form_errors: HashMap::new(),
            },
        );
        self.reload_ai_api_keys(kind, cx);
    }

    /// `loadSecureAiProviderApiKeys` for every provider with a stored row.
    fn reload_ai_api_keys(&mut self, kind: ProviderKind, cx: &mut Context<Self>) {
        let ids: Vec<String> = self
            .provider_settings
            .ai_providers(kind.key())
            .keys()
            .cloned()
            .collect();
        let task = self.store.ai_provider_api_keys(kind.key(), ids);
        cx.spawn(async move |this, cx| {
            if let Ok(results) = task.await {
                this.update(cx, |this, cx| {
                    if let Some(page) = this.ai_settings.get_mut(&kind) {
                        page.api_keys.clear();
                        page.keychain_error = None;
                        for (provider_id, result) in results {
                            match result {
                                Ok(Some(key)) => {
                                    page.api_keys.insert(provider_id, key);
                                }
                                Ok(None) => {}
                                Err(error) => page.keychain_error = Some(error),
                            }
                        }
                        // Availability and the catalogue cache are keyed by the
                        // credentials.
                        this.ensure_ai_availability(kind, false, cx);
                        if kind == ProviderKind::Llm {
                            this.ensure_llm_models(false, cx);
                        }
                        cx.notify();
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    /// The effective config of a provider: the stored row's base URL (or the
    /// registry default) and the credential-store key.
    pub(super) fn ai_provider_config(
        &self,
        kind: ProviderKind,
        provider: &Provider,
    ) -> (String, String) {
        let configs = self.provider_settings.ai_providers(kind.key());
        let stored = configs.get(provider.id);
        let base_url = stored
            .map(|config| config.base_url.trim().to_string())
            .filter(|url| !url.is_empty())
            .or_else(|| provider.base_url.map(|url| url.trim().to_string()))
            .unwrap_or_default();
        let api_key = self
            .ai_settings
            .get(&kind)
            .and_then(|page| page.api_keys.get(provider.id).cloned())
            .or_else(|| stored.map(|config| config.api_key.clone()))
            .unwrap_or_default()
            .trim()
            .to_string();
        (base_url, api_key)
    }

    /// `getProviderSelectionBlockers(...).length === 0` with the shell's
    /// account state: not signed in, not Pro.
    pub(super) fn ai_provider_config_complete(
        &self,
        kind: ProviderKind,
        provider: &Provider,
    ) -> bool {
        let (base_url, api_key) = self.ai_provider_config(kind, provider);
        provider
            .requirements
            .iter()
            .all(|requirement| match requirement {
                Requirement::Config(fields) => fields.iter().all(|field| match *field {
                    "api_key" => !api_key.is_empty(),
                    "base_url" => !base_url.is_empty(),
                    _ => true,
                }),
                Requirement::Entitlement(_) | Requirement::Auth => false,
            })
    }

    /// `getLlmProviderStatus(...).configured`: the blockers pass and, for a
    /// local server or a key-verified provider, the availability check did
    /// too. The selected local provider bypasses reachability so a stopped
    /// server surfaces as a connection error rather than a silent reselect.
    pub(super) fn ai_provider_configured(&self, kind: ProviderKind, provider: &Provider) -> bool {
        if !self.ai_provider_config_complete(kind, provider) {
            return false;
        }
        // The on-device engines (`listSupportedModels`, `isDesktopLocalSttAvailable`,
        // `checkAppleFoundationModelAvailability`) exist on Apple Silicon only.
        if matches!(
            provider.id,
            "soniqo" | "apple_speech" | "local_file" | "apple_foundation"
        ) && !(cfg!(target_os = "macos") && cfg!(target_arch = "aarch64"))
        {
            return false;
        }
        if provider.checks_availability {
            let selected = self
                .provider_settings
                .string_setting(kind.provider_setting(), &["ai", kind.provider_setting()])
                .is_some_and(|id| id == provider.id);
            return selected || self.ai_provider_available(kind, provider) == Some(true);
        }
        if super::ai_availability::requires_key_verification(provider) {
            return self.ai_provider_available(kind, provider) == Some(true);
        }
        true
    }

    fn toggle_ai_provider(
        &mut self,
        kind: ProviderKind,
        provider: &'static Provider,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (base_url, api_key) = self.ai_provider_config(kind, provider);
        let plain = self.text_input_style(false);
        let masked = self.text_input_style(true);
        let Some(page) = self.ai_settings.get_mut(&kind) else {
            return;
        };
        if page.open_provider == Some(provider.id) {
            page.open_provider = None;
            cx.notify();
            return;
        }
        page.open_provider = Some(provider.id);
        if !page.forms.contains_key(provider.id) {
            let base_url_input = cx.new(|cx| {
                let mut input = TextInput::new("", plain, window, cx);
                input.set_text(base_url, cx);
                input
            });
            let api_key_input = cx.new(|cx| {
                let mut input = TextInput::new(
                    if provider.required_fields().contains(&"api_key") {
                        "Enter your API key"
                    } else {
                        "Enter your API key (optional)"
                    },
                    masked,
                    window,
                    cx,
                );
                input.set_text(api_key, cx);
                input
            });
            // `listeners.onChange` submits the form on every change.
            for (input, field) in [(&base_url_input, "base_url"), (&api_key_input, "api_key")] {
                cx.subscribe(input, move |this, input, event: &TextInputEvent, cx| {
                    if *event == TextInputEvent::Changed {
                        let value = input.read(cx).text().to_string();
                        this.write_ai_provider_field(kind, provider, field, value, cx);
                    }
                })
                .detach();
            }
            page.forms.insert(
                provider.id,
                ProviderForm {
                    base_url: base_url_input,
                    api_key: api_key_input,
                },
            );
        }
        cx.notify();
    }

    /// `providerMutation.mutateAsync(value)` -> `setAiProvider`.
    fn write_ai_provider_field(
        &mut self,
        kind: ProviderKind,
        provider: &'static Provider,
        field: &'static str,
        value: String,
        cx: &mut Context<Self>,
    ) {
        let (base_url, api_key) = match field {
            "base_url" => (Some(value), None),
            _ => (None, Some(value)),
        };
        // `useSetAiProvider({ verifyCredentials: requiresKeyVerification })`:
        // the key is proven against the effective config before it is saved.
        let verify = super::ai_availability::requires_key_verification(provider).then(|| {
            let (stored_url, stored_key) = self.ai_provider_config(kind, provider);
            let effective_url = base_url
                .clone()
                .map(|url| url.trim().to_string())
                .filter(|url| !url.is_empty())
                .unwrap_or(stored_url);
            let effective_key = api_key
                .clone()
                .map(|key| key.trim().to_string())
                .unwrap_or(stored_key);
            (effective_url, effective_key)
        });
        let store = self.store.clone();
        let runtime = self.store.runtime().clone();
        let provider_id = provider.id;
        let credential_kind = kind.credential_kind();
        let key_for_page = api_key.clone();
        cx.spawn(async move |this, cx| {
            if let Some((url, key)) = verify {
                let verified = runtime
                    .spawn(async move {
                        crate::ai_verify::verify_provider_credentials(
                            credential_kind,
                            provider_id,
                            &url,
                            &key,
                        )
                        .await
                    })
                    .await
                    .unwrap_or_else(|_| {
                        Err(crate::ai_verify::CredentialError {
                            message:
                                "Couldn’t verify this key. Check your connection and try again."
                                    .into(),
                            retryable: true,
                        })
                    });
                if let Err(error) = verified {
                    this.update(cx, |this, cx| {
                        if let Some(page) = this.ai_settings.get_mut(&kind) {
                            page.form_errors
                                .insert(provider_id.to_string(), error.message);
                        }
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            }
            this.update(cx, |this, _| {
                if let Some(page) = this.ai_settings.get_mut(&kind) {
                    page.form_errors.remove(provider_id);
                    if let Some(key) = key_for_page.as_ref() {
                        page.api_keys.insert(provider_id.to_string(), key.clone());
                    }
                }
            })
            .ok();
            let result = store
                .set_ai_provider(kind.key(), provider_id.to_string(), base_url, api_key)
                .await
                .map_err(anyhow::Error::from)
                .and_then(|r| r);
            this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.reload_settings(cx);
                        if let Some(page) = this.ai_settings.get_mut(&kind) {
                            page.keychain_error = None;
                        }
                        // A new key or URL is a new availability / `["models", ...]`
                        // query key.
                        this.ensure_ai_availability(kind, false, cx);
                        if kind == ProviderKind::Llm {
                            this.ensure_llm_models(false, cx);
                        }
                    }
                    Err(error) => {
                        tracing::error!(%error, "failed to save provider");
                        if let Some(page) = this.ai_settings.get_mut(&kind) {
                            page.keychain_error = Some(error.to_string());
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// `STT` / `LLM`: title, model selection, provider configuration.
    pub(super) fn render_ai_settings(
        &self,
        kind: ProviderKind,
        title: Div,
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(title)
            .child(self.render_ai_model_selection(kind, cx))
            .child(self.render_ai_configure_providers(kind, window, cx))
    }

    /// `SelectProviderAndModel`: the configured providers in the left select,
    /// the chosen provider's models on the right.
    fn render_ai_model_selection(&self, kind: ProviderKind, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        // `effectiveSelection`: a provider picked without a model to show
        // (`pendingProvider`) stands in for the stored selection until a model
        // is chosen.
        let pending = (kind == ProviderKind::Stt)
            .then(|| self.pending_stt_provider.clone())
            .flatten();
        let current_provider = match &pending {
            Some(provider) => Some(provider.clone()),
            None => self
                .provider_settings
                .string_setting(kind.provider_setting(), &["ai", kind.provider_setting()]),
        };
        let current_model = match &pending {
            Some(_) => None,
            None => self
                .provider_settings
                .string_setting(kind.model_setting(), &["ai", kind.model_setting()]),
        };
        let configured: Vec<&'static Provider> = kind
            .providers()
            .iter()
            .filter(|provider| !provider.disabled && self.ai_provider_configured(kind, provider))
            .collect();
        // `getVisibleModelSelection`: an unconfigured provider shows as empty.
        let selected_provider = current_provider.as_deref().and_then(|id| {
            configured
                .iter()
                .copied()
                .find(|provider| provider.id == id)
        });
        let model_setting = kind.model_setting();

        let provider_options: Vec<SelectOption> = configured
            .iter()
            .map(|provider| SelectOption {
                value: provider.id.to_string(),
                label: provider.display_name.to_string(),
                detail: None,
                glyph: Some(provider.icon),
                badges: Vec::new(),
                lock: None,
                heading: None,
            })
            .collect();
        // The LLM page lists the provider's live catalogue (`ModelCombobox`
        // over `listModels`); STT keeps the registry's models.
        let catalogue = match (kind, selected_provider) {
            (ProviderKind::Llm, Some(provider)) => Some(self.llm_models_for(provider)),
            _ => None,
        };
        let model_options: Vec<SelectOption> = match (kind, selected_provider, catalogue) {
            (
                ProviderKind::Llm,
                Some(provider),
                Some(Some(super::llm_models::LlmModels::Loaded(result))),
            ) => result
                .models
                .iter()
                .map(|model| SelectOption {
                    value: model.clone(),
                    label: crate::ai_models::display_llm_model_id(provider.id, model),
                    detail: None,
                    glyph: None,
                    badges: Vec::new(),
                    lock: None,
                    heading: None,
                })
                .collect(),
            (ProviderKind::Llm, Some(_), _) => Vec::new(),
            // The STT rows: `displayModelLabel`, the `DeprecatedBadge` and the
            // `ModelModeBadge` for the model's transcription mode; the Anarlog
            // cloud model without a paid plan offers `Upgrade to use` instead.
            (_, Some(provider), _) if provider.id == "anarlog" => vec![SelectOption {
                value: "cloud".to_string(),
                label: crate::stt_models::display_model_id("cloud"),
                detail: None,
                glyph: crate::stt_models::model_icon("cloud"),
                badges: Vec::new(),
                lock: (!self.is_pro()).then_some(super::settings::SelectLock::UpgradeToUse),
                // `category: "latest"` → `Recommended`.
                heading: Some("Recommended"),
            }],
            (_, Some(provider), _) => provider
                .models
                .iter()
                .map(|model| {
                    let mut badges = Vec::new();
                    if crate::stt_models::is_deprecated_stt_model(provider.id, model) {
                        badges.push(super::settings::SelectBadge::Deprecated);
                    }
                    // `mode === "live" ? "realtime" : mode`; `ModelModeBadge`
                    // renders nothing for a model without a known mode.
                    match crate::stt_capabilities::model_transcription_mode(provider.id, model) {
                        Some(anlg_listener_core::TranscriptionMode::Batch) => {
                            badges.push(super::settings::SelectBadge::AfterRecording)
                        }
                        Some(anlg_listener_core::TranscriptionMode::Live) => {
                            badges.push(super::settings::SelectBadge::Live)
                        }
                        None => {}
                    }
                    SelectOption {
                        value: model.to_string(),
                        label: crate::stt_models::display_model_id(model),
                        detail: None,
                        glyph: crate::stt_models::model_icon(model),
                        badges,
                        lock: None,
                        heading: None,
                    }
                })
                .collect(),
            (_, None, _) => Vec::new(),
        };
        let combobox = match (kind, selected_provider) {
            (ProviderKind::Llm, Some(provider)) => {
                let (loading, ignored) = match catalogue {
                    Some(Some(super::llm_models::LlmModels::Loaded(result))) => (
                        false,
                        result
                            .ignored
                            .iter()
                            .map(|model| {
                                (
                                    model.id.clone(),
                                    crate::ai_models::display_llm_model_id(provider.id, &model.id),
                                    model
                                        .reasons
                                        .contains(&crate::ai_models::IgnoreReason::OldModel),
                                    model
                                        .reasons
                                        .iter()
                                        .map(|reason| reason.label().to_string())
                                        .collect(),
                                )
                            })
                            .collect(),
                    ),
                    _ => (true, Vec::new()),
                };
                let selected_deprecated = current_model.as_deref().is_some_and(|model| {
                    ignored
                        .iter()
                        .any(|(id, _, deprecated, _)| id == model && *deprecated)
                });
                let health = self.llm_health_status();
                Some(Rc::new(super::settings::ComboboxExtras {
                    loading,
                    // `isConfigured && health.status === "success"`
                    configured: matches!(health, Some(crate::ai_health::Health::Success)),
                    pending: matches!(health, Some(crate::ai_health::Health::Pending)),
                    current_label: current_model
                        .as_deref()
                        .map(|model| crate::ai_models::display_llm_model_id(provider.id, model)),
                    ignored,
                    selected_deprecated,
                    on_refresh: Rc::new(|this, cx| this.ensure_llm_models(true, cx)),
                }))
            }
            // `{isConfigured && <HealthStatusIndicator />}` / the green check:
            // the STT select carries the same suffixes from `useConnectionHealth`.
            (ProviderKind::Stt, Some(_))
                if current_model.as_deref().is_some_and(|m| !m.is_empty()) =>
            {
                let health = self.stt_health_status();
                Some(Rc::new(super::settings::ComboboxExtras {
                    loading: false,
                    configured: matches!(health, Some(super::stt_selection::SttHealth::Success)),
                    pending: matches!(health, Some(super::stt_selection::SttHealth::Pending)),
                    current_label: None,
                    ignored: Vec::new(),
                    selected_deprecated: false,
                    on_refresh: Rc::new(|_, _| {}),
                }))
            }
            _ => None,
        };

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(section_title(theme, "Model being used"))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .min_w_0()
                            // `flex-2` next to the model's `flex-3`.
                            .flex_grow()
                            .flex_shrink()
                            .flex_basis(gpui::relative(0.4))
                            .child(self.render_select(
                                SelectSpec {
                                    id: match kind {
                                        ProviderKind::Stt => "stt-provider",
                                        ProviderKind::Llm => "llm-provider",
                                    },
                                    current:
                                        selected_provider.map(|provider| provider.id.to_string()),
                                    placeholder: "Select a provider",
                                    options: Rc::new(provider_options),
                                    search: None,
                                    on_select: Rc::new(move |this, value, _, cx| {
                                        if kind == ProviderKind::Llm {
                                            this.change_llm_provider(value, cx);
                                        } else {
                                            this.change_stt_provider(value, cx);
                                        }
                                    }),
                                    combobox: None,
                                    align_end: false,
                                },
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .text_color(theme.muted_foreground)
                            .tw_text_sm()
                            .child("/"),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_grow()
                            .flex_shrink()
                            .flex_basis(gpui::relative(0.6))
                            .relative()
                            .child(self.render_select(
                                SelectSpec {
                                    id: match kind {
                                        ProviderKind::Stt => "stt-model",
                                        ProviderKind::Llm => "llm-model",
                                    },
                                    current: selected_provider.and(current_model.clone()),
                                    placeholder: "Select a model",
                                    options: Rc::new(model_options),
                                    // The LLM combobox searches; the STT select lists.
                                    search: combobox.as_ref().filter(|_| kind == ProviderKind::Llm).map(|_| {
                                        super::settings::SearchSpec {
                                            placeholder: "Search or create new",
                                            empty_message: "No models available.",
                                            width: None,
                                            placement: super::settings::PanelPlacement::Select,
                                        }
                                    }),
                                    on_select: Rc::new(move |this, value, _, cx| {
                                        if kind == ProviderKind::Stt {
                                            this.change_stt_model(value, cx);
                                        } else {
                                            this.set_setting(
                                                model_setting,
                                                serde_json::Value::String(value),
                                                cx,
                                            );
                                        }
                                    }),
                                    combobox,
                                    // `<SelectContent align="end">` on the STT model list.
                                    align_end: kind == ProviderKind::Stt,
                                },
                                cx,
                            )),
                    ),
            )
            // `supportsReasoningEffort(provider)`: the `Reasoning effort` row.
            .when(
                kind == ProviderKind::Llm
                    && selected_provider.is_some_and(|provider| {
                        crate::ai_models::supports_reasoning_effort(provider.id)
                    }),
                |column| {
                    let effort = self
                        .provider_settings
                        .string_setting(
                            "current_llm_reasoning_effort",
                            &["ai", "current_llm_reasoning_effort"],
                        )
                        .map(|value| crate::ai_models::normalize_reasoning_effort(&value))
                        .unwrap_or("default");
                    column.child(super::settings::setting_row(
                        theme,
                        "Reasoning effort",
                        Some(
                            "How much the model thinks before answering. Default leaves it to the provider.",
                        ),
                        true,
                        self.render_select(
                            SelectSpec::for_setting(
                                "current_llm_reasoning_effort",
                                Some(effort.to_string()),
                                "Default",
                                [
                                    ("default", "Default"),
                                    ("low", "Low"),
                                    ("medium", "Medium"),
                                    ("high", "High"),
                                ]
                                .into_iter()
                                .map(|(value, label)| SelectOption {
                                    value: value.to_string(),
                                    label: label.to_string(),
                                    detail: None,
                                    glyph: None,
                                    badges: Vec::new(),
                                    lock: None,
                                    heading: None,
                                })
                                .collect(),
                            ),
                            cx,
                        ),
                    ))
                },
            )
    }

    /// `ConfigureProviders`: heading + search, then the accordion cards for
    /// every non-built-in provider matching the query.
    fn render_ai_configure_providers(
        &self,
        kind: ProviderKind,
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let page = self.ai_settings.get(&kind);
        let query = page
            .map(|page| page.search.read(cx).text().trim().to_lowercase())
            .unwrap_or_default();
        // STT drops the built-in providers; LLM applies `shouldShowInProviderList`
        // (no Anarlog card, subscription twins folded until searched for).
        let providers: Vec<&'static Provider> = kind
            .providers()
            .iter()
            .filter(|provider| match kind {
                ProviderKind::Stt => !provider.built_in,
                ProviderKind::Llm => {
                    provider.id != "anarlog"
                        && (!query.is_empty() || !is_folded_subscription_provider(provider.id))
                }
            })
            .filter(|provider| {
                query.is_empty()
                    || format!(
                        "{} {} {}",
                        provider.display_name,
                        provider.id,
                        provider.description.unwrap_or("")
                    )
                    .to_lowercase()
                    .contains(&query)
            })
            .collect();

        let mut section = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(section_title(theme, "Configure Providers"))
                    .children(page.map(|page| self.render_provider_search(page, cx))),
            )
            .child(
                div().flex().flex_col().gap_3().children(
                    providers
                        .iter()
                        .map(|provider| self.render_provider_card(kind, provider, window, cx)),
                ),
            );
        // The LLM page only shows the empty state for a non-empty search.
        if providers.is_empty() && (kind == ProviderKind::Stt || !query.is_empty()) {
            section = section.child(
                div()
                    .py_8()
                    .w_full()
                    .flex()
                    .justify_center()
                    .tw_text_sm()
                    .text_color(theme.muted_foreground)
                    .child("No providers found."),
            );
        }
        section
    }

    /// `ProviderSearch`: `ml-auto h-8 w-56 rounded-lg border bg-muted/50 px-2.5
    /// gap-2` with the 14px glass, the field, and a clear button while typing.
    fn render_provider_search(&self, page: &AiSettings, cx: &Context<Self>) -> AnyElement {
        let theme = self.theme;
        let has_query = !page.search.read(cx).text().is_empty();
        let search = page.search.clone();
        div()
            .id("provider-search")
            .ml_auto()
            .flex()
            .h(px(32.0))
            .w(px(224.0))
            .max_w(gpui::relative(0.55))
            .items_center()
            .gap_2()
            .rounded_lg()
            .border_1()
            .border_color(theme.border)
            .bg(alpha(theme.muted, 0.5))
            .px(px(10.0))
            .tw_text_sm()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| {
                search.read(cx).focus_handle(cx).focus(window);
            }))
            .child(icon("search", px(14.0), theme.muted_foreground))
            .child(div().min_w_0().flex_1().child(page.search.clone()))
            .when(has_query, |row| {
                let search = page.search.clone();
                row.child(
                    div()
                        .id("provider-search-clear")
                        .flex_shrink_0()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                            search.update(cx, |input, cx| input.set_text("", cx));
                        }))
                        .child(icon("x", px(14.0), theme.muted_foreground)),
                )
            })
            .into_any_element()
    }

    /// `NonAnarlogProviderCard`: `bg-muted rounded-[22px] border-2` (solid once
    /// the provider is ready, dashed otherwise) with the `px-4 py-4 text-sm
    /// font-medium` trigger and the form in `px-4 pb-4`.
    fn render_provider_card(
        &self,
        kind: ProviderKind,
        provider: &'static Provider,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let page = self.ai_settings.get(&kind);
        let open = page.is_some_and(|page| page.open_provider == Some(provider.id));
        let ready = !provider.checks_availability && self.ai_provider_configured(kind, provider);
        let locked = provider
            .requirements
            .iter()
            .any(|requirement| matches!(requirement, Requirement::Entitlement("pro")));
        let disabled = provider.disabled || locked;

        let mut card = div()
            .id(SharedString::from(format!(
                "provider-{}-{}",
                kind.key(),
                provider.id
            )))
            .flex()
            .flex_col()
            .rounded(px(22.0))
            .border_2()
            .border_color(theme.border)
            .when(!ready, |card| card.border_dashed())
            .bg(theme.muted)
            .child(
                div()
                    .id(SharedString::from(format!(
                        "provider-trigger-{}-{}",
                        kind.key(),
                        provider.id
                    )))
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .px_4()
                    .py_4()
                    .tw_text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(if disabled {
                        theme.muted_foreground
                    } else {
                        theme.foreground
                    })
                    .when(!disabled, |trigger| trigger.cursor_pointer())
                    .when(disabled, |trigger| trigger.cursor_not_allowed())
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .when(!disabled, |trigger| {
                        trigger.on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.toggle_ai_provider(kind, provider, window, cx);
                        }))
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(provider_slot_icon(provider.icon, theme))
                            // `hover:underline` on the trigger text.
                            .child(
                                div()
                                    .id(SharedString::from(format!(
                                        "provider-name-{}-{}",
                                        kind.key(),
                                        provider.id
                                    )))
                                    .hover(|style| style.text_decoration_1())
                                    .child(SharedString::from(provider.display_name)),
                            )
                            .children(provider.badge.map(|badge| {
                                let chip = provider_badge(theme, badge);
                                if badge == "After recording" {
                                    // `Tooltip delayDuration={100}` / `side="top"` / `max-w-64`.
                                    self.tooltip_trigger(
                                        super::tooltip::TooltipSpec::text(
                                            format!("provider-badge-{}-{}", kind.key(), provider.id),
                                            "Runs after the recording finishes, not during the meeting.",
                                            super::tooltip::Side::Top,
                                        )
                                        .delay(100)
                                        .max_width(256.0),
                                        chip,
                                        cx,
                                    )
                                    .into_any_element()
                                } else {
                                    chip.into_any_element()
                                }
                            })),
                    )
                    // `[&[data-state=open]>svg]:rotate-180`
                    .child(icon(
                        if open { "caret-up" } else { "caret-down" },
                        px(16.0),
                        theme.muted_foreground,
                    )),
            );

        if open
            && let Some(page) = page
            && let Some(form) = page.forms.get(provider.id)
        {
            card = card.child(self.render_provider_form(kind, provider, form, page, window, cx));
        }
        card.into_any_element()
    }

    /// `AccordionContent`: the provider context copy, the Base URL / API key
    /// fields, the documentation links, and the Advanced disclosure.
    fn render_provider_form(
        &self,
        kind: ProviderKind,
        provider: &'static Provider,
        form: &ProviderForm,
        page: &AiSettings,
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let required = provider.required_fields();
        let show_api_key = required.contains(&"api_key") && !provider.subscription;
        let show_base_url = required.contains(&"base_url") && !provider.subscription;
        let has_advanced = (!show_base_url && provider.base_url.is_some()) || !show_api_key;
        let show_advanced = !provider.hide_advanced && has_advanced;
        let (base_url, api_key) = self.ai_provider_config(kind, provider);
        let has_stored = !api_key.is_empty()
            || (!base_url.is_empty() && base_url != provider.base_url.unwrap_or("").trim());

        let field = |label: &'static str, input: &Entity<TextInput>| {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .tw_text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(theme.foreground)
                        .child(label),
                )
                .child(
                    // `InputGroup`: `h-9 rounded-md border bg-card shadow-2xs`,
                    // the control in `px-3 text-sm`.
                    div()
                        .flex()
                        .h(px(36.0))
                        .w_full()
                        .items_center()
                        .rounded_md()
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.card)
                        .px_3()
                        .tw_text_sm()
                        .child(div().min_w_0().flex_1().child(input.clone())),
                )
        };

        let mut body = div().flex().flex_col().px_4().pb_4();
        if kind == ProviderKind::Llm {
            body = body.gap_3();
        }
        if let Some(context) = provider_context(kind, provider.id) {
            body = body.child(self.render_provider_context(context, window));
        }

        let mut form_column = div().flex().flex_col().gap_4();
        if provider.subscription {
            form_column = form_column.child(div().mb_3().flex().items_center().gap_2().child(
                if !api_key.is_empty() {
                    div()
                        .tw_text_xs()
                        .text_color(theme.muted_foreground)
                        .child("Connected with your existing subscription.")
                        .into_any_element()
                } else {
                    // `Button variant="outline" size="sm"`: `h-7 px-2 text-xs rounded-full border`.
                    div()
                        .id(SharedString::from(format!("connect-{}", provider.id)))
                        .flex()
                        .h(px(28.0))
                        .items_center()
                        .gap_2()
                        .px_2()
                        .relative()
                        .child(crate::squircle::squircle(
                            crate::squircle::CONTROL_RADIUS,
                            Some(theme.background),
                            Some((1.0, theme.border)),
                        ))
                        .tw_text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(theme.foreground)
                        .cursor_pointer()
                        .child(provider_icon(provider.icon, px(14.0), theme))
                        .child(SharedString::from(format!(
                            "Connect {}",
                            provider.display_name
                        )))
                        .into_any_element()
                },
            ));
        }
        if show_base_url {
            form_column = form_column.child(field("Base URL", &form.base_url));
        }
        if show_api_key {
            form_column = form_column.child(field("API Key", &form.api_key));
        }
        if kind == ProviderKind::Llm
            && let Some(twin) = subscription_twin_id(provider.id)
            && let Some(twin_provider) = LLM_PROVIDERS.iter().find(|candidate| candidate.id == twin)
        {
            // `subscriptionProvider && onConnectSubscription`: the "or" divider
            // and the Connect button (or the connected line with Reset).
            let (_, twin_key) = self.ai_provider_config(kind, twin_provider);
            let rule = || div().flex_1().h(px(1.0)).bg(theme.border);
            form_column = form_column.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(rule())
                            .child(
                                div()
                                    .tw_text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("or"),
                            )
                            .child(rule()),
                    )
                    .child(if !twin_key.is_empty() {
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(div().tw_text_xs().text_color(theme.muted_foreground).child(
                                SharedString::from(format!(
                                    "Connected with your {} subscription.",
                                    twin_provider.display_name
                                )),
                            ))
                            .child(
                                div()
                                    .id(SharedString::from(format!("reset-twin-{}", provider.id)))
                                    .flex()
                                    .h(px(28.0))
                                    .flex_shrink_0()
                                    .items_center()
                                    .tw_text_xs()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(theme.destructive)
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.reset_ai_provider(kind, twin_provider, cx);
                                    }))
                                    .child("Reset"),
                            )
                            .into_any_element()
                    } else {
                        div()
                            .id(SharedString::from(format!("connect-twin-{}", provider.id)))
                            .flex()
                            .h(px(28.0))
                            .w_auto()
                            .items_center()
                            .gap_2()
                            .px_2()
                            .relative()
                            .child(crate::squircle::squircle(
                                crate::squircle::CONTROL_RADIUS,
                                Some(theme.background),
                                Some((1.0, theme.border)),
                            ))
                            .tw_text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .cursor_pointer()
                            .child(provider_icon(twin_provider.icon, px(14.0), theme))
                            .child(SharedString::from(format!(
                                "Connect {}",
                                twin_provider.display_name
                            )))
                            .into_any_element()
                    }),
            );
        }
        if !provider.links.is_empty() {
            form_column =
                form_column.child(div().flex().items_center().gap_4().tw_text_xs().children(
                    provider.links.iter().map(|(link_kind, label, url)| {
                        let url = url.to_string();
                        div()
                            .id(SharedString::from(format!(
                                "link-{}-{}",
                                provider.id, link_kind
                            )))
                            .flex()
                            .items_center()
                            .gap(px(2.0))
                            .text_color(theme.muted_foreground)
                            .cursor_pointer()
                            .hover(move |style| style.text_color(theme.foreground))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(move |_, _, _cx| crate::opener::open_url(&url))
                            .child(SharedString::from(*label))
                            .child(icon("external-link", px(12.0), theme.muted_foreground))
                    }),
                ));
        }
        let reset = has_stored.then(|| {
            // `Button variant="ghost" size="sm"`: `h-7 px-0 text-xs text-destructive`.
            div()
                .id(SharedString::from(format!("reset-{}", provider.id)))
                .flex()
                .h(px(28.0))
                .items_center()
                .tw_text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.destructive)
                .cursor_pointer()
                .hover(move |style| style.text_color(alpha(theme.destructive, 0.8)))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.reset_ai_provider(kind, provider, cx);
                }))
                .child("Reset")
        });
        if show_advanced {
            let expanded = self.ai_advanced_open.contains(&(kind, provider.id));
            let mut advanced = div().flex().flex_col().child(
                div()
                    .id(SharedString::from(format!("advanced-{}", provider.id)))
                    .flex()
                    .items_center()
                    .gap_1()
                    .tw_text_xs()
                    .text_color(theme.muted_foreground)
                    .cursor_pointer()
                    .hover(move |style| style.text_color(theme.foreground))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if !this.ai_advanced_open.remove(&(kind, provider.id)) {
                            this.ai_advanced_open.insert((kind, provider.id));
                        }
                        cx.notify();
                    }))
                    // `<summary>` draws the disclosure triangle before the label.
                    .child(icon(
                        if expanded {
                            "caret-down"
                        } else {
                            "caret-right"
                        },
                        px(12.0),
                        theme.muted_foreground,
                    ))
                    .child("Advanced"),
            );
            if expanded {
                let mut inner = div().mt_2().flex().flex_col().gap_4();
                if !show_base_url && provider.base_url.is_some() {
                    inner = inner.child(field("Base URL", &form.base_url));
                }
                if !show_api_key {
                    inner = inner.child(field("API Key", &form.api_key));
                }
                inner = inner.children(reset);
                advanced = advanced.child(inner);
            }
            form_column = form_column.child(advanced);
        } else {
            form_column = form_column.children(reset);
        }
        if let Some(error) = &page.keychain_error {
            form_column = form_column.child(
                div()
                    .tw_text_xs()
                    .text_color(theme.destructive)
                    .child(SharedString::from(error.clone())),
            );
        }
        // `providerMutation.error.message`
        if let Some(error) = page.form_errors.get(provider.id) {
            form_column = form_column.child(
                div()
                    .tw_text_xs()
                    .text_color(theme.destructive)
                    .child(SharedString::from(error.clone())),
            );
        }
        body.child(form_column)
    }

    /// `StyledStreamdown className="mb-2"`: the provider's Markdown blurb at
    /// `mt-1 text-sm` (paragraphs `mb-1`, links `font-medium underline`)
    /// through the shared Markdown → TipTap converter.
    fn render_provider_context(&self, markdown: &str, window: &Window) -> AnyElement {
        let theme = self.theme;
        let blocks = crate::document::from_body("markdown", markdown);
        let renderer = self.document_renderer(window);
        div()
            .mt_1()
            .mb_2()
            .flex()
            .flex_col()
            .children(blocks.iter().enumerate().filter_map(|(index, block)| {
                // The last block's `mb-1` collapses into the wrapper's `mb-2`.
                let last = index + 1 == blocks.len();
                match block {
                    crate::document::Block::Paragraph(spans) => Some(
                        div()
                            .when(!last, |block| block.mb_1())
                            .tw_text_sm()
                            .child(renderer.inline_text(
                                spans,
                                px(14.0),
                                px(20.0),
                                theme.foreground,
                            ))
                            .into_any_element(),
                    ),
                    // `ul { list-disc pl-6 mb-1 }`, `li { mb-1 }`
                    crate::document::Block::List { items, .. } => Some(
                        div()
                            .when(!last, |list| list.mb_1())
                            .pl_6()
                            .flex()
                            .flex_col()
                            .children(items.iter().enumerate().map(|(item_index, item)| {
                                let last_item = last && item_index + 1 == items.len();
                                let spans: Vec<crate::document::Span> = item
                                    .blocks
                                    .iter()
                                    .filter_map(|block| match block {
                                        crate::document::Block::Paragraph(spans) => {
                                            Some(spans.clone())
                                        }
                                        _ => None,
                                    })
                                    .flatten()
                                    .collect();
                                div()
                                    .relative()
                                    .when(!last_item, |item| item.mb_1())
                                    .tw_text_sm()
                                    .child(
                                        div()
                                            .absolute()
                                            .left(px(-14.0))
                                            .top_0()
                                            .text_color(theme.foreground)
                                            .child("\u{2022}"),
                                    )
                                    .child(renderer.inline_text(
                                        &spans,
                                        px(14.0),
                                        px(20.0),
                                        theme.foreground,
                                    ))
                            }))
                            .into_any_element(),
                    ),
                    _ => None,
                }
            }))
            .into_any_element()
    }

    /// `useClearAiProvider`: drop the row and the credential-store key.
    fn reset_ai_provider(
        &mut self,
        kind: ProviderKind,
        provider: &'static Provider,
        cx: &mut Context<Self>,
    ) {
        if let Some(page) = self.ai_settings.get_mut(&kind) {
            page.api_keys.remove(provider.id);
            if let Some(form) = page.forms.get(provider.id) {
                let base = provider.base_url.unwrap_or("").to_string();
                form.base_url
                    .update(cx, |input, cx| input.set_text(base, cx));
                form.api_key.update(cx, |input, cx| input.set_text("", cx));
            }
        }
        let task = self
            .store
            .clear_ai_provider(kind.key(), provider.id.to_string());
        cx.spawn(async move |this, cx| {
            match task.await.map_err(anyhow::Error::from).and_then(|r| r) {
                Ok(()) => {
                    this.update(cx, |this, cx| this.reload_settings(cx)).ok();
                }
                Err(error) => tracing::error!(%error, "failed to clear provider"),
            }
        })
        .detach();
    }
}

/// `API_SUBSCRIPTION_TWINS`
fn subscription_twin_id(provider_id: &str) -> Option<&'static str> {
    match provider_id {
        "openai" => Some("chatgpt"),
        "anthropic" => Some("claude"),
        "xai" => Some("grok"),
        "moonshot" => Some("kimi_code"),
        _ => None,
    }
}

/// `isFoldedSubscriptionProvider`
fn is_folded_subscription_provider(provider_id: &str) -> bool {
    matches!(provider_id, "chatgpt" | "claude" | "grok" | "kimi_code")
}

/// `<h3 className="text-md font-sans font-semibold">` (`text-md` is not a
/// Tailwind size, so the heading inherits 16px/24px).
fn section_title(theme: crate::theme::Theme, label: &'static str) -> Div {
    div()
        .tw_text_base()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(theme.foreground)
        .child(label)
}

/// `ProviderBadge`: `After recording` is a `bg-background/40 rounded-md
/// px-1.5 py-0.5 text-[11px] font-medium` chip, everything else a
/// `rounded-full border px-2 text-xs font-light` pill.
fn provider_badge(theme: crate::theme::Theme, badge: &'static str) -> Div {
    let after_recording = badge == "After recording";
    div()
        .text_color(theme.muted_foreground)
        .when(after_recording, |chip| {
            chip.rounded_md()
                .px(px(6.0))
                .py(px(2.0))
                .bg(alpha(theme.background, 0.4))
                .text_size(px(11.0))
                .line_height(px(16.0))
                .font_weight(gpui::FontWeight::MEDIUM)
        })
        .when(!after_recording, |pill| {
            // `.rounded-full` is `0.5rem` in the desktop app.
            pill.rounded(px(8.0))
                .border_1()
                .border_color(theme.border)
                .px_2()
                .tw_text_xs()
                .font_weight(gpui::FontWeight::LIGHT)
        })
        .child(badge)
}

/// `ProviderIconSlot` / `ProviderButtonIcon`: the brand mark in a square slot.
/// `ProviderIconSlot`: the `size-5` slot whose art is `size-3`, except the
/// Soniox mark scaled by `--soniox-icon-scale` back to the full slot.
pub(crate) fn provider_slot_icon(glyph: Icon, theme: crate::theme::Theme) -> AnyElement {
    let soniox = matches!(glyph, Icon::Image(path) if path.contains("soniox"));
    let art = if soniox || matches!(glyph, Icon::Model(_)) {
        px(20.0)
    } else {
        px(12.0)
    };
    div()
        .size(px(20.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .child(provider_icon(glyph, art, theme))
        .into_any_element()
}

pub(crate) fn provider_icon(
    glyph: Icon,
    size: gpui::Pixels,
    theme: crate::theme::Theme,
) -> AnyElement {
    match glyph {
        Icon::Image(path) => img(ImageSource::Resource(Resource::Embedded(path.into())))
            .size(size)
            .flex_shrink_0()
            .into_any_element(),
        // `object-cover object-left` on the 164×30 NVIDIA wordmark shows its
        // eye: the image scaled to the slot's height, its left edge kept.
        Icon::Model(path) if path.contains("nvidia") => div()
            .size(size)
            .flex_shrink_0()
            .overflow_hidden()
            .child(
                img(ImageSource::Resource(Resource::Embedded(path.into())))
                    .h(size)
                    .w(size * (164.0 / 30.0))
                    .flex_shrink_0(),
            )
            .into_any_element(),
        Icon::Model(path) => img(ImageSource::Resource(Resource::Embedded(path.into())))
            .size(size)
            .object_fit(gpui::ObjectFit::Contain)
            .flex_shrink_0()
            .into_any_element(),
        Icon::Mono(path, color) => {
            let color = color.map(gpui::rgb).unwrap_or(theme.foreground);
            gpui::svg()
                .path(SharedString::from(path))
                .size(size)
                .flex_shrink_0()
                .text_color(color)
                .into_any_element()
        }
        Icon::Glyph(name) => icon(name, size, theme.foreground).into_any_element(),
    }
}

/// `ProviderContext` copy for the STT page and `llm/configure.tsx` for the
/// LLM page.
fn provider_context(kind: ProviderKind, provider_id: &str) -> Option<&'static str> {
    let text = match (kind, provider_id) {
        (ProviderKind::Stt, "anarlog") => {
            "**Anarlog Cloud** routes request to the **best available model** for highest accuracy and performance."
        }
        (ProviderKind::Stt, "deepgram") => {
            "Use [Deepgram](https://deepgram.com) for transcriptions. If you want to use a [Dedicated](https://developers.deepgram.com/reference/custom-endpoints#deepgram-dedicated-endpoints) or [EU](https://developers.deepgram.com/reference/custom-endpoints#eu-endpoints) endpoint, or a Deepgram-compatible server on this computer or your local network, you can do that in the **advanced** section."
        }
        (ProviderKind::Stt, "soniox") => "Use [Soniox](https://soniox.com) for transcriptions.",
        (ProviderKind::Stt, "assemblyai") => {
            "Use [AssemblyAI](https://www.assemblyai.com) for transcriptions."
        }
        (ProviderKind::Stt, "gladia") => "Use [Gladia](https://www.gladia.io) for transcriptions.",
        (ProviderKind::Stt, "openai") => "Use [OpenAI](https://openai.com) for transcriptions.",
        (ProviderKind::Stt, "openrouter") => {
            "Use [OpenRouter](https://openrouter.ai) to transcribe with supported speech-to-text models through one API key. OpenRouter transcription runs after recording."
        }
        (ProviderKind::Stt, "dashscope") => {
            "Use Alibaba Cloud Model Studio's Qwen ASR for **live transcription**. The default endpoint is the Singapore region; change it under Advanced when your API key belongs to another region."
        }
        (ProviderKind::Stt, "zai") => {
            "Use [Z.AI GLM ASR](https://docs.z.ai/guides/audio/glm-asr-2512) for batch transcription. Anarlog automatically splits recordings to fit Z.AI's 30-second upload limit."
        }
        (ProviderKind::Stt, "siliconflow") => {
            "Use [SiliconFlow](https://docs.siliconflow.com/en/api-reference/audio/create-audio-transcriptions) for batch transcription. The default endpoint is the international service; use `https://api.siliconflow.cn/v1` under Advanced for a China-region API key."
        }
        (ProviderKind::Stt, "cloudflare_workers_ai") => {
            "Use a [Cloudflare Workers AI](https://developers.cloudflare.com/workers-ai/) endpoint that exposes Deepgram-compatible Nova-3 transcription."
        }
        (ProviderKind::Stt, "mistral") => {
            "Use [Mistral](https://mistral.ai) for transcriptions. Keep the Base URL as `https://api.mistral.ai/v1` (Reset under Advanced if you pasted a transcriptions endpoint). **Voxtral Mini Transcribe 2** transcribes after recording; the realtime model is for live captions."
        }
        (ProviderKind::Stt, "cohere") => {
            "Use [Cohere Transcribe](https://docs.cohere.com/docs/transcribe) for batch transcription. Files must be 25 MB or smaller and use one selected language. Cohere does not return timestamps or speaker labels, so Anarlog estimates word timing."
        }
        (ProviderKind::Stt, "google_generative_ai") => {
            "Use [Gemini 3.5 Transcribe](https://blog.google/innovation-and-ai/models-and-research/gemini-models/gemini-3-5-transcribe/) with a Google AI Studio API key. **3.5 Transcribe Live** captions during recording (preview sessions last up to 10 minutes). **3.5 Transcribe** runs after recording with speaker labels and word timestamps; Anarlog splits files past 15 minutes."
        }
        (ProviderKind::Stt, "google_cloud") => {
            "Use [Google Cloud Speech-to-Text](https://cloud.google.com/speech-to-text) to transcribe after recording. Long recordings are split automatically and combined into one transcript. Paste an OAuth access token in the API key field; refresh it when it expires."
        }
        (ProviderKind::Stt, "azure_speech") => {
            "Use [Azure AI Speech](https://learn.microsoft.com/azure/ai-services/speech-service/rest-speech-to-text) fast transcription. Enter the regional Speech resource endpoint as the Base URL and its subscription key as the API key."
        }
        (ProviderKind::Stt, "aws_transcribe") => {
            "Amazon Transcribe's native file API requires SigV4 plus an S3 object. Enter an OpenAI-compatible gateway URL that performs that AWS authentication and upload, then paste the gateway token as the API key."
        }
        (ProviderKind::Stt, "speechmatics") => {
            "Use [Speechmatics](https://docs.speechmatics.com/speech-to-text/batch/quickstart) enhanced batch transcription. The default endpoint uses the EU region and can be changed under Advanced."
        }
        (ProviderKind::Stt, "revai") => {
            "Use [Rev AI](https://docs.rev.ai/api/asynchronous/get-started) asynchronous transcription. Anarlog uploads the recording, waits for the job, and retrieves word timestamps and speaker labels."
        }
        (ProviderKind::Stt, "custom") => {
            "Point this at any **Deepgram-compatible** endpoint, including a server on this computer or your local network."
        }
        (ProviderKind::Llm, "claude") => {
            "Uses your **Claude Pro or Max** plan. Sign in through Connect â no Anthropic API key needed."
        }
        (ProviderKind::Llm, "chatgpt") => {
            "Uses your **ChatGPT Plus or Pro** plan. Sign in through Connect â we'll finish the handshake automatically."
        }
        (ProviderKind::Llm, "grok") => {
            "Uses your **SuperGrok or X Premium+** plan through xAI's subscription login."
        }
        (ProviderKind::Llm, "github_copilot") => {
            "Uses your **GitHub Copilot** plan. Approve the device code in the browser to connect."
        }
        (ProviderKind::Llm, "kimi_code") => {
            "Uses your **Kimi Code** membership. Paste the coding API key from the Kimi Code console."
        }
        (ProviderKind::Llm, "apple_foundation") => {
            "- Uses Apple's on-device **System Language Model**.\n- Requires macOS 26 or later, a Mac that supports Apple Intelligence, and Apple Intelligence turned on.\n- This experiment is text-only and works best with shorter transcripts."
        }
        (ProviderKind::Llm, "lmstudio") => {
            "- Ensure LM Studio server is **running.** (Default port is 1234)\n- Enable **CORS** in LM Studio config."
        }
        (ProviderKind::Llm, "ollama") => {
            "- Ensure Ollama is **running** (`ollama serve`)\n- Pull a model first (`ollama pull llama3.2`)"
        }
        (ProviderKind::Llm, "unsloth") => {
            "- Ensure the Unsloth server is **running.** (Default port is 8888)\n- Paste the API key from Unsloth. It starts with `sk-unsloth-`.\n- Only models **loaded** in Unsloth show up in the list."
        }
        (ProviderKind::Llm, "custom") => "We only support **OpenAI-compatible** endpoints for now.",
        (ProviderKind::Llm, "openrouter") => {
            "We filter out models from the combobox based on heuristics like **input modalities** and **tool support**."
        }
        (ProviderKind::Llm, "openai") => {
            "Paste an **API key**, or connect your **ChatGPT Plus or Pro** plan."
        }
        (ProviderKind::Llm, "anthropic") => {
            "Paste an **API key**, or connect your **Claude Pro or Max** plan."
        }
        (ProviderKind::Llm, "xai") => {
            "Paste an **API key**, or connect your **SuperGrok or X Premium+** plan."
        }
        (ProviderKind::Llm, "moonshot") => {
            "Paste a Moonshot **API key**, or connect your **Kimi Code** membership. The default endpoint is the international service and can be changed under Advanced."
        }
        (ProviderKind::Llm, "zai") => {
            "Uses Z.AI's **OpenAI-compatible GLM API**. The default endpoint is the international service and can be changed under Advanced."
        }
        (ProviderKind::Llm, "alibaba_cloud") => {
            "Uses Alibaba Cloud Model Studio's **OpenAI-compatible API**. The default endpoint is the Singapore region; change the Base URL under Advanced when your API key belongs to another region."
        }
        (ProviderKind::Llm, "siliconflow") => {
            "Uses SiliconFlow's **OpenAI-compatible API**. The default endpoint is the international service; use `https://api.siliconflowcn/v1` under Advanced for a China-region API key."
        }
        (ProviderKind::Llm, "azure_openai") => {
            "Enter your **Azure OpenAI endpoint** (e.g. `https://your-resource.openai.azure.com`) as the Base URL and your **API key**. [Report issues](https://anarlog.so/discord)"
        }
        (ProviderKind::Llm, "azure_ai") => {
            "Enter your **Azure AI Foundry endpoint** as the Base URL and your **API key**. Supports Claude and other models deployed via Azure AI Foundry. [Report issues](https://anarlog.so/discord)"
        }
        (ProviderKind::Llm, "amazon_bedrock") => {
            "Enter the regional **Bedrock Mantle OpenAI-compatible URL** (for example, `https://bedrock-mantle.us-east-1.api.aws/v1`) and a Bedrock long-term API key."
        }
        _ => return None,
    };
    Some(text)
}
