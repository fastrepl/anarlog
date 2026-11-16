use std::collections::BTreeMap;

use anlg_db_execute::TransactionStatement;
use desktop_runtime::{Result, ServiceError};
use gpui::{Context, Entity, Render, Subscription, Window, div, prelude::*};
use serde_json::{Value, json};

use crate::{
    meeting::config::{ProviderKind, ProviderServices},
    ui::{
        input::{InputEvent, TextInput},
        theme::theme,
    },
};

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
        let key = format!("ai_provider:{}:{provider}", kind.key());
        let mut config: Value = self
            .values
            .get(&key)
            .map(|value| serde_json::from_str(value))
            .transpose()
            .map_err(failure)?
            .unwrap_or_else(|| json!({}));
        let object = config.as_object_mut().ok_or_else(|| {
            failure("Stored provider configuration is malformed; it was preserved.")
        })?;
        object.insert("base_url".into(), json!(base));
        Ok([
            (format!("current_{}_provider", kind.key()), json!(provider)),
            (format!("current_{}_model", kind.key()), json!(model.trim())),
            (key, config),
        ].into_iter().map(|(key, value)| TransactionStatement {
            sql: "INSERT INTO app_settings (id,value_json,updated_at) SELECT ?1,?2,strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE (SELECT value_json FROM app_settings WHERE id=?1) IS ?3 ON CONFLICT(id) DO UPDATE SET value_json=excluded.value_json,updated_at=excluded.updated_at".into(),
            params: vec![json!(key), json!(value.to_string()), json!(self.values.get(&key))],
            expected_rows_affected: Some(1),
        }).collect())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Operation {
    Loading,
    Saving,
}

pub struct ProviderView {
    services: ProviderServices,
    kind: ProviderKind,
    snapshot: Option<Snapshot>,
    provider: Entity<TextInput>,
    model: Entity<TextInput>,
    base: Entity<TextInput>,
    secret: Entity<TextInput>,
    dirty: bool,
    generation: u64,
    operation: Option<Operation>,
    status: String,
    _subscriptions: Vec<Subscription>,
}

impl ProviderView {
    pub fn select_kind(&mut self, kind: ProviderKind, cx: &mut Context<Self>) {
        if self.kind != kind && !self.has_unsaved(cx) {
            self.kind = kind;
            self.restore(cx);
        }
    }

    pub fn new(services: ProviderServices, cx: &mut Context<Self>) -> Self {
        let provider =
            cx.new(|cx| TextInput::new("Provider ID: anarlog, openai, deepgram, custom…", cx));
        let model = cx.new(|cx| TextInput::new("Model name", cx));
        let base = cx.new(|cx| TextInput::new("Base URL (blank uses provider default)", cx));
        let secret =
            cx.new(|cx| TextInput::new("New API key (stored in the system keyring)", cx).secret());
        let subscriptions = [&provider, &model, &base, &secret]
            .into_iter()
            .map(|input| {
                cx.subscribe(input, |this, _, event, cx| {
                    if matches!(event, InputEvent::Changed) {
                        this.dirty = true;
                        this.generation = this.generation.wrapping_add(1);
                        cx.notify();
                    }
                })
            })
            .collect();
        let mut view = Self {
            services,
            kind: ProviderKind::Stt,
            snapshot: None,
            provider,
            model,
            base,
            secret,
            dirty: false,
            generation: 0,
            operation: None,
            status: String::new(),
            _subscriptions: subscriptions,
        };
        view.reload(true, cx);
        view
    }

    pub fn has_unsaved(&self, cx: &gpui::App) -> bool {
        self.dirty
            || self.operation == Some(Operation::Saving)
            || [&self.provider, &self.model, &self.base, &self.secret]
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
            let rows = services.executor.execute("SELECT id,value_json FROM app_settings WHERE (id LIKE 'ai_provider:%' OR id IN ('current_stt_provider','current_stt_model','current_llm_provider','current_llm_model')) AND length(CAST(value_json AS BLOB)) <= 524288 LIMIT 256".into(), vec![]).await.map_err(failure)?;
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

    fn restore(&mut self, cx: &mut Context<Self>) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let provider = snapshot.choice(&format!("current_{}_provider", self.kind.key()));
        let model = snapshot.choice(&format!("current_{}_model", self.kind.key()));
        let base = snapshot
            .parsed
            .get(&format!("ai_provider:{}:{provider}", self.kind.key()))
            .and_then(|value| value["base_url"].as_str())
            .unwrap_or_default()
            .to_owned();
        self.provider
            .update(cx, |input, cx| input.set_text(provider, cx));
        self.model.update(cx, |input, cx| input.set_text(model, cx));
        self.base.update(cx, |input, cx| input.set_text(base, cx));
        self.secret
            .update(cx, |input, cx| input.set_text(String::new(), cx));
        self.dirty = false;
    }

    fn save(&mut self, key_only: bool, cx: &mut Context<Self>) {
        if self.operation.is_some()
            || [&self.provider, &self.model, &self.base, &self.secret]
                .into_iter()
                .any(|input| input.read(cx).buffer.marked.is_some())
        {
            return;
        }
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let provider = self.provider.read(cx).buffer.text.trim().to_owned();
        let model = self.model.read(cx).buffer.text.clone();
        let base = self.base.read(cx).buffer.text.trim().to_owned();
        let snapshot = snapshot.clone();
        let secret = self.secret.read(cx).buffer.text.clone();
        if key_only && (secret.is_empty() || !secret.is_ascii()) {
            self.status = "Enter an ASCII API key to save to the keyring.".into();
            cx.notify();
            return;
        }
        if !key_only && !secret.is_empty() {
            self.status = "Save the API key before saving the provider choice.".into();
            cx.notify();
            return;
        }
        let writer = self.services.secret_write.clone();
        let kind = self.kind;
        let reply = if key_only {
            self.services.runtime.service(move |_| async move {
                snapshot.statements(kind, &provider, &model, &base)?;
                writer(kind, provider, secret).await
            })
        } else {
            self.services.runtime.submit(move |services| async move {
                let statements = snapshot.statements(kind, &provider, &model, &base)?;
                services
                    .executor
                    .execute_transaction(statements)
                    .await
                    .map_err(failure)?;
                Ok(())
            })
        };
        self.operation = Some(Operation::Saving);
        let generation = self.generation;
        cx.spawn(async move |this, cx| {
            let result = match reply { Ok(reply) => reply.receive().await, Err(error) => Err(error) };
            let _ = this.update(cx, |this, cx| {
                this.operation = None;
                match result {
                    Ok(()) if key_only => {
                        if this.generation == generation {
                            this.secret.update(cx, |input, cx| input.set_text(String::new(), cx));
                        }
                        this.status = "API key saved securely. Save provider choice separately.".into();
                    }
                    Ok(()) => {
                        this.status = "Provider choice saved. Credentials are checked when the provider is used.".into();
                        if this.generation == generation { this.dirty = false; }
                        this.reload(false, cx);
                    }
                    Err(error) => this.status = format!("{error}. Draft retained; reload to resolve a concurrent change."),
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
}

impl Render for ProviderView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let kinds = [
            ("Transcription", ProviderKind::Stt),
            ("Intelligence", ProviderKind::Llm),
        ]
        .into_iter()
        .map(|(label, kind)| {
            div()
                .id(label)
                .child(label)
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    if this.has_unsaved(cx) {
                        this.status =
                            "Save or restore edits before switching provider kind.".into();
                    } else {
                        this.kind = kind;
                        this.restore(cx);
                    }
                    cx.notify();
                }))
        });
        div().flex().flex_col().gap_2()
            .child("Provider connection")
            .child(div().flex().gap_3().children(kinds))
            .child(match self.kind { ProviderKind::Stt => "Editing transcription provider", ProviderKind::Llm => "Editing intelligence provider" })
            .child(self.provider.clone()).child(self.model.clone()).child(self.base.clone()).child(self.secret.clone())
            .child(div().text_xs().text_color(colors.muted_foreground).child("Anarlog uses account sign-in. Other providers use the system keyring. Start local models through Manage local models."))
            .child(self.status.clone())
            .child(div().flex().gap_3().when(self.operation.is_none(), |view| view
                .child(div().id("provider-choice").cursor_pointer().child("Save provider choice").on_click(cx.listener(|this, _, _, cx| this.save(false, cx))))
                .child(div().id("provider-key").cursor_pointer().child("Save API key").on_click(cx.listener(|this, _, _, cx| this.save(true, cx))))
                .child(div().id("provider-restore").cursor_pointer().child("Discard and reload").on_click(cx.listener(|this, _, _, cx| this.reload(true, cx))))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
