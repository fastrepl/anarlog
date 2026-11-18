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
            || model.trim().is_empty()
            || model.len() > 512
        {
            return Err(failure("Enter a provider ID and model name."));
        }
        if matches!(provider, "local" | "local_file") {
            return Err(failure("Start local models using Manage local models."));
        }
        let key = format!("ai_provider:{}:{provider}", kind.key());
        let config = self.provider_config(kind, &key, base)?;
        Ok([
            (format!("current_{}_provider", kind.key()), json!(provider)),
            (format!("current_{}_model", kind.key()), json!(model.trim())),
            (key, config),
        ]
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
        TransactionStatement {
            sql: "INSERT INTO app_settings (id,value_json,updated_at) SELECT ?1,?2,strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE (SELECT value_json FROM app_settings WHERE id=?1) IS ?3 ON CONFLICT(id) DO UPDATE SET value_json=excluded.value_json,updated_at=excluded.updated_at".into(),
            params: vec![json!(key), json!(value.to_string()), json!(self.values.get(&key))],
            expected_rows_affected: Some(1),
        }
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

/// Providers the native app can select as the active one. On-device engines start from
/// Manage local models, and subscriptions need a sign-in flow the native app lacks.
fn selectable(kind: ProviderKind, provider: &ProviderInfo) -> bool {
    !provider.subscription
        && !(kind == ProviderKind::Stt && provider.built_in && provider.id != "anarlog")
        && !(kind == ProviderKind::Llm && provider.id == "apple_foundation")
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
    /// A provider picked in the menu whose model has not been saved yet.
    draft: Option<&'static str>,
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
    pub fn select_kind(&mut self, kind: ProviderKind, cx: &mut Context<Self>) {
        if self.kind != kind && !self.has_unsaved(cx) {
            self.kind = kind;
            self.expanded = None;
            self.menu = false;
            self.restore(cx);
        }
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
            draft: None,
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
            || self.draft.is_some()
            || self.operation == Some(Operation::Saving)
            || [&self.model, &self.base, &self.secret]
                .into_iter()
                .any(|input| input.read(cx).buffer.marked.is_some())
    }

    fn reload(&mut self, restore: bool, cx: &mut Context<Self>) {
        if self.operation.is_some() {
            return;
        }
        self.operation = Some(Operation::Loading);
        let generation = self.generation;
        let reply = self.services.runtime.read(desktop_runtime::CancellationToken::new(), |services| async move {
            let rows = services.executor.execute("SELECT id,value_json FROM app_settings WHERE (id LIKE 'ai_provider:%' OR id IN ('current_stt_provider','current_stt_model','current_llm_provider','current_llm_model','current_llm_reasoning_effort')) AND length(CAST(value_json AS BLOB)) <= 524288 LIMIT 256".into(), vec![]).await.map_err(failure)?;
            let values: BTreeMap<String, String> = rows.iter().map(|row| Ok((row["id"].as_str().ok_or_else(|| failure("Invalid setting ID"))?.to_owned(), row["value_json"].as_str().ok_or_else(|| failure("Invalid setting value"))?.to_owned()))).collect::<Result<_>>()?;
            let parsed = values.iter().map(|(key, value)| Ok((key.clone(), serde_json::from_str(value).map_err(failure)?))).collect::<Result<_>>()?;
            Ok(Snapshot { values, parsed })
        });
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                this.operation = None;
                match result {
                    Ok(snapshot) => {
                        this.snapshot = Some(snapshot);
                        if restore && this.generation == generation {
                            this.restore(cx);
                        }
                    }
                    Err(error) => this.status = error.to_string(),
                }
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
        let model = self.current("model");
        self.draft = None;
        self.model.update(cx, |input, cx| input.set_text(model, cx));
        self.restore_card(cx);
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

    fn pick_provider(&mut self, provider: &'static str, cx: &mut Context<Self>) {
        self.menu = false;
        if provider == self.current("provider") {
            self.restore(cx);
        } else {
            self.draft = Some(provider);
            self.model
                .update(cx, |input, cx| input.set_text(String::new(), cx));
            self.status = "Enter a model and press Return to use this provider.".into();
        }
        cx.notify();
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

    /// Saves the active provider and model ("Model being used").
    fn save_choice(&mut self, cx: &mut Context<Self>) {
        let Some(snapshot) = self.snapshot.clone().filter(|_| !self.busy(cx)) else {
            return;
        };
        let provider = self.provider();
        let model = self.model.read(cx).buffer.text.clone();
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
                                Saved::Choice => {
                                    this.dirty_choice = false;
                                    this.draft = None;
                                }
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
                        )
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn provider_menu(&self, colors: &Theme, cx: &mut Context<Self>) -> gpui::Stateful<Div> {
        let current = self.provider();
        controls::menu_panel(280., colors).child(
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
                            let enabled = selectable(self.kind, info) && !locked;
                            let id = info.id;
                            controls::menu_item(
                                SharedString::from(format!("provider-option-{id}")),
                                info.name,
                                if locked {
                                    Some("Upgrade to Pro to use this provider.".into())
                                } else {
                                    info.badge.map(Into::into)
                                },
                                id == current,
                                colors,
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
    fn model_select(&self, provider: &str, colors: &Theme, cx: &mut Context<Self>) -> Div {
        let models = match self.kind {
            ProviderKind::Stt => provider_catalog::stt_models(provider),
            ProviderKind::Llm => &[],
        };
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
                pill(colors)
                    .when(provider.is_empty(), |view| view.opacity(0.5))
                    .child(
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
        let selected = models.iter().find(|model| **model == current).copied();
        field
            .child(
                pill(colors)
                    .id("model-select")
                    .role(gpui::Role::ComboBox)
                    .aria_label("Model")
                    .aria_value(selected.unwrap_or("Select a model"))
                    .aria_expanded(self.model_menu)
                    .justify_between()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.model_menu = !this.model_menu;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .when(selected.is_none(), |view| {
                                view.text_color(colors.muted_foreground)
                            })
                            .child(selected.unwrap_or("Select a model")),
                    )
                    .child(controls::icon(
                        "ArrowDown01Icon.svg",
                        16.,
                        colors.foreground.opacity(0.5),
                    )),
            )
            .when(self.model_menu, |view| {
                view.child(controls::dropdown(
                    controls::menu_panel(controls::CONTROL_WIDTH, colors).children(
                        models.iter().map(|model| {
                            let model = *model;
                            controls::menu_item(
                                SharedString::from(format!("model-{model}")),
                                model,
                                None,
                                Some(model) == selected,
                                colors,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.model_menu = false;
                                    this.model
                                        .update(cx, |input, cx| input.set_text(model.into(), cx));
                                    this.save_choice(cx);
                                    cx.notify();
                                },
                            ))
                        }),
                    ),
                    true,
                ))
            })
    }
}

impl ProviderView {
    /// `Reasoning effort` (`settings/ai/llm/select.tsx`), stored for providers that take one.
    fn reasoning_effort(&self, colors: &Theme, cx: &mut Context<Self>) -> gpui::Stateful<Div> {
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
                .when(self.effort_menu, |view| {
                    view.child(controls::dropdown(
                        controls::menu_panel(controls::CONTROL_WIDTH, colors).children(
                            EFFORTS.into_iter().map(|(value, label)| {
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
                            }),
                        ),
                        false,
                    ))
                }),
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
                                    .when(self.menu, |view| {
                                        view.child(controls::dropdown(
                                            self.provider_menu(&colors, cx),
                                            false,
                                        ))
                                    }),
                            )
                            .child(div().text_color(colors.muted_foreground).child("/"))
                            .child(self.model_select(&provider, &colors, cx)),
                    )
                    .when(
                        self.kind == ProviderKind::Llm
                            && !provider.is_empty()
                            && !matches!(provider.as_str(), "anarlog" | "apple_foundation"),
                        |view| view.child(self.reasoning_effort(&colors, cx)),
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

    #[gpui::test]
    fn saving_the_model_keeps_an_api_key_typed_but_not_saved(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = desktop_runtime::RuntimeHandle::start(desktop_runtime::Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        futures::executor::block_on(ready.receive()).unwrap();
        let services = ProviderServices {
            runtime: runtime.clone(),
            api_url: String::new(),
            cloud: Arc::new(|| Box::pin(async { Err(ServiceError::Closed) })),
            local: Arc::new(|_| Box::pin(async { Err(ServiceError::Closed) })),
            secret: Arc::new(|_, _| Box::pin(async { Ok(None) })),
            secret_write: Arc::new(|_, _, _| Box::pin(async { Ok(()) })),
        };
        let view = cx.new(|cx| ProviderView::new(services, cx));
        let settle = |cx: &mut TestAppContext| {
            for _ in 0..400 {
                cx.run_until_parked();
                if view.read_with(cx, |view, _| {
                    view.snapshot.is_some() && view.operation.is_none()
                }) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            panic!("provider view did not settle");
        };
        settle(cx);
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
        settle(cx);
        view.read_with(cx, |view, cx| {
            assert_eq!(view.secret.read(cx).buffer.text, "typed-key");
            assert!(view.has_unsaved(cx));
        });
        futures::executor::block_on(runtime.shutdown()).unwrap();
    }

    #[test]
    fn stale_provider_choice_rolls_back_the_whole_transaction() {
        futures::executor::block_on(async {
            let directory = std::path::PathBuf::from(std::env::var_os("HOME").unwrap())
                .join(".cache/anarlog-provider-tests")
                .join(uuid::Uuid::new_v4().to_string());
            std::fs::create_dir_all(&directory).unwrap();
            let (runtime, ready) =
                desktop_runtime::RuntimeHandle::start(desktop_runtime::Profile {
                    database: directory.join("library.sqlite"),
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
            std::fs::remove_dir_all(directory).unwrap();
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
