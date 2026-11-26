use std::collections::BTreeMap;

use anlg_db_execute::TransactionStatement;
use desktop_runtime::{Result, ServiceError};
use gpui::{
    Context, Div, Entity, FontWeight, Render, SharedString, Stateful, Subscription, Window, div,
    prelude::*, px,
};
use serde_json::{Value, json};

use crate::{
    meeting::config::{ProviderKind, ProviderServices},
    ui::{
        input::{InputEvent, TextInput},
        theme::{Theme, theme},
    },
};

use super::{
    controls,
    provider_catalog::{self, ProviderInfo},
};
use crate::ui::a11y::Accessible;

impl gpui::Global for ProviderServices {}

/// Shipping's `BillingProvider` runs `configurePaidSettings` each time the account turns paid,
/// including when the plan loads at launch: a missing transcription provider becomes Anarlog's
/// hosted model and a missing or unusable language model becomes Anarlog's "Auto". The
/// choices are saved, so meetings, summaries, chat and the provider notices all use them.
pub fn configure_when_paid(services: ProviderServices, cx: &mut gpui::App) -> Subscription {
    // Unpaid until the plan loads, like shipping's pending claims.
    let mut paid = false;
    let mut check = move |cx: &mut gpui::App| {
        let now = super::account::is_paid(cx);
        if now && !paid {
            let services = services.clone();
            // The API key lookup needs Tokio, so the whole check runs as a runtime job.
            match services
                .runtime
                .clone()
                .service(move |_| async move { configure_paid(&services).await })
            {
                Ok(reply) => cx
                    .spawn(async move |_| {
                        if let Err(error) = reply.receive().await {
                            tracing::warn!(%error, "paid AI defaults were not saved");
                        }
                    })
                    .detach(),
                Err(error) => tracing::warn!(%error, "paid AI defaults were not saved"),
            }
        }
        paid = now;
    };
    check(cx);
    cx.observe_global::<super::account::Account>(check)
}

/// The settings `configure_paid` may write.
const PAID_KEYS: [&str; 4] = [
    "current_stt_provider",
    "current_stt_model",
    "current_llm_provider",
    "current_llm_model",
];

async fn configure_paid(services: &ProviderServices) -> Result<()> {
    let statements = paid_defaults(services).await?;
    if statements.is_empty() {
        return Ok(());
    }
    services
        .runtime
        .submit(move |services| async move {
            services
                .executor
                .execute_transaction(statements)
                .await
                .map_err(failure)?;
            Ok(())
        })?
        .receive()
        .await
}

/// The writes that give a paid account Anarlog's hosted models where it has no usable choice.
/// Each applies only while its setting still holds the value read here, so a choice saved in
/// between wins and the whole fallback rolls back.
async fn paid_defaults(services: &ProviderServices) -> Result<Vec<TransactionStatement>> {
    // Read before deciding: a save landing after this read fails the comparison below.
    let stored = services
        .runtime
        .submit(|services| async move {
            let rows = services
                .executor
                .execute(
                    "SELECT id, value_json FROM app_settings WHERE id IN (?,?,?,?)".into(),
                    PAID_KEYS.iter().map(|key| json!(key)).collect(),
                )
                .await
                .map_err(failure)?;
            Ok(rows
                .iter()
                .filter_map(|row| {
                    Some((
                        row["id"].as_str()?.to_owned(),
                        row["value_json"].as_str()?.to_owned(),
                    ))
                })
                .collect::<BTreeMap<_, _>>())
        })?
        .receive()
        .await?;
    let preferences = crate::meeting::config::preferences(&services.runtime).await?;
    let mut values = Vec::new();
    if preferences.text("current_stt_provider").is_empty() {
        values.extend([
            ("current_stt_provider", "anarlog"),
            ("current_stt_model", "cloud"),
        ]);
    }
    if uses_hosted_llm(services, &preferences).await {
        values.extend([
            ("current_llm_provider", "anarlog"),
            ("current_llm_model", "Auto"),
        ]);
    }
    Ok(values
        .into_iter()
        .map(|(key, value)| {
            compare_and_set(key, &json!(value), stored.get(key).map(String::as_str))
        })
        .collect())
}

/// Writes `key` only while it still holds `original` (`None`: absent), else affects no row,
/// which fails the transaction's expected row count.
fn compare_and_set(key: &str, value: &Value, original: Option<&str>) -> TransactionStatement {
    TransactionStatement {
        sql: "INSERT INTO app_settings (id,value_json,updated_at) SELECT ?1,?2,strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE (SELECT value_json FROM app_settings WHERE id=?1) IS ?3 ON CONFLICT(id) DO UPDATE SET value_json=excluded.value_json,updated_at=excluded.updated_at".into(),
        params: vec![json!(key), json!(value.to_string()), json!(original)],
        expected_rows_affected: Some(1),
    }
}

/// Shipping's `shouldUseHostedLlm`: the saved language model is missing, unknown, or its
/// provider lacks a required base URL or API key.
async fn uses_hosted_llm(
    services: &ProviderServices,
    preferences: &crate::meeting::config::Preferences,
) -> bool {
    let provider = preferences.text("current_llm_provider");
    if provider.is_empty() || preferences.text("current_llm_model").is_empty() {
        return true;
    }
    // The native app's on-device language model, which shipping's list lacks.
    if provider == "local" {
        return false;
    }
    let Some(info) = provider_catalog::LLM
        .iter()
        .find(|info| info.id == provider)
    else {
        return true;
    };
    let blocked = |base: &str, key: &str| {
        info.needs_base_url && base.trim().is_empty() || info.needs_api_key && key.trim().is_empty()
    };
    if !blocked(info.base_url, "") {
        return false;
    }
    let config = preferences.provider(ProviderKind::Llm, &provider);
    let base = config["base_url"]
        .as_str()
        .filter(|base| !base.is_empty())
        .unwrap_or(info.base_url)
        .to_owned();
    match (services.secret)(ProviderKind::Llm, provider).await {
        Ok(key) => blocked(
            &base,
            &key.unwrap_or_else(|| config["api_key"].as_str().unwrap_or_default().into()),
        ),
        Err(_) => true,
    }
}

#[derive(Clone, Default)]
struct Snapshot {
    values: BTreeMap<String, String>,
    parsed: BTreeMap<String, Value>,
}

fn failure(message: impl ToString) -> ServiceError {
    ServiceError::Failed(message.to_string().into())
}

impl Snapshot {
    fn choice(&self, key: &str) -> String {
        self.parsed
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    }

    fn statements(
        &self,
        kind: ProviderKind,
        provider: &str,
        model: &str,
        base: &str,
    ) -> Result<Vec<TransactionStatement>> {
        if provider.is_empty()
            || provider.len() > 128
            || !provider
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            // Shipping saves a transcription provider before its model is chosen.
            || (kind == ProviderKind::Llm && model.trim().is_empty())
            || model.len() > 512
        {
            return Err(failure("Enter a provider ID and model name."));
        }
        if matches!(provider, "local" | "local_file") {
            return Err(failure("Start local models using Manage local models."));
        }
        let key = format!("ai_provider:{}:{provider}", kind.key());
        let config = self.provider_config(kind, &key, base)?;
        let mut values = vec![
            (format!("current_{}_provider", kind.key()), json!(provider)),
            (format!("current_{}_model", kind.key()), json!(model.trim())),
            (key, config),
        ];
        // Effort support differs per model, so shipping resets it with every choice.
        if kind == ProviderKind::Llm {
            values.push(("current_llm_reasoning_effort".into(), json!("default")));
        }
        Ok(values
            .into_iter()
            .map(|(key, value)| self.upsert(key, value))
            .collect())
    }

    /// Saves one provider's row (base URL) without changing the active provider.
    fn provider_statement(
        &self,
        kind: ProviderKind,
        provider: &str,
        base: &str,
    ) -> Result<TransactionStatement> {
        let key = format!("ai_provider:{}:{provider}", kind.key());
        let config = self.provider_config(kind, &key, base)?;
        Ok(self.upsert(key, config))
    }

    /// The stored row with `base_url` replaced, in shipping's `{type, base_url, api_key}`
    /// shape; keys live in the keyring, so `api_key` stays empty. Unknown fields are kept.
    fn provider_config(&self, kind: ProviderKind, key: &str, base: &str) -> Result<Value> {
        if !base.is_empty() {
            let url = reqwest::Url::parse(base).map_err(|_| failure("Invalid provider URL"))?;
            let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
            if (url.scheme() != "https" && !(local && url.scheme() == "http"))
                || !url.username().is_empty()
                || url.password().is_some()
            {
                return Err(failure(
                    "Use HTTPS, or HTTP on localhost, without embedded credentials.",
                ));
            }
        }
        let mut config: Value = self
            .values
            .get(key)
            .map(|value| serde_json::from_str(value))
            .transpose()
            .map_err(failure)?
            .unwrap_or_else(|| json!({}));
        let object = config.as_object_mut().ok_or_else(|| {
            failure("Stored provider configuration is malformed; it was preserved.")
        })?;
        object.insert("base_url".into(), json!(base));
        object.entry("type").or_insert(json!(kind.key()));
        object.entry("api_key").or_insert(json!(""));
        Ok(config)
    }

    fn upsert(&self, key: String, value: Value) -> TransactionStatement {
        compare_and_set(&key, &value, self.values.get(&key).map(String::as_str))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Operation {
    Loading,
    Saving,
}

/// `ProviderLobeIcon` / `ProviderBrandImage`: logos without their own colors take the text
/// color, like shipping's `currentColor` icons and brand filter.
pub(super) fn provider_logo(info: &ProviderInfo, size: f32, colors: &Theme) -> gpui::AnyElement {
    let tinted = |path: SharedString| {
        gpui::svg()
            .path(path)
            .size(px(size))
            .flex_shrink_0()
            .text_color(colors.foreground)
            .into_any_element()
    };
    let image = |path: SharedString| {
        gpui::img(path)
            .size(px(size))
            .flex_shrink_0()
            .into_any_element()
    };
    let lobe = SharedString::from(format!("product/providers/{}.svg", info.id));
    if let Some(bytes) = super::assets::load(&lobe) {
        let mono = bytes.windows(12).any(|window| window == b"currentColor");
        return if mono { tinted(lobe) } else { image(lobe) };
    }
    if let Some((id, path, _)) = super::assets::BRAND_IMAGES
        .iter()
        .find(|(id, _, _)| *id == info.id)
    {
        let path = SharedString::from(*path);
        let keeps_color = matches!(*id, "alebex" | "meta");
        return if path.ends_with(".svg") && !keeps_color {
            tinted(path)
        } else {
            image(path)
        };
    }
    if info.id == "soniqo" {
        return tinted("product/AudioWaveformIcon.svg".into());
    }
    div()
        .size(px(size + 4.))
        .flex()
        .items_center()
        .justify_center()
        .bg(colors.border)
        .text_size(px(10.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(info.name.chars().next().unwrap_or_default().to_string())
        .into_any_element()
}

fn catalog(kind: ProviderKind) -> &'static [ProviderInfo] {
    match kind {
        ProviderKind::Stt => provider_catalog::STT,
        ProviderKind::Llm => provider_catalog::LLM,
    }
}

/// Subscription twins of API providers stay out of the list unless searched for
/// (`settings/ai/llm/subscriptions/twins.ts`).
fn folded(kind: ProviderKind, id: &str) -> bool {
    kind == ProviderKind::Llm && matches!(id, "chatgpt" | "claude" | "grok" | "kimi_code")
}

fn is_local_stt_provider(kind: ProviderKind, provider: &str) -> bool {
    kind == ProviderKind::Stt && matches!(provider, "soniqo" | "apple_speech")
}

/// What a save submitted, so only those edits become clean.
#[derive(Clone, Copy)]
enum Saved {
    Choice,
    Card,
    Effort,
}

/// `settings/ai/{stt,llm}`: "Model being used" plus the provider cards. Choices save as one
/// compare-and-swap transaction; API keys go to the system keyring.
pub struct ProviderView {
    services: ProviderServices,
    kind: ProviderKind,
    snapshot: Option<Snapshot>,
    on_device: Option<crate::meeting::config::OnDeviceStatus>,
    on_device_loading: bool,
    /// A provider picked in the menu until its choice is saved, or until a language model
    /// is entered for it. It is not an unsaved edit: leaving drops it, as in shipping.
    draft: Option<&'static str>,
    /// Each provider's last model this session, per kind (shipping's `lastSelectedModelsRef`,
    /// which each settings page keeps for itself).
    remembered: std::collections::HashMap<(ProviderKind, String), String>,
    /// A choice to save once the load or save in flight finishes.
    queued: bool,
    model: Entity<TextInput>,
    search: Entity<TextInput>,
    menu: bool,
    model_menu: bool,
    effort_menu: bool,
    expanded: Option<&'static str>,
    base: Entity<TextInput>,
    secret: Entity<TextInput>,
    /// Unsaved edits to the model choice and to the expanded card, which save separately.
    dirty_choice: bool,
    dirty_card: bool,
    generation: u64,
    operation: Option<Operation>,
    status: String,
    _subscriptions: Vec<Subscription>,
}

impl ProviderView {
    /// Shows `kind`'s page, starting from the saved choice unless edits are unsaved.
    pub fn select_kind(&mut self, kind: ProviderKind, cx: &mut Context<Self>) {
        if self.has_unsaved(cx) {
            return;
        }
        if self.kind != kind {
            self.kind = kind;
            self.expanded = None;
        }
        self.menu = false;
        self.model_menu = false;
        self.effort_menu = false;
        self.refresh_on_device(cx);
        self.restore(cx);
    }

    pub fn new(services: ProviderServices, cx: &mut Context<Self>) -> Self {
        let model = cx.new(|cx| TextInput::new("Select a model", cx).inline());
        let search = cx.new(|cx| TextInput::new("Search providers...", cx).inline());
        let base = cx.new(|cx| TextInput::new("Base URL", cx));
        let secret = cx.new(|cx| TextInput::new("Enter your API key", cx).secret());
        let mut subscriptions: Vec<Subscription> = [&model, &base, &secret]
            .into_iter()
            .map(|input| {
                cx.subscribe(input, |this, input, event, cx| match event {
                    InputEvent::Changed => {
                        if input == this.model {
                            this.dirty_choice = true;
                        } else {
                            this.dirty_card = true;
                        }
                        this.generation = this.generation.wrapping_add(1);
                        cx.notify();
                    }
                    InputEvent::Submitted if input == this.model => this.save_choice(cx),
                    InputEvent::Submitted => this.save_provider(cx),
                    InputEvent::Rejected => {}
                })
            })
            .collect();
        subscriptions.push(cx.subscribe(&search, |_, _, _: &InputEvent, cx| cx.notify()));
        let mut view = Self {
            services,
            kind: ProviderKind::Stt,
            snapshot: None,
            on_device: None,
            on_device_loading: false,
            draft: None,
            remembered: Default::default(),
            queued: false,
            model,
            search,
            menu: false,
            model_menu: false,
            effort_menu: false,
            expanded: None,
            base,
            secret,
            dirty_choice: false,
            dirty_card: false,
            generation: 0,
            operation: None,
            status: String::new(),
            _subscriptions: subscriptions,
        };
        view.reload(true, cx);
        view
    }

    pub fn has_unsaved(&self, cx: &gpui::App) -> bool {
        self.dirty_choice
            || self.dirty_card
            || self.queued
            || self.operation == Some(Operation::Saving)
            || [&self.model, &self.base, &self.secret]
                .into_iter()
                .any(|input| input.read(cx).buffer.marked.is_some())
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if !self.has_unsaved(cx) {
            self.reload(true, cx);
        } else {
            self.refresh_on_device(cx);
        }
    }

    fn refresh_on_device(&mut self, cx: &mut Context<Self>) {
        if self.on_device_loading {
            return;
        }
        self.on_device_loading = true;
        let on_device = (self.services.on_device)();
        cx.spawn(async move |this, cx| {
            let status = on_device.await;
            let _ = this.update(cx, |this, cx| {
                this.on_device = Some(status);
                this.on_device_loading = false;
                cx.notify();
            });
        })
        .detach();
    }

    fn reload(&mut self, restore: bool, cx: &mut Context<Self>) {
        if self.operation.is_some() {
            return;
        }
        self.operation = Some(Operation::Loading);
        self.on_device_loading = true;
        let generation = self.generation;
        let reply = self.services.runtime.read(desktop_runtime::CancellationToken::new(), |services| async move {
            let rows = services.executor.execute("SELECT id,value_json FROM app_settings WHERE (id LIKE 'ai_provider:%' OR id IN ('current_stt_provider','current_stt_model','current_llm_provider','current_llm_model','current_llm_reasoning_effort')) AND length(CAST(value_json AS BLOB)) <= 524288 LIMIT 256".into(), vec![]).await.map_err(failure)?;
            let values: BTreeMap<String, String> = rows.iter().map(|row| Ok((row["id"].as_str().ok_or_else(|| failure("Invalid setting ID"))?.to_owned(), row["value_json"].as_str().ok_or_else(|| failure("Invalid setting value"))?.to_owned()))).collect::<Result<_>>()?;
            let parsed = values.iter().map(|(key, value)| Ok((key.clone(), serde_json::from_str(value).map_err(failure)?))).collect::<Result<_>>()?;
            Ok(Snapshot { values, parsed })
        });
        let on_device = (self.services.on_device)();
        cx.spawn(async move |this, cx| {
            let snapshot = async {
                match reply {
                    Ok(reply) => reply.receive().await,
                    Err(error) => Err(error),
                }
            };
            let (result, status) = futures::future::join(snapshot, on_device).await;
            let _ = this.update(cx, |this, cx| {
                this.operation = None;
                this.on_device = Some(status);
                this.on_device_loading = false;
                match result {
                    Ok(snapshot) => {
                        this.snapshot = Some(snapshot);
                        if restore
                            && this.generation == generation
                            && !this.queued
                            && this.draft.is_none()
                        {
                            this.restore(cx);
                        }
                        if this
                            .draft
                            .is_some_and(|draft| this.current("provider") == draft)
                        {
                            this.draft = None;
                        }
                    }
                    Err(error) => this.status = error.to_string(),
                }
                this.save_queued(cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn current(&self, field: &str) -> String {
        self.snapshot
            .as_ref()
            .map(|snapshot| snapshot.choice(&format!("current_{}_{field}", self.kind.key())))
            .unwrap_or_default()
    }

    fn provider(&self) -> String {
        self.draft
            .map(str::to_owned)
            .unwrap_or_else(|| self.current("provider"))
    }

    /// The base URL stored for `provider`, else the catalog default.
    fn stored_base(&self, provider: &str) -> String {
        self.snapshot
            .as_ref()
            .and_then(|snapshot| {
                snapshot
                    .parsed
                    .get(&format!("ai_provider:{}:{provider}", self.kind.key()))
            })
            .and_then(|value| value["base_url"].as_str())
            .filter(|base| !base.is_empty())
            .map(str::to_owned)
            .or_else(|| {
                catalog(self.kind)
                    .iter()
                    .find(|info| info.id == provider)
                    .map(|info| info.base_url.to_owned())
            })
            .unwrap_or_default()
    }

    fn configured(&self, provider: &str) -> bool {
        self.snapshot.as_ref().is_some_and(|snapshot| {
            snapshot
                .values
                .contains_key(&format!("ai_provider:{}:{provider}", self.kind.key()))
        })
    }

    fn restore(&mut self, cx: &mut Context<Self>) {
        self.restore_choice(cx);
        self.restore_card(cx);
    }

    fn restore_choice(&mut self, cx: &mut Context<Self>) {
        let model = self.current("model");
        self.draft = None;
        self.model.update(cx, |input, cx| input.set_text(model, cx));
        self.dirty_choice = false;
    }

    fn restore_card(&mut self, cx: &mut Context<Self>) {
        let base = self
            .expanded
            .map(|provider| self.stored_base(provider))
            .unwrap_or_default();
        self.base.update(cx, |input, cx| input.set_text(base, cx));
        self.secret
            .update(cx, |input, cx| input.set_text(String::new(), cx));
        self.dirty_card = false;
    }

    fn is_local_model_installed(&self, model_id: &str) -> bool {
        self.on_device
            .as_ref()
            .is_some_and(|status| status.installed_stt_models.contains(model_id))
    }

    fn provider_unavailable_reason(
        &self,
        kind: ProviderKind,
        provider: &ProviderInfo,
    ) -> Option<String> {
        if provider.subscription {
            return Some(
                "Connecting a subscription needs the browser sign-in flow, which the native app does not have yet."
                    .into(),
            );
        }
        match (kind, provider.id) {
            (ProviderKind::Stt, "soniqo") => match &self.on_device {
                Some(status) if status.soniqo_available => None,
                Some(_) => Some("Requires Apple Silicon.".into()),
                None => Some("Checking on-device availability…".into()),
            },
            (ProviderKind::Stt, "apple_speech") => match &self.on_device {
                Some(status) if status.apple_speech_available => None,
                Some(_) => Some("Requires macOS 26 or later.".into()),
                None => Some("Checking on-device availability…".into()),
            },
            (ProviderKind::Stt, "local_file") => Some("Not available in this build.".into()),
            (ProviderKind::Llm, "apple_foundation") => match &self.on_device {
                Some(status) => status.apple_foundation_unavailable.clone(),
                None => Some("Checking Apple Intelligence availability…".into()),
            },
            _ => None,
        }
    }

    /// Shipping's `handleProviderChange`: switches to `provider` with a model chosen for it and
    /// saves right away. A transcription provider with no model to default to is saved
    /// without one; a language model provider waits until one is entered.
    fn pick_provider(&mut self, provider: &'static str, cx: &mut Context<Self>) {
        self.menu = false;
        let current = self.current("provider");
        // While a save is in flight the stored choice is about to change, so picking it again
        // is a new choice to queue, not a no-op.
        if provider == current && self.operation.is_none() {
            self.restore(cx);
            cx.notify();
            return;
        }
        if let Some(info) = catalog(self.kind).iter().find(|info| info.id == provider)
            && let Some(reason) = self.provider_unavailable_reason(self.kind, info)
        {
            self.status = reason;
            cx.notify();
            return;
        }
        let model = self.next_model(provider, cx);
        if is_local_stt_provider(self.kind, provider) && model.is_empty() {
            self.status =
                "Download a model in Manage local models before selecting this provider.".into();
            cx.notify();
            return;
        }
        let saved = self.current("model");
        if !current.is_empty() && !saved.is_empty() {
            self.remembered.insert((self.kind, current), saved);
        }
        self.draft = Some(provider);
        self.dirty_choice = false;
        self.model
            .update(cx, |input, cx| input.set_text(model.clone(), cx));
        if self.kind == ProviderKind::Stt || !model.is_empty() {
            self.save_choice(cx);
        } else {
            self.status = "Enter a model and press Return to use this provider.".into();
        }
        cx.notify();
    }

    /// The model `provider` starts with: its last model this session if still offered, else
    /// the first one offered, else its default (`getPreferredProviderModel` and
    /// `getDefaultSttModel`).
    fn next_model(&self, provider: &str, cx: &gpui::App) -> String {
        let models = self.models(provider, cx);
        // Typed models cannot be checked against a list; shipping keeps custom ones.
        let typed = models.is_empty()
            && (self.kind == ProviderKind::Llm || matches!(provider, "custom" | "nvidia"));
        self.remembered
            .get(&(self.kind, provider.to_owned()))
            .filter(|model| {
                typed
                    || models
                        .iter()
                        .any(|(id, _, disabled_reason)| disabled_reason.is_none() && id == model)
            })
            .cloned()
            .or_else(|| {
                models
                    .iter()
                    .find(|(_, _, disabled_reason)| disabled_reason.is_none())
                    .map(|(id, ..)| (*id).to_owned())
            })
            .or_else(|| {
                (self.kind == ProviderKind::Stt && !is_local_stt_provider(self.kind, provider))
                    .then(|| provider_catalog::default_stt_model(provider))
                    .flatten()
                    .map(str::to_owned)
            })
            .unwrap_or_default()
    }

    /// `provider`'s fixed models as `(id, label, disabled_reason)`; without them the model is typed.
    /// Anarlog's transcription model is offered on a paid plan, where shipping lists it as
    /// downloaded.
    fn models(
        &self,
        provider: &str,
        cx: &gpui::App,
    ) -> Vec<(&'static str, &'static str, Option<SharedString>)> {
        match self.kind {
            ProviderKind::Stt => provider_catalog::stt_models(provider)
                .iter()
                .map(|id| {
                    let disabled_reason = if *id == "cloud" && !super::account::is_paid(cx) {
                        Some(SharedString::from("Upgrade to use"))
                    } else if is_local_stt_provider(self.kind, provider)
                        && !self.is_local_model_installed(id)
                    {
                        Some(SharedString::from("Download in Manage local models"))
                    } else {
                        None
                    };
                    (*id, provider_catalog::stt_model_label(id), disabled_reason)
                })
                .collect(),
            ProviderKind::Llm if provider == "anarlog" => provider_catalog::ANARLOG_LLM_MODELS
                .iter()
                .map(|(id, label)| (*id, *label, None))
                .collect(),
            ProviderKind::Llm if provider == "apple_foundation" => vec![(
                provider_catalog::APPLE_FOUNDATION_MODEL,
                provider_catalog::APPLE_FOUNDATION_MODEL,
                self.on_device
                    .as_ref()
                    .and_then(|status| status.apple_foundation_unavailable.clone())
                    .map(SharedString::from),
            )],
            ProviderKind::Llm => Vec::new(),
        }
    }

    fn toggle_card(&mut self, provider: &'static str, cx: &mut Context<Self>) {
        let editing = self.base.read(cx).buffer.text
            != self
                .expanded
                .map(|id| self.stored_base(id))
                .unwrap_or_default()
            || !self.secret.read(cx).buffer.text.is_empty();
        if editing && self.operation.is_none() {
            self.status = "Press Return to save the provider, or clear the API key field.".into();
            cx.notify();
            return;
        }
        self.expanded = (self.expanded != Some(provider)).then_some(provider);
        self.restore_card(cx);
        cx.notify();
    }

    fn busy(&self, cx: &gpui::App) -> bool {
        self.operation.is_some()
            || [&self.model, &self.base, &self.secret]
                .into_iter()
                .any(|input| input.read(cx).buffer.marked.is_some())
    }

    /// Saves the active provider and model ("Model being used"), after the load or save in
    /// flight if there is one.
    fn save_choice(&mut self, cx: &mut Context<Self>) {
        if self.operation.is_some() {
            self.queued = true;
            return;
        }
        let Some(snapshot) = self.snapshot.clone().filter(|_| !self.busy(cx)) else {
            return;
        };
        let provider = self.provider();
        let model = self.model.read(cx).buffer.text.clone();
        if let Some(info) = catalog(self.kind).iter().find(|info| info.id == provider)
            && let Some(reason) = self.provider_unavailable_reason(self.kind, info)
        {
            self.status = reason;
            self.restore_choice(cx);
            cx.notify();
            return;
        }
        if is_local_stt_provider(self.kind, &provider)
            && !self.is_local_model_installed(model.trim())
        {
            self.status = "Download this model in Manage local models before selecting it.".into();
            self.restore_choice(cx);
            cx.notify();
            return;
        }
        let base = self.stored_base(&provider);
        let kind = self.kind;
        let reply = self.services.runtime.submit(move |services| async move {
            let statements = snapshot.statements(kind, &provider, &model, &base)?;
            services
                .executor
                .execute_transaction(statements)
                .await
                .map_err(failure)?;
            Ok(())
        });
        self.finish(
            reply,
            Saved::Choice,
            "Provider choice saved. Credentials are checked when the provider is used.",
            cx,
        );
    }

    fn save_queued(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.queued) {
            self.save_choice(cx);
        }
    }

    /// Saves the expanded card: its base URL row, and a new API key to the keyring.
    fn save_provider(&mut self, cx: &mut Context<Self>) {
        let (Some(snapshot), Some(provider)) = (self.snapshot.clone(), self.expanded) else {
            return;
        };
        if self.busy(cx) {
            return;
        }
        let base = self.base.read(cx).buffer.text.trim().to_owned();
        let secret = self.secret.read(cx).buffer.text.clone();
        if !secret.is_ascii() {
            self.status = "Enter an ASCII API key.".into();
            cx.notify();
            return;
        }
        let kind = self.kind;
        let writer = self.services.secret_write.clone();
        let runtime = self.services.runtime.clone();
        let reply = self.services.runtime.service(move |_| async move {
            let statement = snapshot.provider_statement(kind, provider, &base)?;
            if !secret.is_empty() {
                writer(kind, provider.to_owned(), secret).await?;
            }
            runtime
                .submit(move |services| async move {
                    services
                        .executor
                        .execute_transaction(vec![statement])
                        .await
                        .map_err(failure)?;
                    Ok(())
                })?
                .receive()
                .await
        });
        self.finish(
            reply,
            Saved::Card,
            "Provider saved. API keys are stored in the system keyring.",
            cx,
        );
    }

    fn finish(
        &mut self,
        reply: Result<desktop_runtime::Reply<()>>,
        part: Saved,
        saved: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.operation = Some(Operation::Saving);
        let generation = self.generation;
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                this.operation = None;
                match result {
                    Ok(()) => {
                        this.status = saved.into();
                        // Only what this save submitted is clean; other edits stay unsaved.
                        if this.generation == generation {
                            match part {
                                Saved::Choice => this.dirty_choice = false,
                                Saved::Card => {
                                    this.dirty_card = false;
                                    this.secret
                                        .update(cx, |input, cx| input.set_text(String::new(), cx));
                                }
                                Saved::Effort => {}
                            }
                        }
                        this.reload(false, cx);
                    }
                    Err(error) => {
                        this.status = format!(
                            "{error}. Draft retained; reload to resolve a concurrent change."
                        );
                        this.save_queued(cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// The provider `SelectContent`: each item is the provider's icon and name, with the
    /// upgrade note or description under it (`settings/ai/{stt,llm}/select.tsx`).
    fn provider_menu(&self, colors: &Theme, cx: &mut Context<Self>) -> gpui::Stateful<Div> {
        let current = self.provider();
        controls::menu_panel(colors).child(
            div()
                .id("provider-options")
                .max_h(px(300.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .children(
                    catalog(self.kind)
                        .iter()
                        .filter(|info| !folded(self.kind, info.id))
                        .map(|info| {
                            // Shipping's hosted language model requires a paid plan.
                            let locked = self.kind == ProviderKind::Llm
                                && info.id == "anarlog"
                                && !super::account::is_paid(cx);
                            let unavailable = self.provider_unavailable_reason(self.kind, info);
                            let needs_download = unavailable.is_none()
                                && is_local_stt_provider(self.kind, info.id)
                                && !provider_catalog::stt_models(info.id)
                                    .iter()
                                    .any(|model| self.is_local_model_installed(model));
                            let enabled = !locked && unavailable.is_none() && !needs_download;
                            let id = info.id;
                            let note: Option<String> = if locked {
                                Some("Upgrade to Pro to use this provider.".into())
                            } else if info.subscription {
                                None
                            } else if let Some(reason) = unavailable {
                                Some(reason)
                            } else if needs_download {
                                Some("Download a model in Manage local models to use.".into())
                            } else if self.kind == ProviderKind::Stt {
                                provider_catalog::stt_description(id).map(str::to_owned)
                            } else {
                                None
                            };
                            controls::select_item(
                                SharedString::from(format!("provider-option-{id}")),
                                match note.as_deref() {
                                    Some(note) => format!("{}, {note}", info.name).into(),
                                    None => info.name.into(),
                                },
                                id == current,
                                colors,
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .size(px(20.))
                                                    .flex_shrink_0()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(provider_logo(info, 12., colors)),
                                            )
                                            .child(info.name),
                                    )
                                    .when_some(note, |view, note| {
                                        view.child(
                                            div()
                                                .text_size(px(11.))
                                                .line_height(px(16.))
                                                .text_color(colors.muted_foreground)
                                                .child(note),
                                        )
                                    }),
                            )
                            .when(locked, |view| {
                                view.text_color(colors.muted_foreground).cursor_default()
                            })
                            .when(!enabled && !locked, |view| {
                                view.opacity(0.5).cursor_default()
                            })
                            .a11y_disabled(!enabled)
                            .when(enabled, |view| {
                                view.on_click(
                                    cx.listener(move |this, _, _, cx| this.pick_provider(id, cx)),
                                )
                            })
                        }),
                ),
        )
    }

    fn card(
        &self,
        info: &'static ProviderInfo,
        colors: &Theme,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let open = self.expanded == Some(info.id);
        let ready = self.configured(info.id);
        let id = info.id;
        div()
            .id(SharedString::from(format!("provider-card-{id}")))
            .border_2()
            .border_color(colors.border)
            .when(!ready, |view| view.border_dashed())
            .bg(colors.muted)
            .child(
                div()
                    .id(SharedString::from(format!("provider-header-{id}")))
                    .a11y_button(info.name)
                    .aria_expanded(open)
                    .px_4()
                    .py_4()
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_card(id, cx)))
                    .child(
                        div()
                            .size(px(16.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(provider_logo(info, 12., colors)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .line_height(px(20.))
                            .font_weight(crate::ui::theme::MEDIUM)
                            .child(info.name),
                    )
                    .when_some(info.badge, |view, badge| {
                        view.child(
                            div()
                                .px_2()
                                .border_1()
                                .border_color(colors.border)
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child(badge),
                        )
                    })
                    .child(div().flex_1())
                    .child(controls::icon(
                        if open { "ArrowUp01Icon.svg" } else { "ArrowDown01Icon.svg" },
                        16.,
                        colors.foreground,
                    )),
            )
            .when(open, |view| {
                view.child(
                    div()
                        .px_4()
                        .pb_4()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .when(info.subscription, |view| {
                            view.child(controls::description(
                                "Connecting a subscription needs the browser sign-in flow, which the native app does not have yet.",
                                colors,
                            ))
                        })
                        .when(!info.subscription && (info.needs_base_url || !info.base_url.is_empty()), |view| {
                            view.child(field("Base URL", self.base.clone()))
                        })
                        .when(!info.subscription && info.needs_api_key, |view| {
                            view.child(field("API Key", self.secret.clone()))
                        })
                        .when(!info.subscription, |view| {
                            view.child(controls::description("Press Return to save.", colors))
                        }),
                )
            })
    }
}

fn field(label: &'static str, input: Entity<TextInput>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(crate::ui::theme::MEDIUM)
                .child(label),
        )
        .child(input)
}

impl ProviderView {
    /// The model half of "Model being used": a menu of the provider's models when it has a
    /// fixed list, else the typed identifier.
    fn model_select(
        &self,
        provider: &str,
        colors: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let models = self.models(provider, cx);
        let field = div().relative().flex_grow(3.).flex_basis(px(0.)).min_w_0();
        if provider.is_empty() {
            return field.child(
                pill(colors)
                    .justify_between()
                    .opacity(0.5)
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_color(colors.muted_foreground)
                            .child("Select a model"),
                    )
                    .child(controls::icon(
                        "ArrowDown01Icon.svg",
                        16.,
                        colors.foreground.opacity(0.5),
                    )),
            );
        }
        if models.is_empty() {
            return field.child(
                pill(colors).child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h(px(20.))
                        .line_height(px(20.))
                        .child(self.model.clone()),
                ),
            );
        }
        let current = self.model.read(cx).buffer.text.clone();
        let selected = models
            .iter()
            .find(|(id, ..)| *id == current)
            .map(|(id, label, _)| (*id, *label));
        let display = selected.map(|(_, label)| label.to_owned()).or_else(|| {
            (!current.trim().is_empty()).then(|| match self.kind {
                ProviderKind::Stt => provider_catalog::stt_model_label(&current).to_owned(),
                ProviderKind::Llm => current.clone(),
            })
        });
        let muted = colors.muted_foreground;
        // Shipping heads Anarlog's cloud model with its "latest" category.
        let recommended = self.kind == ProviderKind::Stt && provider == "anarlog";
        let menu = self.model_menu.then(|| {
            controls::menu_panel(colors).child(
                div()
                    .id("model-options")
                    .max_h(px(300.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .when(recommended, |view| {
                        view.child(
                            div()
                                .px_2()
                                .pt_2()
                                .pb_1()
                                .text_size(px(11.))
                                .font_weight(crate::ui::theme::MEDIUM)
                                .text_color(muted)
                                .child("RECOMMENDED"),
                        )
                    })
                    .children(models.into_iter().map(|(model, label, disabled_reason)| {
                        let offered = disabled_reason.is_none();
                        controls::menu_item(
                            SharedString::from(format!("model-{model}")),
                            label,
                            disabled_reason,
                            Some(model) == selected.map(|(id, _)| id),
                            colors,
                        )
                        .a11y_disabled(!offered)
                        .when(!offered, |view| view.text_color(muted).cursor_default())
                        .when(offered, |view| {
                            view.on_click(cx.listener(move |this, _, _, cx| {
                                this.model_menu = false;
                                this.model
                                    .update(cx, |input, cx| input.set_text(model.into(), cx));
                                this.save_choice(cx);
                                cx.notify();
                            }))
                        })
                    })),
            )
        });
        field
            .child(
                pill(colors)
                    .id("model-select")
                    .role(gpui::Role::ComboBox)
                    .aria_label("Model")
                    .aria_value(display.clone().unwrap_or_else(|| "Select a model".into()))
                    .aria_expanded(self.model_menu)
                    .justify_between()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.model_menu = !this.model_menu;
                        if this.model_menu {
                            this.refresh_on_device(cx);
                        }
                        cx.notify();
                    }))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .when(display.is_none(), |view| {
                                view.text_color(colors.muted_foreground)
                            })
                            .child(display.unwrap_or_else(|| "Select a model".into())),
                    )
                    .child(controls::icon(
                        "ArrowDown01Icon.svg",
                        16.,
                        colors.foreground.opacity(0.5),
                    )),
            )
            .child(controls::dropdown(
                "model-select-menu",
                menu,
                true,
                |this: &mut Self| this.model_menu = false,
                window,
                cx,
            ))
    }
}

impl ProviderView {
    /// `Reasoning effort` (`settings/ai/llm/select.tsx`), stored for providers that take one.
    fn reasoning_effort(
        &self,
        colors: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        const EFFORTS: [(&str, &str); 4] = [
            ("default", "Default"),
            ("low", "Low"),
            ("medium", "Medium"),
            ("high", "High"),
        ];
        let stored = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.choice("current_llm_reasoning_effort"))
            .unwrap_or_default();
        let current = EFFORTS
            .iter()
            .find(|(value, _)| *value == stored)
            .unwrap_or(&EFFORTS[0]);
        let enabled = self.snapshot.is_some() && !self.busy(cx);
        controls::setting_row(
            "Reasoning effort",
            Some(
                "How much the model thinks before answering. Default leaves it to the provider."
                    .into(),
            ),
            true,
            div()
                .relative()
                .w_full()
                .child(
                    controls::select_trigger(
                        "reasoning-effort",
                        Some(current.1.into()),
                        "",
                        enabled,
                        colors,
                    )
                    .aria_label("Reasoning effort")
                    .aria_expanded(self.effort_menu)
                    .when(enabled, |view| {
                        view.on_click(cx.listener(|this, _, _, cx| {
                            this.effort_menu = !this.effort_menu;
                            cx.notify();
                        }))
                    }),
                )
                .child(controls::dropdown(
                    "reasoning-effort-menu",
                    self.effort_menu.then(|| {
                        controls::menu_panel(colors).children(EFFORTS.into_iter().map(
                            |(value, label)| {
                                controls::menu_item(
                                    SharedString::from(format!("effort-{value}")),
                                    label,
                                    None,
                                    value == current.0,
                                    colors,
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.effort_menu = false;
                                        this.save_effort(value, cx);
                                    },
                                ))
                            },
                        ))
                    }),
                    false,
                    |this: &mut Self| this.effort_menu = false,
                    window,
                    cx,
                )),
            colors,
        )
    }

    fn save_effort(&mut self, effort: &'static str, cx: &mut Context<Self>) {
        let Some(snapshot) = self.snapshot.clone().filter(|_| !self.busy(cx)) else {
            return;
        };
        let statement = snapshot.upsert("current_llm_reasoning_effort".into(), json!(effort));
        let reply = self.services.runtime.submit(move |services| async move {
            services
                .executor
                .execute_transaction(vec![statement])
                .await
                .map_err(failure)?;
            Ok(())
        });
        self.finish(reply, Saved::Effort, "Reasoning effort saved.", cx);
    }
}

fn pill(colors: &Theme) -> Div {
    div()
        .h(px(36.))
        .px_3()
        .flex()
        .items_center()
        .gap_2()
        .border_1()
        .border_color(colors.input)
        .bg(colors.card)
        .text_sm()
}

impl Render for ProviderView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let provider = self.provider();
        let provider_info = catalog(self.kind).iter().find(|info| info.id == provider);
        let provider_name = provider_info
            .map(|info| SharedString::from(info.name))
            .or_else(|| (!provider.is_empty()).then(|| provider.clone().into()));
        let query = self.search.read(cx).buffer.text.trim().to_lowercase();
        let cards = catalog(self.kind)
            .iter()
            .filter(|info| !info.built_in)
            .filter(|info| !folded(self.kind, info.id) || !query.is_empty())
            .filter(|info| {
                query.is_empty()
                    || info.name.to_lowercase().contains(&query)
                    || info.id.contains(&query)
            })
            .collect::<Vec<_>>();
        let empty = cards.is_empty();
        let heading = |text: &'static str| {
            div()
                .text_base()
                .line_height(px(24.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(text)
        };
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(heading("Model being used"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .child(
                                div()
                                    .relative()
                                    .flex_grow(2.)
                                    .flex_basis(px(0.))
                                    .min_w_0()
                                    .child(
                                        pill(&colors)
                                            .id("provider-select")
                                            .role(gpui::Role::ComboBox)
                                            .aria_label("Provider")
                                            .aria_expanded(self.menu)
                                            .justify_between()
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.menu = !this.menu;
                                                if this.menu {
                                                    this.refresh_on_device(cx);
                                                }
                                                cx.notify();
                                            }))
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .when_some(provider_info, |view, info| {
                                                        view.child(provider_logo(
                                                            info, 14., &colors,
                                                        ))
                                                    })
                                                    .child(
                                                        div()
                                                            .min_w_0()
                                                            .truncate()
                                                            .when(provider_name.is_none(), |view| {
                                                                view.text_color(
                                                                    colors.muted_foreground,
                                                                )
                                                            })
                                                            .child(provider_name.unwrap_or_else(
                                                                || "Select a provider".into(),
                                                            )),
                                                    ),
                                            )
                                            .child(controls::icon(
                                                "ArrowDown01Icon.svg",
                                                16.,
                                                colors.foreground.opacity(0.5),
                                            )),
                                    )
                                    .child(controls::dropdown(
                                        "provider-select-menu",
                                        self.menu.then(|| self.provider_menu(&colors, cx)),
                                        false,
                                        |this: &mut Self| this.menu = false,
                                        window,
                                        cx,
                                    )),
                            )
                            .child(div().text_color(colors.muted_foreground).child("/"))
                            .child(self.model_select(&provider, &colors, window, cx)),
                    )
                    .when(
                        self.kind == ProviderKind::Llm
                            && !provider.is_empty()
                            && !matches!(provider.as_str(), "anarlog" | "apple_foundation"),
                        |view| view.child(self.reasoning_effort(&colors, window, cx)),
                    )
                    .when(
                        self.kind == ProviderKind::Stt
                            && matches!(provider.as_str(), "soniqo" | "apple_speech"),
                        |view| {
                            view.child(controls::description(
                                "Download or remove on-device transcription models in Manage local models.",
                                &colors,
                            ))
                        },
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .child(heading("Configure Providers"))
                            .child(
                                div()
                                    .w(px(224.))
                                    .h(px(32.))
                                    .px_3()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .border_1()
                                    .border_color(colors.border)
                                    .bg(colors.accent.opacity(0.5))
                                    .text_sm()
                                    .child(controls::icon(
                                        "Search01Icon.svg",
                                        16.,
                                        colors.muted_foreground,
                                    ))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .h(px(20.))
                                            .line_height(px(20.))
                                            .child(self.search.clone()),
                                    ),
                            ),
                    )
                    .when(!self.status.is_empty(), |view| {
                        view.child(controls::description(self.status.clone(), &colors))
                    })
                    .children(cards.into_iter().map(|info| self.card(info, &colors, cx)))
                    .when(empty, |view| {
                        view.child(
                            div()
                                .py_8()
                                .text_center()
                                .text_sm()
                                .text_color(colors.muted_foreground)
                                .child("No providers found."),
                        )
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use gpui::{AppContext, TestAppContext};

    use super::*;

    struct Fixture {
        view: Entity<ProviderView>,
        runtime: desktop_runtime::RuntimeHandle,
        services: ProviderServices,
        on_device: Arc<std::sync::Mutex<crate::meeting::config::OnDeviceStatus>>,
        _directory: tempfile::TempDir,
    }

    fn set_paid(cx: &mut TestAppContext) {
        use super::super::account::{self, Account, Claims, User};
        cx.update(|cx| {
            account::set(
                Account {
                    user: Some(User {
                        id: "user".into(),
                        email: None,
                    }),
                    claims: Some(Claims {
                        entitlements: vec!["hyprnote_pro".into()],
                        subscription_status: Some("active".into()),
                        current_period_end: Some(account::now() + 86_400),
                        ..Claims::default()
                    }),
                    workspaces: Some(vec![].into()),
                    ..Account::default()
                },
                cx,
            )
        });
    }

    fn fixture(cx: &mut TestAppContext) -> Fixture {
        cx.executor().allow_parking();
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = desktop_runtime::RuntimeHandle::start(desktop_runtime::Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        futures::executor::block_on(ready.receive()).unwrap();
        let on_device = Arc::new(std::sync::Mutex::new(
            crate::meeting::config::OnDeviceStatus {
                soniqo_available: true,
                apple_speech_available: true,
                installed_stt_models: Default::default(),
                apple_foundation_unavailable: None,
            },
        ));
        let on_device_state = on_device.clone();
        let services = ProviderServices {
            runtime: runtime.clone(),
            api_url: String::new(),
            cloud: Arc::new(|| Box::pin(async { Err(ServiceError::Closed) })),
            local: Arc::new(|_| Box::pin(async { Err(ServiceError::Closed) })),
            on_device: Arc::new(move || {
                let status = on_device_state.lock().unwrap().clone();
                Box::pin(async move { status })
            }),
            // Like the keyring resolver, which reads on Tokio's blocking pool.
            secret: Arc::new(|_, _| {
                Box::pin(async { tokio::task::spawn_blocking(|| None).await.map_err(failure) })
            }),
            secret_write: Arc::new(|_, _, _| Box::pin(async { Ok(()) })),
        };
        let view = cx.new(|cx| ProviderView::new(services.clone(), cx));
        let fixture = Fixture {
            view,
            runtime,
            services,
            on_device,
            _directory: directory,
        };
        fixture.settle(cx);
        fixture
    }

    impl Fixture {
        /// Runs until the view has loaded and nothing is in flight or queued.
        fn settle(&self, cx: &mut TestAppContext) {
            for _ in 0..400 {
                cx.run_until_parked();
                if self.view.read_with(cx, |view, _| {
                    view.snapshot.is_some()
                        && view.operation.is_none()
                        && !view.queued
                        && view.on_device.is_some()
                        && !view.on_device_loading
                }) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            panic!("provider view did not settle");
        }

        fn set_on_device(
            &self,
            status: crate::meeting::config::OnDeviceStatus,
            cx: &mut TestAppContext,
        ) {
            *self.on_device.lock().unwrap() = status;
            self.view.update(cx, |view, cx| view.refresh(cx));
            self.settle(cx);
        }

        fn pick(&self, provider: &'static str, cx: &mut TestAppContext) {
            self.view
                .update(cx, |view, cx| view.pick_provider(provider, cx));
            self.settle(cx);
        }

        /// A stored setting's string value.
        fn setting(&self, key: &'static str) -> Option<String> {
            let rows = futures::executor::block_on(
                self.runtime
                    .submit(move |services| async move {
                        services
                            .executor
                            .execute(
                                "SELECT value_json FROM app_settings WHERE id = ?1".into(),
                                vec![json!(key)],
                            )
                            .await
                            .map_err(failure)
                    })
                    .unwrap()
                    .receive(),
            )
            .unwrap();
            rows.first().map(|row| {
                serde_json::from_str::<String>(row["value_json"].as_str().unwrap()).unwrap()
            })
        }

        fn shutdown(self) {
            futures::executor::block_on(self.runtime.shutdown()).unwrap();
        }
    }

    #[gpui::test]
    fn saving_the_model_keeps_an_api_key_typed_but_not_saved(cx: &mut TestAppContext) {
        let fixture = fixture(cx);
        let view = fixture.view.clone();
        view.update(cx, |view, cx| {
            view.expanded = Some("deepgram");
            view.secret.update(cx, |input, cx| {
                input.set_text("typed-key".into(), cx);
                cx.emit(InputEvent::Changed);
            });
        });
        cx.run_until_parked();
        // Picking a model from its menu saves the choice right away.
        view.update(cx, |view, cx| {
            view.draft = Some("deepgram");
            view.model
                .update(cx, |input, cx| input.set_text("nova-3".into(), cx));
            view.save_choice(cx);
        });
        fixture.settle(cx);
        view.read_with(cx, |view, cx| {
            assert_eq!(view.secret.read(cx).buffer.text, "typed-key");
            assert!(view.has_unsaved(cx));
        });
        fixture.shutdown();
    }

    #[gpui::test]
    fn rejected_model_choice_keeps_unsaved_provider_card_drafts(cx: &mut TestAppContext) {
        let fixture = fixture(cx);
        fixture.set_on_device(
            crate::meeting::config::OnDeviceStatus {
                soniqo_available: true,
                apple_speech_available: true,
                installed_stt_models: Default::default(),
                apple_foundation_unavailable: Some(
                    "Enable Apple Intelligence in System Settings.".into(),
                ),
            },
            cx,
        );
        let view = fixture.view.clone();
        view.update(cx, |view, cx| {
            view.select_kind(ProviderKind::Llm, cx);
            view.expanded = Some("openai");
            view.secret.update(cx, |input, cx| {
                input.set_text("unsaved-openai-key".into(), cx);
                cx.emit(InputEvent::Changed);
            });
            view.dirty_card = true;
            view.draft = Some("apple_foundation");
            view.model.update(cx, |input, cx| {
                input.set_text(provider_catalog::APPLE_FOUNDATION_MODEL.into(), cx);
            });
            view.save_choice(cx);
        });
        fixture.settle(cx);
        view.read_with(cx, |view, cx| {
            assert_eq!(view.secret.read(cx).buffer.text, "unsaved-openai-key");
            assert!(view.dirty_card);
            assert!(view.has_unsaved(cx));
            assert!(
                view.status.contains("Enable Apple Intelligence"),
                "{}",
                view.status
            );
        });
        fixture.shutdown();
    }

    #[gpui::test]
    fn picking_a_provider_saves_a_model_for_it_and_never_blocks_leaving(cx: &mut TestAppContext) {
        let fixture = fixture(cx);
        // Leaves OpenAI's transcription model remembered for this session.
        fixture.pick("openai", cx);
        fixture.pick("deepgram", cx);
        assert_eq!(fixture.setting("current_stt_provider").unwrap(), "deepgram");
        assert_eq!(
            fixture.setting("current_stt_model").unwrap(),
            "flux-general-multi"
        );
        assert!(fixture.view.read_with(cx, |view, cx| !view.has_unsaved(cx)));
        // A language model provider whose models are typed waits for one, without
        // holding the page, and never takes its transcription model.
        fixture
            .view
            .update(cx, |view, cx| view.select_kind(ProviderKind::Llm, cx));
        fixture.pick("openai", cx);
        assert_eq!(fixture.setting("current_llm_provider"), None);
        assert!(fixture.view.read_with(cx, |view, cx| !view.has_unsaved(cx)));
        fixture.shutdown();
    }

    #[gpui::test]
    fn a_provider_picked_while_another_saves_is_saved_after_it(cx: &mut TestAppContext) {
        let fixture = fixture(cx);
        fixture.view.update(cx, |view, cx| {
            view.pick_provider("deepgram", cx);
            assert!(view.operation.is_some(), "the first choice is still saving");
            view.pick_provider("assemblyai", cx);
        });
        fixture.settle(cx);
        assert_eq!(
            fixture.setting("current_stt_provider").unwrap(),
            "assemblyai"
        );
        assert_eq!(
            fixture.setting("current_stt_model").unwrap(),
            "universal-3-5-pro"
        );
        // Picking the saved provider again before another pick's save finishes keeps it.
        fixture.view.update(cx, |view, cx| {
            view.pick_provider("deepgram", cx);
            view.pick_provider("assemblyai", cx);
        });
        fixture.settle(cx);
        assert_eq!(
            fixture.setting("current_stt_provider").unwrap(),
            "assemblyai"
        );
        fixture.shutdown();
    }

    /// A Pro account with nothing chosen kept "Transcription provider needed" until Anarlog
    /// was picked by hand. Turning paid now saves Anarlog's hosted models for a missing
    /// transcription choice and an unusable language model, as shipping does.
    #[gpui::test]
    fn turning_paid_saves_anarlog_for_missing_and_unusable_choices(cx: &mut TestAppContext) {
        let fixture = fixture(cx);
        // OpenAI without an API key cannot answer.
        futures::executor::block_on(
            fixture
                .runtime
                .submit(|services| async move {
                    services
                        .executor
                        .execute(
                            "INSERT INTO app_settings (id, value_json) VALUES ('current_llm_provider', '\"openai\"'), ('current_llm_model', '\"gpt-5\"')".into(),
                            vec![],
                        )
                        .await
                        .map_err(failure)
                })
                .unwrap()
                .receive(),
        )
        .unwrap();
        let services = fixture.services.clone();
        let _watch = cx.update(|cx| configure_when_paid(services, cx));
        cx.run_until_parked();
        assert_eq!(fixture.setting("current_stt_provider"), None);
        set_paid(cx);
        for _ in 0..400 {
            cx.run_until_parked();
            if fixture.setting("current_llm_model").as_deref() == Some("Auto") {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(fixture.setting("current_stt_provider").unwrap(), "anarlog");
        assert_eq!(fixture.setting("current_stt_model").unwrap(), "cloud");
        assert_eq!(fixture.setting("current_llm_provider").unwrap(), "anarlog");
        assert_eq!(fixture.setting("current_llm_model").unwrap(), "Auto");
        fixture.shutdown();
    }

    #[gpui::test]
    fn a_choice_saved_while_paid_defaults_are_decided_is_kept(cx: &mut TestAppContext) {
        let fixture = fixture(cx);
        let services = fixture.services.clone();
        // Decided with nothing saved, so both kinds would get Anarlog.
        let statements = futures::executor::block_on(
            fixture
                .runtime
                .service(move |_| async move { paid_defaults(&services).await })
                .unwrap()
                .receive(),
        )
        .unwrap();
        assert!(!statements.is_empty());
        // The user saves a provider before those defaults are written.
        fixture.pick("deepgram", cx);
        let written = futures::executor::block_on(
            fixture
                .runtime
                .submit(move |services| async move {
                    services
                        .executor
                        .execute_transaction(statements)
                        .await
                        .map_err(failure)
                })
                .unwrap()
                .receive(),
        );
        assert!(written.is_err(), "the stale defaults are rejected");
        assert_eq!(fixture.setting("current_stt_provider").unwrap(), "deepgram");
        assert_eq!(fixture.setting("current_llm_provider"), None);
        fixture.shutdown();
    }

    #[gpui::test]
    fn picking_anarlog_saves_its_hosted_model(cx: &mut TestAppContext) {
        set_paid(cx);
        let fixture = fixture(cx);
        fixture.pick("anarlog", cx);
        assert_eq!(fixture.setting("current_stt_model").unwrap(), "cloud");
        fixture
            .view
            .update(cx, |view, cx| view.select_kind(ProviderKind::Llm, cx));
        fixture.pick("anarlog", cx);
        assert_eq!(fixture.setting("current_llm_model").unwrap(), "Auto");
        fixture.shutdown();
    }

    #[gpui::test]
    fn picking_on_device_providers_saves_soniqo_and_apple_foundation(cx: &mut TestAppContext) {
        let fixture = fixture(cx);
        fixture.pick("deepgram", cx);

        // Undownloaded local providers must not replace the working provider.
        fixture.pick("soniqo", cx);
        assert_eq!(fixture.setting("current_stt_provider").unwrap(), "deepgram");
        fixture.view.read_with(cx, |view, _| {
            assert!(
                view.status.contains("Manage local models"),
                "{}",
                view.status
            );
        });

        // With only Batch downloaded, picking Soniqo defaults to Batch and rejects Streaming.
        fixture.set_on_device(
            crate::meeting::config::OnDeviceStatus {
                soniqo_available: true,
                apple_speech_available: true,
                installed_stt_models: ["soniqo-parakeet-batch".to_owned()].into_iter().collect(),
                apple_foundation_unavailable: Some(
                    "Enable Apple Intelligence in System Settings.".into(),
                ),
            },
            cx,
        );
        fixture.pick("soniqo", cx);
        assert_eq!(fixture.setting("current_stt_provider").unwrap(), "soniqo");
        assert_eq!(
            fixture.setting("current_stt_model").unwrap(),
            "soniqo-parakeet-batch"
        );
        fixture.view.update(cx, |view, cx| {
            view.model.update(cx, |input, cx| {
                input.set_text("soniqo-parakeet-streaming".into(), cx)
            });
            view.save_choice(cx);
        });
        fixture.settle(cx);
        assert_eq!(
            fixture.setting("current_stt_model").unwrap(),
            "soniqo-parakeet-batch"
        );

        // Unavailable Apple Intelligence refuses selection until availability changes on refresh.
        set_paid(cx);
        fixture
            .view
            .update(cx, |view, cx| view.select_kind(ProviderKind::Llm, cx));
        fixture.settle(cx);
        fixture.pick("anarlog", cx);
        assert_eq!(fixture.setting("current_llm_provider").unwrap(), "anarlog");
        fixture.pick("apple_foundation", cx);
        assert_eq!(fixture.setting("current_llm_provider").unwrap(), "anarlog");
        fixture.view.read_with(cx, |view, _| {
            assert!(
                view.status.contains("Enable Apple Intelligence"),
                "{}",
                view.status
            );
        });

        *fixture.on_device.lock().unwrap() = crate::meeting::config::OnDeviceStatus {
            soniqo_available: true,
            apple_speech_available: true,
            installed_stt_models: Default::default(),
            apple_foundation_unavailable: None,
        };
        fixture.view.update(cx, |view, cx| view.refresh(cx));
        fixture.settle(cx);
        fixture.pick("apple_foundation", cx);
        assert_eq!(
            fixture.setting("current_llm_provider").unwrap(),
            "apple_foundation"
        );
        assert_eq!(
            fixture.setting("current_llm_model").unwrap(),
            provider_catalog::APPLE_FOUNDATION_MODEL
        );
        fixture.shutdown();
    }

    #[test]
    fn stale_provider_choice_rolls_back_the_whole_transaction() {
        futures::executor::block_on(async {
            let directory = tempfile::tempdir().unwrap();
            let (runtime, ready) =
                desktop_runtime::RuntimeHandle::start(desktop_runtime::Profile {
                    database: directory.path().join("library.sqlite"),
                })
                .unwrap();
            ready.receive().await.unwrap();
            let initial = Snapshot::default()
                .statements(ProviderKind::Llm, "custom", "old", "https://old.example")
                .unwrap();
            runtime
                .submit(move |services| async move {
                    services
                        .executor
                        .execute_transaction(initial)
                        .await
                        .map_err(failure)?;
                    Ok(())
                })
                .unwrap()
                .receive()
                .await
                .unwrap();
            let mut stale = Snapshot::default();
            stale
                .values
                .insert("current_llm_provider".into(), json!("custom").to_string());
            stale.values.insert(
                "current_llm_model".into(),
                json!("concurrent-version").to_string(),
            );
            let changes = stale
                .statements(ProviderKind::Llm, "openai", "new", "https://new.example")
                .unwrap();
            let rejected = runtime
                .submit(move |services| async move {
                    services
                        .executor
                        .execute_transaction(changes)
                        .await
                        .map_err(failure)?;
                    Ok(())
                })
                .unwrap()
                .receive()
                .await;
            assert!(rejected.is_err());
            let rows = runtime.submit(|services| async move {
                services.executor.execute("SELECT id,value_json FROM app_settings WHERE id IN ('current_llm_provider','current_llm_model','ai_provider:llm:openai') ORDER BY id".into(), vec![]).await.map_err(failure)
            }).unwrap().receive().await.unwrap();
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0]["value_json"], json!("old").to_string());
            assert_eq!(rows[1]["value_json"], json!("custom").to_string());
            runtime.shutdown().await.unwrap();
            directory.close().unwrap();
        });
    }

    #[test]
    fn provider_choice_preserves_extra_fields_and_rejects_insecure_credentials() {
        let mut snapshot = Snapshot::default();
        snapshot.values.insert(
            "ai_provider:llm:custom".into(),
            json!({"base_url":"https://old.example","extra":42}).to_string(),
        );
        let statements = snapshot
            .statements(
                ProviderKind::Llm,
                "custom",
                "fixture",
                "https://new.example",
            )
            .unwrap();
        let stored: Value =
            serde_json::from_str(statements[2].params[1].as_str().unwrap()).unwrap();
        assert_eq!(stored["extra"], 42);
        let created = Snapshot::default()
            .provider_statement(ProviderKind::Stt, "deepgram", "")
            .unwrap();
        let created: Value = serde_json::from_str(created.params[1].as_str().unwrap()).unwrap();
        assert_eq!(
            created,
            json!({"type": "stt", "base_url": "", "api_key": ""})
        );
        assert!(
            snapshot
                .statements(
                    ProviderKind::Llm,
                    "custom",
                    "fixture",
                    "http://remote.example"
                )
                .is_err()
        );
        assert!(
            snapshot
                .statements(
                    ProviderKind::Llm,
                    "custom",
                    "fixture",
                    "https://user:password@example.com"
                )
                .is_err()
        );
        assert!(
            snapshot
                .statements(ProviderKind::Llm, "local", "fixture", "")
                .is_err()
        );
    }
}
