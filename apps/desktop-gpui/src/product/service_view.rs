use std::{collections::BTreeMap, sync::Arc, time::Duration};

use desktop_runtime::ServiceError;
use gpui::{
    Context, Entity, EventEmitter, Render, SharedString, Subscription, Window, div, prelude::*, px,
};

use crate::{
    contracts::ProductEvent,
    ui::{
        input::{InputEvent, TextInput},
        theme::theme,
    },
};

use super::services::{
    Mutation, Operation, Outcome, Panel, ProductServices, RequestGate, Scope, Surface,
};

struct FormInput {
    input: Entity<TextInput>,
    dirty: bool,
    _subscription: Subscription,
}

pub struct ServiceView {
    services: Arc<dyn ProductServices>,
    pub surface: Surface,
    scope: Scope,
    gate: RequestGate,
    panel: Option<Arc<Panel>>,
    inputs: BTreeMap<Arc<str>, FormInput>,
    status: String,
    confirmation: Option<Operation>,
    loading: bool,
}

impl EventEmitter<ProductEvent> for ServiceView {}
impl EventEmitter<Scope> for ServiceView {}

impl ServiceView {
    pub fn new(
        services: Arc<dyn ProductServices>,
        surface: Surface,
        scope: Scope,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self {
            services,
            surface,
            scope,
            gate: RequestGate::default(),
            panel: None,
            inputs: BTreeMap::new(),
            status: "Loading…".into(),
            confirmation: None,
            loading: false,
        };
        this.reload(cx);
        this
    }

    pub fn change_scope(&mut self, scope: Scope, cx: &mut Context<Self>) {
        self.gate.next(scope.clone());
        self.scope = scope.clone();
        self.panel = None;
        self.inputs.clear();
        self.confirmation = None;
        cx.emit(scope);
        self.reload(cx);
    }

    pub fn has_unsaved(&self) -> bool {
        self.gate.busy() || self.inputs.values().any(|input| input.dirty)
    }

    pub fn permissions_ready(&self) -> bool {
        self.panel
            .as_ref()
            .is_some_and(|panel| panel.permissions_ready)
    }

    pub fn suspend(&mut self) {
        self.gate.next(self.scope.clone());
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        if self.gate.busy() {
            return;
        }
        let request = self.gate.next(self.scope.clone());
        let request_for_worker = request.clone();
        let services = self.services.clone();
        let surface = self.surface;
        let worker = cx
            .background_executor()
            .spawn(async move { services.load(surface, request_for_worker).await });
        self.loading = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = worker.await;
            let should_poll = this
                .update(cx, |this, cx| {
                    if !this.gate.accepts(&request, &this.scope) {
                        return false;
                    }
                    this.loading = false;
                    match result.and_then(|panel| panel.validate(&this.scope).map(|()| panel)) {
                        Ok(panel) => {
                            this.apply(panel, cx);
                            true
                        }
                        Err(error) => {
                            this.status = error.to_string();
                            this.panel = None;
                            cx.notify();
                            false
                        }
                    }
                })
                .unwrap_or(false);
            let interval = match surface {
                Surface::Permissions | Surface::Models => Some(Duration::from_secs(1)),
                Surface::CloudSync => Some(Duration::from_secs(10)),
                Surface::Imports | Surface::Calendar => Some(Duration::from_secs(2)),
                _ => None,
            };
            if should_poll && let Some(interval) = interval {
                cx.background_executor().timer(interval).await;
                let _ = this.update(cx, |this, cx| {
                    if this.gate.accepts(&request, &this.scope) && !this.gate.busy() {
                        this.reload(cx);
                    }
                });
            }
        })
        .detach();
    }

    fn apply(&mut self, panel: Panel, cx: &mut Context<Self>) {
        if self
            .inputs
            .iter()
            .any(|(id, input)| input.dirty && !panel.fields.iter().any(|field| &field.id == id))
        {
            self.panel = None;
            self.confirmation = None;
            self.status = "The service changed this form. Your edits are retained; discard them explicitly before loading the new form.".into();
            cx.notify();
            return;
        }
        for field in panel.fields.iter() {
            if let Some(input) = self.inputs.get_mut(&field.id) {
                if !input.dirty && input.input.read(cx).buffer.text.as_str() != field.value.as_ref()
                {
                    input
                        .input
                        .update(cx, |input, cx| input.set_text(field.value.to_string(), cx));
                }
            } else {
                let input = cx.new(|cx| {
                    let mut input = TextInput::new(field.label.to_string(), cx);
                    input.set_text(field.value.to_string(), cx);
                    input
                });
                let id = field.id.clone();
                let subscription = cx.subscribe(&input, move |this, _, event, cx| {
                    match event {
                        InputEvent::Changed => {
                            if let Some(input) = this.inputs.get_mut(&id) {
                                input.dirty = true;
                            }
                        }
                        InputEvent::Rejected => this.status = "Input exceeds 4096 bytes.".into(),
                        InputEvent::Submitted => {}
                    }
                    cx.notify();
                });
                self.inputs.insert(
                    field.id.clone(),
                    FormInput {
                        input,
                        dirty: false,
                        _subscription: subscription,
                    },
                );
            }
        }
        self.inputs
            .retain(|id, _| panel.fields.iter().any(|field| &field.id == id));
        self.status = panel.status.to_string();
        self.panel = Some(Arc::new(panel));
        cx.notify();
    }

    fn choose(&mut self, operation: Operation, cx: &mut Context<Self>) {
        if self.gate.busy() || operation.disabled_reason.is_some() {
            return;
        }
        if operation.action.requires_confirmation() {
            self.confirmation = Some(operation);
            cx.notify();
        } else {
            self.perform(operation, cx);
        }
    }

    fn perform(&mut self, operation: Operation, cx: &mut Context<Self>) {
        let Some(panel) = &self.panel else {
            return;
        };
        if !panel.operations.iter().any(|candidate| {
            candidate.id == operation.id
                && candidate.action == operation.action
                && candidate.target_id == operation.target_id
                && candidate.disabled_reason.is_none()
        }) {
            self.status = ServiceError::Conflict.to_string();
            cx.notify();
            return;
        }
        let request = match self.gate.begin_mutation(self.scope.clone()) {
            Ok(request) => request,
            Err(_) => return,
        };
        let fields: Arc<[(Arc<str>, Arc<str>)]> = self
            .inputs
            .iter()
            .map(|(id, input)| {
                (
                    id.clone(),
                    Arc::from(input.input.read(cx).buffer.text.as_str()),
                )
            })
            .collect();
        let submitted = fields.clone();
        let clears_identity = operation.action.clears_identity();
        if clears_identity {
            self.panel = None;
            self.inputs.clear();
            self.scope = Scope::default();
            cx.emit(Scope::default());
        }
        self.confirmation = None;
        self.status = format!("{}…", operation.label);
        let services = self.services.clone();
        let surface = self.surface;
        let worker_request = request.clone();
        let worker = cx.background_executor().spawn(async move {
            services
                .perform(surface, worker_request, Mutation { operation, fields })
                .await
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = worker.await;
            let _ = this.update(cx, |this, cx| {
                let completion_scope = if clears_identity && this.scope == Scope::default() {
                    &request.scope
                } else {
                    &this.scope
                };
                if !this.gate.finish(&request, completion_scope) {
                    return;
                }
                match result {
                    Ok(outcome) => {
                        for (id, value) in submitted.iter() {
                            if let Some(input) = this.inputs.get_mut(id)
                                && input.input.read(cx).buffer.text.as_str() == value.as_ref()
                            {
                                input.dirty = false;
                            }
                        }
                        match outcome {
                            Outcome::Refresh => this.reload(cx),
                            Outcome::OpenSession(id) => cx.emit(ProductEvent::OpenSession(id)),
                            Outcome::IdentityChanged(scope) => {
                                if clears_identity && scope.account_id.is_some() {
                                    this.status = "Identity-clearing operation returned an authenticated identity.".into();
                                    cx.emit(ProductEvent::Failed(ServiceError::Conflict));
                                } else {
                                    this.change_scope(scope, cx);
                                }
                            }
                        }
                    }
                    Err(error) => {
                        this.status = error.to_string();
                        cx.emit(ProductEvent::Failed(error));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for ServiceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let busy = self.gate.busy();
        let mut view = div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .child(self.status.clone()),
            )
            .child(
                div()
                    .id("refresh-service")
                    .child(if self.loading {
                        "Refreshing…"
                    } else {
                        "Refresh"
                    })
                    .when(!busy && !self.loading, |view| {
                        view.cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| this.reload(cx)))
                    }),
            );
        if let Some(panel) = self.panel.clone() {
            view = view.children(panel.rows.iter().map(|row| {
                div()
                    .id(SharedString::from(row.id.clone()))
                    .p_3()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(colors.border)
                    .child(div().child(row.title.to_string()))
                    .child(
                        div()
                            .text_sm()
                            .text_color(colors.muted_foreground)
                            .child(row.detail.to_string()),
                    )
            }));
            for field in panel.fields.iter() {
                if let Some(input) = self.inputs.get(&field.id) {
                    view = view.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(field.label.to_string())
                            .child(input.input.clone()),
                    );
                }
            }
            view = view.child(div().flex().flex_wrap().gap_3().children(
                panel.operations.iter().map(|operation| {
                    let label = operation.label.to_string();
                    let reason = operation.disabled_reason.clone();
                    div()
                        .id(SharedString::from(operation.id.clone()))
                        .px_3()
                        .py_2()
                        .rounded(px(8.))
                        .bg(colors.accent)
                        .child(label)
                        .when_some(reason.clone(), |view, reason| {
                            view.child(div().text_xs().child(reason.to_string()))
                        })
                        .when(!busy && reason.is_none(), |view| {
                            let operation = operation.clone();
                            view.cursor_pointer().on_click(
                                cx.listener(move |this, _, _, cx| {
                                    this.choose(operation.clone(), cx)
                                }),
                            )
                        })
                }),
            ));
        }
        if self.inputs.values().any(|input| input.dirty) && !busy {
            view = view.child(
                div()
                    .id("discard-form-edits")
                    .cursor_pointer()
                    .child("Discard form edits")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.inputs.clear();
                        this.confirmation = None;
                        this.reload(cx);
                    })),
            );
        }
        if busy {
            view = view.child(
                div()
                    .id("cancel-pending-operation")
                    .cursor_pointer()
                    .child("Cancel operation")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.gate.next(this.scope.clone());
                        this.status =
                            "Cancellation requested. Refresh to confirm the persisted result."
                                .into();
                        cx.notify();
                    })),
            );
        }
        if let Some(operation) = self.confirmation.clone() {
            view = view.child(
                div()
                    .p_4()
                    .border_1()
                    .border_color(colors.destructive)
                    .rounded(px(8.))
                    .child(
                        operation
                            .confirmation
                            .as_ref()
                            .map(|text| text.to_string())
                            .unwrap_or_default(),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_4()
                            .child(
                                div()
                                    .id("confirm-operation")
                                    .cursor_pointer()
                                    .child("Confirm")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.perform(operation.clone(), cx)
                                    })),
                            )
                            .child(
                                div()
                                    .id("cancel-operation")
                                    .cursor_pointer()
                                    .child("Cancel")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.confirmation = None;
                                        cx.notify();
                                    })),
                            ),
                    ),
            );
        }
        view
    }
}
