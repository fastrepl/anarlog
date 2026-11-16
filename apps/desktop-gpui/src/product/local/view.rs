use std::{collections::BTreeMap, sync::Arc};

use gpui::{Context, EventEmitter, Render, SharedString, Window, div, prelude::*, px};

use crate::{
    contracts::ProductEvent,
    product::services::{
        Mutation, Operation, Outcome, Panel, ProductServices, RequestGate, Scope, Surface,
    },
    ui::theme::theme,
};

pub struct LocalSettingsView {
    services: Arc<dyn ProductServices>,
    surface: Surface,
    scope: Scope,
    gate: RequestGate,
    panel: Option<Panel>,
    fields: BTreeMap<Arc<str>, Arc<str>>,
    error: String,
    loading: bool,
    confirmation: Option<Operation>,
    suspended: bool,
}

impl EventEmitter<ProductEvent> for LocalSettingsView {}
impl EventEmitter<Scope> for LocalSettingsView {}

impl LocalSettingsView {
    pub fn new(
        services: Arc<dyn ProductServices>,
        surface: Surface,
        scope: Scope,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self {
            services,
            surface,
            scope,
            gate: RequestGate::default(),
            panel: None,
            fields: BTreeMap::new(),
            error: String::new(),
            loading: false,
            confirmation: None,
            suspended: false,
        };
        view.reload(cx);
        if surface == Surface::Models {
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_secs(1))
                        .await;
                    if !this
                        .update(cx, |this, cx| {
                            if this.suspended {
                                return false;
                            }
                            if !this.loading && !this.gate.busy() {
                                this.reload(cx);
                            }
                            true
                        })
                        .unwrap_or(false)
                    {
                        break;
                    }
                }
            })
            .detach();
        }
        view
    }

    pub fn has_unsaved(&self) -> bool {
        self.gate.busy()
    }
    pub fn permissions_ready(&self) -> bool {
        self.panel.as_ref().is_some_and(|p| p.permissions_ready)
    }
    pub fn suspend(&mut self) {
        self.suspended = true;
        self.gate.next(self.scope.clone());
    }

    pub fn change_scope(&mut self, scope: Scope, cx: &mut Context<Self>) {
        self.gate.next(scope.clone());
        self.scope = scope;
        self.panel = None;
        self.fields.clear();
        self.confirmation = None;
        self.reload(cx);
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        if self.gate.busy() {
            return;
        }
        let request = self.gate.next(self.scope.clone());
        let worker_request = request.clone();
        let services = self.services.clone();
        let surface = self.surface;
        self.loading = true;
        let worker = cx
            .background_executor()
            .spawn(async move { services.load(surface, worker_request).await });
        cx.spawn(async move |this, cx| {
            let result = worker.await;
            let _ = this.update(cx, |this, cx| {
                if !this.gate.accepts(&request, &this.scope) {
                    return;
                }
                this.loading = false;
                match result.and_then(|panel| {
                    panel.validate(&this.scope)?;
                    Ok(panel)
                }) {
                    Ok(panel) => {
                        for field in panel.fields.iter() {
                            this.fields
                                .entry(field.id.clone())
                                .or_insert_with(|| field.value.clone());
                        }
                        this.panel = Some(panel);
                        this.error.clear();
                    }
                    Err(error) => this.error = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn choose(&mut self, operation: Operation, cx: &mut Context<Self>) {
        if self.gate.busy() || operation.disabled_reason.is_some() {
            return;
        }
        if operation.confirmation.is_some() {
            self.confirmation = Some(operation);
            cx.notify();
        } else {
            self.perform(operation, cx);
        }
    }

    fn perform(&mut self, operation: Operation, cx: &mut Context<Self>) {
        let request = match self.gate.begin_mutation(self.scope.clone()) {
            Ok(request) => request,
            Err(error) => {
                self.error = error.to_string();
                return;
            }
        };
        let worker_request = request.clone();
        let services = self.services.clone();
        let surface = self.surface;
        let fields = self
            .fields
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>()
            .into();
        self.confirmation = None;
        self.error.clear();
        let worker = cx.background_executor().spawn(async move {
            services
                .perform(surface, worker_request, Mutation { operation, fields })
                .await
        });
        cx.spawn(async move |this, cx| {
            let result = worker.await;
            let _ = this.update(cx, |this, cx| {
                if !this.gate.finish(&request, &this.scope) {
                    return;
                }
                match result {
                    Ok(Outcome::OpenSession(id)) => cx.emit(ProductEvent::OpenSession(id)),
                    Ok(Outcome::IdentityChanged(scope)) => {
                        this.scope = scope.clone();
                        this.panel = None;
                        this.fields.clear();
                        cx.emit(scope);
                        this.reload(cx);
                    }
                    Ok(Outcome::Refresh) => this.reload(cx),
                    Err(error) => {
                        this.error = error.to_string();
                        cx.emit(ProductEvent::Failed(error));
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

impl Render for LocalSettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let busy = self.gate.busy();
        let mut view = div()
            .id("local-settings")
            .w_full()
            .max_w(px(720.))
            .flex()
            .flex_col()
            .gap_4()
            .when(!self.error.is_empty(), |view| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(colors.destructive)
                        .child(self.error.clone()),
                )
            })
            .when(self.loading, |view| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(colors.muted_foreground)
                        .child("Loading…"),
                )
            });
        if let Some(panel) = self.panel.clone() {
            for row in panel.rows.iter() {
                let actions: Vec<_> = panel
                    .operations
                    .iter()
                    .filter(|op| op.target_id.as_deref() == Some(row.id.as_ref()))
                    .cloned()
                    .collect();
                let mut line = div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .py_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child(row.title.to_string()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(colors.muted_foreground)
                                    .child(row.detail.to_string()),
                            ),
                    );
                for operation in actions {
                    let enabled = !busy && operation.disabled_reason.is_none();
                    line = line.child(crate::product::controls::outline_button(
                        SharedString::from(operation.id.clone()),
                        operation.label.to_string(),
                        enabled,
                        &colors,
                        Some(Box::new(cx.listener(move |this, _, _, cx| {
                            this.choose(operation.clone(), cx)
                        }))),
                    ));
                }
                view = view.child(line);
            }
            if self.surface == Surface::Exports {
                view = view.child(
                    div().flex().gap_2().children(
                        ["pdf", "txt", "md", "org", "json"]
                            .into_iter()
                            .map(|format| {
                                let selected = self
                                    .fields
                                    .get("format")
                                    .is_some_and(|v| v.as_ref() == format);
                                div()
                                    .id(format)
                                    .px_3()
                                    .py_2()
                                    .rounded(px(8.))
                                    .border_1()
                                    .border_color(colors.border)
                                    .when(selected, |v| v.bg(colors.accent))
                                    .child(format.to_uppercase())
                                    .when(!busy, |v| {
                                        v.cursor_pointer().on_click(cx.listener(
                                            move |this, _, _, cx| {
                                                this.fields.insert("format".into(), format.into());
                                                cx.notify();
                                            },
                                        ))
                                    })
                            }),
                    ),
                );
                for (key, label) in [
                    ("memo", "Include memo"),
                    ("summary", "Include summary"),
                    ("transcript", "Include transcript"),
                ] {
                    let selected = self.fields.get(key).is_some_and(|v| v.as_ref() == "true");
                    view = view.child(
                        div()
                            .id(key)
                            .flex()
                            .gap_2()
                            .text_sm()
                            .child(if selected { "☑" } else { "☐" })
                            .child(label)
                            .when(!busy, |v| {
                                v.cursor_pointer()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.fields.insert(
                                            key.into(),
                                            if selected {
                                                "false".into()
                                            } else {
                                                "true".into()
                                            },
                                        );
                                        cx.notify();
                                    }))
                            }),
                    );
                }
            }
            view = view.child(
                div().flex().flex_wrap().gap_2().children(
                    panel
                        .operations
                        .iter()
                        .filter(|op| {
                            op.target_id.is_none()
                                || !panel
                                    .rows
                                    .iter()
                                    .any(|row| Some(row.id.as_ref()) == op.target_id.as_deref())
                        })
                        .map(|operation| {
                            let operation = operation.clone();
                            let enabled = !busy && operation.disabled_reason.is_none();
                            div()
                                .id(SharedString::from(operation.id.clone()))
                                .px_3()
                                .py_2()
                                .rounded(px(8.))
                                .bg(colors.accent)
                                .text_sm()
                                .child(operation.label.to_string())
                                .when(enabled, |button| {
                                    button.cursor_pointer().on_click(cx.listener(
                                        move |this, _, _, cx| this.choose(operation.clone(), cx),
                                    ))
                                })
                        }),
                ),
            );
        }
        if let Some(operation) = self.confirmation.clone() {
            view = view.child(
                div()
                    .p_4()
                    .border_1()
                    .border_color(colors.border)
                    .rounded(px(8.))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        operation
                            .confirmation
                            .clone()
                            .unwrap_or_default()
                            .to_string(),
                    )
                    .child(
                        div()
                            .id("confirm")
                            .cursor_pointer()
                            .child("Confirm")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.perform(operation.clone(), cx)
                            })),
                    )
                    .child(
                        div()
                            .id("cancel-confirm")
                            .cursor_pointer()
                            .child("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.confirmation = None;
                                cx.notify();
                            })),
                    ),
            );
        }
        view.child(
            div()
                .id("refresh")
                .text_sm()
                .cursor_pointer()
                .child(if busy { "Working…" } else { "Refresh" })
                .when(!busy, |view| {
                    view.on_click(cx.listener(|this, _, _, cx| this.reload(cx)))
                }),
        )
    }
}
