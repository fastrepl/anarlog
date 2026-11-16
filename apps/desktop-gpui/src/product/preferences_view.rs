use std::{collections::BTreeMap, sync::Arc};

use desktop_runtime::{CancellationToken, RuntimeHandle, ServiceError};
use futures::future::{Either, select};
use gpui::{Context, Entity, EventEmitter, Render, Subscription, Window, div, prelude::*, px};

use crate::ui::{
    input::{InputEvent, TextInput},
    theme::theme,
};

use super::{
    navigation,
    settings::{self, Definition, Draft, Setting, Snapshot},
};

pub struct PreferenceField {
    runtime: RuntimeHandle,
    def: &'static Definition,
    draft: Draft,
    input: Entity<TextInput>,
    _subscription: Subscription,
}

impl PreferenceField {
    fn new(
        runtime: RuntimeHandle,
        def: &'static Definition,
        setting: Setting,
        cx: &mut Context<Self>,
    ) -> Self {
        let draft = Draft::new(setting);
        let input = cx.new(|cx| {
            let mut input = TextInput::new(def.kind.clone(), cx);
            if draft.text.len() <= 4096 {
                input.set_text(draft.text.clone(), cx);
            }
            input
        });
        let subscription = cx.subscribe(&input, |this, _, event, cx| {
            match event {
                InputEvent::Changed => {
                    this.draft.text = this.input.read(cx).buffer.text.clone();
                    this.draft.dirty = true;
                    this.draft.error = None;
                }
                InputEvent::Submitted => this.save(cx),
                InputEvent::Rejected => this.draft.error = Some("The native input limit is 4096 bytes. Existing longer preferences remain unchanged.".into()),
            }
            cx.notify();
        });
        Self {
            runtime,
            def,
            draft,
            input,
            _subscription: subscription,
        }
    }

    fn observe(&mut self, setting: Setting, cx: &mut Context<Self>) {
        if self.draft.latest == setting {
            return;
        }
        self.draft.observe(setting);
        if !self.draft.dirty && !self.draft.saving && self.draft.text.len() <= 4096 {
            self.input
                .update(cx, |input, cx| input.set_text(self.draft.text.clone(), cx));
        }
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        if self.draft.saving || !self.draft.dirty {
            return;
        }
        let value = match self.draft.value(self.def) {
            Ok(value) => value,
            Err(error) => {
                self.draft.error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        let reply = settings::save(
            &self.runtime,
            self.def.key.clone(),
            self.draft.base.clone(),
            value,
        );
        let submitted = self.draft.text.clone();
        self.draft.saving = true;
        self.draft.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                this.draft.saving = false;
                match result {
                    Ok(setting) => {
                        this.draft.saved(setting, &submitted);
                    }
                    Err(error) => this.draft.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for PreferenceField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let overlong = self.draft.text.len() > 4096;
        let gate = settings::write_gate(&self.def.key).or(
            matches!(self.def.key.as_str(), "theme" | "app_icon")
                .then_some("Requires the application appearance service."),
        ).or(if overlong {
            Some("The stored value is preserved. Editing preferences longer than 4096 bytes requires the shipping app.")
        } else {
            None
        });
        div()
            .py_3()
            .border_b_1()
            .border_color(colors.border)
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(navigation::label(&self.def.key))
                    .child(
                        div()
                            .text_xs()
                            .text_color(colors.muted_foreground)
                            .child(format!(
                                "{} · {}",
                                self.draft.base.source,
                                if self.def.synced {
                                    "Synced preference"
                                } else {
                                    "This device"
                                }
                            )),
                    ),
            )
            .when_some(gate, |view, reason| {
                view.child(div().text_sm().child(if overlong {
                    format!("{} bytes (preserved)", self.draft.text.len())
                } else {
                    self.draft.text.clone()
                }))
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.muted_foreground)
                        .child(reason),
                )
            })
            .when(gate.is_none(), |view| {
                view.when(self.def.kind == "boolean", |view| {
                    view.child(
                        div()
                            .id("toggle-preference")
                            .px_3()
                            .py_2()
                            .rounded(px(8.))
                            .bg(colors.accent)
                            .child(if self.draft.text == "true" {
                                "Enabled"
                            } else {
                                "Disabled"
                            })
                            .when(!self.draft.saving, |view| {
                                view.cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.draft.text = (this.draft.text != "true").to_string();
                                        this.draft.dirty = true;
                                        this.input.update(cx, |input, cx| {
                                            input.set_text(this.draft.text.clone(), cx)
                                        });
                                        this.save(cx);
                                    }))
                            }),
                    )
                })
                .when(self.def.kind != "boolean", |view| {
                    view.child(self.input.clone())
                })
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .child(
                            div()
                                .id("save")
                                .px_3()
                                .py_1()
                                .rounded(px(6.))
                                .bg(colors.accent)
                                .when(self.draft.dirty && !self.draft.saving, |button| {
                                    button
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| this.save(cx)))
                                })
                                .child(if self.draft.saving {
                                    "Saving…"
                                } else if self.draft.dirty {
                                    "Save preference"
                                } else {
                                    "No changes"
                                }),
                        )
                        .when(self.draft.dirty && !self.draft.saving, |view| {
                            view.child(
                                div()
                                    .id("restore")
                                    .cursor_pointer()
                                    .child("Restore draft")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.draft.restore();
                                        this.input.update(cx, |input, cx| {
                                            input.set_text(this.draft.text.clone(), cx)
                                        });
                                        cx.notify();
                                    })),
                            )
                        }),
                )
            })
            .when_some(self.draft.error.clone(), |view, error| {
                view.child(div().text_sm().text_color(colors.destructive).child(error))
            })
    }
}

pub struct PreferencesView {
    runtime: RuntimeHandle,
    pub section: &'static str,
    snapshot: Arc<Snapshot>,
    fields: BTreeMap<String, Entity<PreferenceField>>,
    cancellation: CancellationToken,
    status: String,
    active: bool,
}

impl EventEmitter<ServiceError> for PreferencesView {}

impl PreferencesView {
    pub fn new(runtime: RuntimeHandle, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            runtime,
            section: "app",
            snapshot: Arc::new(Snapshot::new()),
            fields: BTreeMap::new(),
            cancellation: CancellationToken::new(),
            status: "Loading preferences…".into(),
            active: true,
        };
        this.watch(cx);
        this
    }

    fn watch(&mut self, cx: &mut Context<Self>) {
        self.cancellation.cancel();
        self.cancellation = CancellationToken::new();
        let cancel = self.cancellation.clone();
        let reply = settings::watch(&self.runtime);
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let mut watch = match result {
                Ok(watch) => watch,
                Err(error) => {
                    if !cancel.is_cancelled() {
                        let _ = this.update(cx, |this, cx| this.fail(error, cx));
                    }
                    return;
                }
            };
            loop {
                if cancel.is_cancelled() {
                    break;
                }
                let error = watch
                    .terminal_error()
                    .or_else(|| watch.errors.try_recv().ok());
                if let Some(error) = error {
                    let _ = this.update(cx, |this, cx| this.fail(error, cx));
                    break;
                }
                let rows = watch.snapshots.borrow_and_update().rows.clone();
                let decoded = cx
                    .background_executor()
                    .spawn(async move { settings::decode(&rows).map(Arc::new) })
                    .await;
                if cancel.is_cancelled() {
                    break;
                }
                if this
                    .update(cx, |this, cx| match decoded {
                        Ok(snapshot) => this.apply(snapshot, cx),
                        Err(error) => this.fail(error, cx),
                    })
                    .is_err()
                {
                    break;
                }
                match select(
                    Box::pin(cancel.cancelled()),
                    Box::pin(watch.snapshots.changed()),
                )
                .await
                {
                    Either::Left(_) | Either::Right((Err(_), _)) => break,
                    Either::Right((Ok(()), _)) => {}
                }
            }
            let _ = watch.unsubscribe().await;
        })
        .detach();
    }

    fn fail(&mut self, error: ServiceError, cx: &mut Context<Self>) {
        self.status = format!("Preferences: {error}");
        cx.emit(error);
        cx.notify();
    }

    fn apply(&mut self, snapshot: Arc<Snapshot>, cx: &mut Context<Self>) {
        self.status = "Changes save to the canonical library. Preferences alone do not enable services that have not been integrated.".into();
        if snapshot == self.snapshot {
            cx.notify();
            return;
        }
        for (key, field) in &self.fields {
            if let Some(setting) = snapshot.get(key) {
                field.update(cx, |field, cx| field.observe(setting.clone(), cx));
            }
        }
        self.snapshot = snapshot;
        self.ensure_fields(cx);
        cx.notify();
    }

    fn ensure_fields(&mut self, cx: &mut Context<Self>) {
        for def in settings::definitions()
            .iter()
            .filter(|def| navigation::section(def) == self.section)
        {
            if let Some(setting) = self.snapshot.get(&def.key) {
                self.fields.entry(def.key.clone()).or_insert_with(|| {
                    cx.new(|cx| {
                        PreferenceField::new(self.runtime.clone(), def, setting.clone(), cx)
                    })
                });
            }
        }
    }

    pub fn select(&mut self, section: &'static str, cx: &mut Context<Self>) {
        if !self.active {
            self.active = true;
            self.watch(cx);
        }
        self.section = section;
        self.ensure_fields(cx);
        cx.notify();
    }

    pub fn suspend(&mut self) {
        self.active = false;
        self.cancellation.cancel();
    }

    pub fn has_unsaved(&self, cx: &gpui::App) -> bool {
        self.fields.values().any(|field| {
            let field = field.read(cx);
            field.draft.dirty || field.draft.saving
        })
    }
}

impl Drop for PreferencesView {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

impl Render for PreferencesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .child(self.status.clone()),
            )
            .child(
                div()
                    .id("refresh-preferences")
                    .cursor_pointer()
                    .text_sm()
                    .child("Refresh preferences")
                    .on_click(cx.listener(|this, _, _, cx| this.watch(cx))),
            )
            .children(
                settings::definitions()
                    .iter()
                    .filter(|def| navigation::section(def) == self.section)
                    .filter_map(|def| self.fields.get(&def.key))
                    .cloned(),
            )
    }
}
