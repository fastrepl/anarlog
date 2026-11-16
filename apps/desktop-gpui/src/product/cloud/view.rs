use std::{collections::BTreeMap, sync::Arc};

use desktop_runtime::Result;
use futures::future::BoxFuture;
use gpui::{
    Context, Entity, EventEmitter, Render, SharedString, Subscription, Task, Window, div,
    prelude::*, px, uniform_list,
};

use super::{
    CloudServices, ConflictDecision, ConflictReview, ManagementCommand, SecureAuthProvider,
};
use crate::{
    contracts::ProductEvent,
    product::services::{
        Action, Mutation, Operation, Outcome, Panel, ProductServices, RequestGate, Scope, Surface,
    },
    ui::{
        input::{InputEvent, TextInput},
        theme::theme,
    },
};

pub struct CloudView {
    cloud: CloudServices,
    surface: Surface,
    scope: Scope,
    gate: RequestGate,
    panel: Option<Panel>,
    inputs: BTreeMap<Arc<str>, Entity<TextInput>>,
    subscriptions: Vec<Subscription>,
    identities: Option<Task<()>>,
    dialog: Option<Operation>,
    management: Option<&'static str>,
    selected: Option<(String, String)>,
    conflict: Option<ConflictReview>,
    status: String,
    dirty: bool,
    loading: bool,
    page: usize,
}

impl EventEmitter<Scope> for CloudView {}
impl EventEmitter<ProductEvent> for CloudView {}

impl CloudView {
    pub fn new(
        cloud: CloudServices,
        surface: Surface,
        scope: Scope,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut identities = cloud.identities();
        let mut this = Self {
            cloud,
            surface,
            scope,
            gate: RequestGate::default(),
            panel: None,
            inputs: BTreeMap::new(),
            subscriptions: Vec::new(),
            identities: None,
            dialog: None,
            management: None,
            selected: None,
            conflict: None,
            status: String::new(),
            dirty: false,
            loading: false,
            page: 0,
        };
        this.identities = Some(cx.spawn(async move |this, cx| {
            while identities.changed().await.is_ok() {
                let identity = identities.borrow_and_update().clone();
                if this
                    .update(cx, |this, cx| {
                        if this.scope.account_id == identity.account_id {
                            this.reload(cx);
                        } else {
                            this.change_scope(
                                Scope {
                                    account_id: identity.account_id,
                                    ..Scope::default()
                                },
                                cx,
                            );
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
        this.reload(cx);
        this
    }

    pub fn has_unsaved(&self) -> bool {
        self.dirty || self.gate.busy()
    }

    pub fn suspend(&mut self) {
        self.gate.next(self.scope.clone());
    }

    pub fn change_scope(&mut self, scope: Scope, cx: &mut Context<Self>) {
        if self.has_unsaved() && self.scope.account_id == scope.account_id {
            self.status = "Finish or cancel the current change before switching workspaces".into();
            cx.notify();
            return;
        }
        self.suspend();
        self.scope = scope.clone();
        self.panel = None;
        self.inputs.clear();
        self.subscriptions.clear();
        self.dialog = None;
        self.management = None;
        self.conflict = None;
        self.selected = None;
        self.page = 0;
        self.dirty = false;
        cx.emit(scope);
        self.reload(cx);
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        if self.gate.busy() {
            return;
        }
        let request = self.gate.next(self.scope.clone());
        let load = self
            .cloud
            .load_page(self.surface, request.clone(), self.page);
        let worker = cx.background_executor().spawn(load);
        self.loading = true;
        cx.spawn(async move |this, cx| {
            let result = worker.await;
            let _ = this.update(cx, |this, cx| {
                if !this.gate.accepts(&request, &this.scope) {
                    return;
                }
                this.loading = false;
                match result.and_then(|panel| panel.validate(&this.scope).map(|()| panel)) {
                    Ok(panel) => {
                        if !this.dirty {
                            this.inputs.clear();
                            this.subscriptions.clear();
                            for field in panel.fields.iter().filter(|field| {
                                !matches!(field.id.as_ref(), "target" | "recovery_key")
                            }) {
                                let input = cx.new(|cx| {
                                    let mut input = TextInput::new(field.label.to_string(), cx);
                                    input.set_text(field.value.to_string(), cx);
                                    input
                                });
                                this.subscriptions.push(cx.subscribe(
                                    &input,
                                    |this, _, event, cx| {
                                        if matches!(event, InputEvent::Changed) {
                                            this.dirty = true;
                                        }
                                        cx.notify();
                                    },
                                ));
                                this.inputs.insert(field.id.clone(), input);
                            }
                        }
                        this.status = panel.status.to_string();
                        this.panel = Some(panel);
                    }
                    Err(error) => this.status = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn choose(&mut self, mut operation: Operation, cx: &mut Context<Self>) {
        if self.gate.busy() {
            return;
        }
        operation.target_id = self.selected.as_ref().map(|(_, id)| Arc::from(id.as_str()));
        self.management = None;
        self.dialog = Some(operation);
        cx.notify();
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let Some(operation) = self.dialog.clone() else {
            return;
        };
        let Ok(request) = self.gate.begin_mutation(self.scope.clone()) else {
            return;
        };
        let future = if let Some(kind) = self.management {
            let command = match kind {
                "rename" => ManagementCommand::RenameWorkspace(
                    self.inputs
                        .get("name")
                        .map(|input| input.read(cx).buffer.text.clone())
                        .unwrap_or_default(),
                ),
                "remove" => ManagementCommand::RemoveMember(
                    operation
                        .target_id
                        .as_deref()
                        .unwrap_or_default()
                        .to_string(),
                ),
                _ => ManagementCommand::RevokeShareAccess(
                    operation
                        .target_id
                        .as_deref()
                        .unwrap_or_default()
                        .to_string(),
                ),
            };
            let future = self.cloud.manage(self.scope.clone(), command);
            Box::pin(async move { future.await.map(|()| Outcome::Refresh) })
                as BoxFuture<'static, Result<Outcome>>
        } else if operation.action == Action::ImportRecoveryKey {
            self.cloud.import_recovery_file(request.clone())
        } else {
            let fields = self
                .inputs
                .iter()
                .map(|(id, input)| (id.clone(), Arc::from(input.read(cx).buffer.text.as_str())))
                .collect::<Vec<_>>();
            self.cloud.perform(
                self.surface,
                request.clone(),
                Mutation {
                    operation,
                    fields: fields.into(),
                },
            )
        };
        self.complete(request, future, cx);
    }

    fn complete(
        &mut self,
        request: crate::product::services::Request,
        future: BoxFuture<'static, Result<Outcome>>,
        cx: &mut Context<Self>,
    ) {
        let worker = cx.background_executor().spawn(future);
        cx.spawn(async move |this, cx| {
            let result = worker.await;
            let _ = this.update(cx, |this, cx| {
                if !this.gate.finish(&request, &this.scope) {
                    return;
                }
                match result {
                    Ok(outcome) => {
                        this.dialog = None;
                        this.dirty = false;
                        this.conflict = None;
                        match outcome {
                            Outcome::IdentityChanged(scope) => this.change_scope(scope, cx),
                            Outcome::OpenSession(id) => cx.emit(ProductEvent::OpenSession(id)),
                            Outcome::Refresh => this.reload(cx),
                        }
                    }
                    Err(error) => this.status = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn review(&mut self, cx: &mut Context<Self>) {
        if self.gate.busy() {
            return;
        }
        let request = self.gate.next(self.scope.clone());
        let future = self.cloud.conflict_review(self.scope.clone());
        let worker = cx.background_executor().spawn(future);
        cx.spawn(async move |this, cx| {
            let result = worker.await;
            let _ = this.update(cx, |this, cx| {
                if !this.gate.accepts(&request, &this.scope) {
                    return;
                }
                match result {
                    Ok(review) => this.conflict = review,
                    Err(error) => this.status = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn resolve(&mut self, decision: ConflictDecision, cx: &mut Context<Self>) {
        let Some(review) = &self.conflict else {
            return;
        };
        let revision = review.remote_revision;
        let Ok(request) = self.gate.begin_mutation(self.scope.clone()) else {
            return;
        };
        let future = self
            .cloud
            .resolve_conflict(self.scope.clone(), revision, decision);
        self.complete(
            request,
            Box::pin(async move { future.await.map(|()| Outcome::Refresh) }),
            cx,
        );
    }
}

impl Render for CloudView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let busy = self.gate.busy();
        let mut body = div()
            .id("cloud-settings")
            .w_full()
            .flex()
            .flex_col()
            .gap_6()
            .text_sm()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_lg().child(self.surface.title()))
                    .child(
                        div()
                            .id("refresh-cloud")
                            .cursor_pointer()
                            .text_xs()
                            .child(if self.loading {
                                "Refreshing…"
                            } else {
                                "Refresh"
                            })
                            .when(!busy, |view| {
                                view.on_click(cx.listener(|this, _, _, cx| this.reload(cx)))
                            }),
                    ),
            )
            .child(
                div()
                    .text_color(colors.muted_foreground)
                    .child(self.status.clone()),
            );
        if let Some(panel) = self.panel.clone() {
            let rows = panel.rows.clone();
            let count = rows.len();
            body = body.child(uniform_list("cloud-rows", count, cx.processor(move |_this, range: std::ops::Range<usize>, _, cx| {
                range.map(|index| {
                    let row = &rows[index];
                    let id = row.id.clone();
                    div().id(SharedString::from(id.clone())).h(px(60.)).px_3().flex().items_center().justify_between().border_b_1().border_color(colors.border)
                        .child(div().flex().flex_col().gap_1().child(row.title.to_string()).child(div().text_xs().text_color(colors.muted_foreground).child(row.detail.to_string())))
                        .when(id.contains(':'), |view| view.cursor_pointer().child("›").on_click(cx.listener(move |this, _, _, cx| {
                            if this.has_unsaved() { this.status = "Save or cancel the current form before changing selection".into(); cx.notify(); return; }
                            if let Some((kind, target)) = id.split_once(':') {
                                if kind == "workspace" {
                                    let mut scope = this.scope.clone();
                                    scope.workspace_id = Some(target.into());
                                    this.change_scope(scope, cx);
                                } else {
                                    this.selected = Some((kind.into(), target.into()));
                                    cx.notify();
                                }
                            }
                        })))
                }).collect()
            })).h(px((count.min(6) * 60) as f32)));
            body = body.child(
                div().flex().flex_wrap().gap_2().children(
                    panel
                        .operations
                        .iter()
                        .filter(|operation| {
                            selection_allows(
                                &operation.action,
                                self.selected.as_ref().map(|(kind, _)| kind.as_str()),
                            )
                        })
                        .map(|operation| {
                            let operation = operation.clone();
                            div()
                                .id(SharedString::from(operation.id.clone()))
                                .rounded(px(12.))
                                .border_1()
                                .border_color(colors.border)
                                .px_3()
                                .py_2()
                                .child(operation.label.to_string())
                                .when(!busy && operation.disabled_reason.is_none(), |view| {
                                    view.cursor_pointer().on_click(cx.listener(
                                        move |this, _, _, cx| this.choose(operation.clone(), cx),
                                    ))
                                })
                        }),
                ),
            );
        }
        body = body.child(
            div()
                .flex()
                .gap_3()
                .when(self.page > 0 && !busy, |view| {
                    view.child(
                        div()
                            .id("cloud-previous-page")
                            .cursor_pointer()
                            .child("Previous")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.page -= 1;
                                this.selected = None;
                                this.reload(cx);
                            })),
                    )
                })
                .when(
                    self.panel
                        .as_ref()
                        .is_some_and(|panel| panel.rows.len() == 128)
                        && !busy,
                    |view| {
                        view.child(
                            div()
                                .id("cloud-next-page")
                                .cursor_pointer()
                                .child("Next")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.page += 1;
                                    this.selected = None;
                                    this.reload(cx);
                                })),
                        )
                    },
                ),
        );
        if self.surface == Surface::Sharing {
            body = body.child(
                div()
                    .id("review-share-conflict")
                    .cursor_pointer()
                    .child("Review publication conflict")
                    .when(!busy, |view| {
                        view.on_click(cx.listener(|this, _, _, cx| this.review(cx)))
                    }),
            );
        }
        let commands = if self.surface == Surface::Teams && self.scope.workspace_id.is_some() {
            vec![
                ("rename", "Rename workspace", Action::CreateWorkspace),
                ("remove", "Remove selected member", Action::SetMemberRole),
            ]
        } else if self.surface == Surface::Sharing
            && self
                .selected
                .as_ref()
                .is_some_and(|(kind, _)| kind == "share-grant")
        {
            vec![("revoke", "Revoke selected access", Action::SetShareRole)]
        } else {
            vec![]
        };
        body = body.child(
            div().flex().gap_2().children(
                commands
                    .into_iter()
                    .filter(|(kind, _, _)| {
                        *kind != "remove"
                            || self
                                .selected
                                .as_ref()
                                .is_some_and(|(kind, _)| kind == "member")
                    })
                    .map(|(kind, label, action)| {
                        div()
                            .id(kind)
                            .rounded(px(12.))
                            .border_1()
                            .border_color(colors.border)
                            .px_3()
                            .py_2()
                            .child(label)
                            .when(!busy, |view| {
                                view.cursor_pointer().on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        let mut operation =
                                            super::panels::operation(action.clone(), label);
                                        operation.confirmation = Some(
                                            format!(
                                                "{label}? Confirm this access or workspace change."
                                            )
                                            .into(),
                                        );
                                        this.choose(operation, cx);
                                        this.management = Some(kind);
                                    },
                                ))
                            })
                    }),
            ),
        );
        if let Some(review) = self.conflict.clone() {
            body = body.child(div().flex().gap_4()
                .child(snapshot("local-snapshot", format!("Local draft: {}", review.local_title), review.local_preview))
                .child(snapshot("remote-snapshot", format!("Published: {}", review.remote_title), review.remote_preview)))
                .child(div().text_xs().child("Both original documents remain intact. Publishing local changes replaces the reviewed web revision; newer edits produce another conflict."))
                .child(div().flex().gap_3()
                    .child(div().id("keep-remote").px_3().py_2().border_1().border_color(colors.border).rounded(px(12.)).child("Keep web copy; retain local draft")
                        .when(!busy, |view| view.cursor_pointer().on_click(cx.listener(|this, _, _, cx| this.resolve(ConflictDecision::KeepRemote, cx)))))
                    .child(div().id("publish-local").px_3().py_2().bg(colors.accent).rounded(px(12.)).child("Replace reviewed web copy with local draft")
                        .when(!busy, |view| view.cursor_pointer().on_click(cx.listener(|this, _, _, cx| this.resolve(ConflictDecision::PublishLocal, cx))))));
        }
        if let Some(operation) = &self.dialog {
            let mut dialog = div()
                .p_4()
                .rounded(px(16.))
                .border_1()
                .border_color(colors.border)
                .flex()
                .flex_col()
                .gap_4()
                .child(div().text_lg().child(operation.label.to_string()));
            if let Some(confirmation) = &operation.confirmation {
                dialog = dialog.child(confirmation.to_string());
            }
            if let Some(panel) = &self.panel {
                for field in panel
                    .fields
                    .iter()
                    .filter(|field| fields_for(&operation.action).contains(&field.id.as_ref()))
                {
                    if let Some(input) = self.inputs.get(&field.id) {
                        let choices = choices_for(&field.id, self.surface);
                        let control = if choices.is_empty() {
                            div().child(input.clone())
                        } else {
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_2()
                                .children(choices.iter().map(|choice| {
                                    let input = input.clone();
                                    let choice = *choice;
                                    div()
                                        .id(SharedString::from(format!("{}-{choice}", field.id)))
                                        .px_3()
                                        .py_1()
                                        .rounded(px(12.))
                                        .border_1()
                                        .border_color(colors.border)
                                        .when(input.read(cx).buffer.text == choice, |view| {
                                            view.bg(colors.accent)
                                        })
                                        .child(choice)
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            input.update(cx, |input, cx| {
                                                input.set_text(choice.into(), cx)
                                            });
                                            this.dirty = true;
                                            cx.notify();
                                        }))
                                }))
                        };
                        dialog = dialog.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(field.label.to_string())
                                .child(control),
                        );
                    }
                }
            }
            body = body.child(
                dialog.child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_3()
                        .child(
                            div()
                                .id("cancel-cloud-dialog")
                                .px_3()
                                .py_2()
                                .child("Cancel")
                                .when(!busy, |view| {
                                    view.cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.dialog = None;
                                            this.dirty = false;
                                            this.reload(cx);
                                        }))
                                }),
                        )
                        .child(
                            div()
                                .id("submit-cloud-dialog")
                                .px_3()
                                .py_2()
                                .rounded(px(12.))
                                .bg(colors.accent)
                                .child(if busy { "Working…" } else { "Confirm" })
                                .when(!busy, |view| {
                                    view.cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| this.submit(cx)))
                                }),
                        ),
                ),
            );
        }
        body
    }
}

fn snapshot(id: &'static str, title: String, lines: Arc<[String]>) -> impl IntoElement {
    let count = lines.len();
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_2()
        .child(title)
        .child(
            uniform_list(id, count, move |range, _, _| {
                range
                    .map(|index| div().h(px(20.)).text_xs().child(lines[index].clone()))
                    .collect()
            })
            .h(px(260.)),
        )
}

fn selection_allows(action: &Action, selected: Option<&str>) -> bool {
    match action {
        Action::AcceptInvitation | Action::DeclineInvitation => selected == Some("invitation"),
        Action::ResendInvitation | Action::RevokeInvitation => {
            matches!(selected, Some("invitation" | "share-invitation"))
        }
        Action::SetMemberRole | Action::TransferOwnership => selected == Some("member"),
        Action::ApproveDevice => selected == Some("enrollment"),
        Action::RenameDevice | Action::RemoveDevice => selected == Some("device"),
        Action::ReplaceDevice => matches!(selected, None | Some("device")),
        Action::SetShareRole => selected == Some("share-grant"),
        Action::RespondShareRequest => selected == Some("share-request"),
        Action::DeleteComment => selected == Some("comment"),
        _ => true,
    }
}

fn fields_for(action: &Action) -> &'static [&'static str] {
    match action {
        Action::SignIn => &["provider"],
        Action::CreateWorkspace => &["name"],
        Action::InviteMember | Action::InviteToShare => &["email", "role"],
        Action::SetMemberRole | Action::SetShareRole => &["role"],
        Action::SetWorkspacePolicy => &[
            "allowed_scopes",
            "default_scope",
            "retention_days",
            "training_opt_out",
            "consent",
            "require_sso",
        ],
        Action::SetSharingSlug => &["slug"],
        Action::RenameDevice => &["device_name"],
        Action::SetShareScope => &["scope", "workspace"],
        Action::RespondShareRequest => &["decision", "role"],
        Action::AddComment => &["comment"],
        Action::Checkout | Action::StartTrial => &["interval"],
        _ => &[],
    }
}

fn choices_for(field: &str, surface: Surface) -> &'static [&'static str] {
    match field {
        "role" if surface == Surface::Teams => &["admin", "member"],
        "role" => &["viewer", "commenter", "editor"],
        "scope" => &["restricted", "workspace", "public"],
        "default_scope" => &["restricted", "workspace", "link", "public"],
        "provider" => &["google", "apple"],
        "decision" => &["approved", "denied"],
        "interval" => &["monthly", "yearly"],
        "plan" => &["pro", "lite"],
        "training_opt_out" | "consent" | "require_sso" => &["true", "false"],
        _ => &[],
    }
}
