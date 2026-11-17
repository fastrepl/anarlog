//! Shipping's `ResourceShareButton` dialog (`resource-sharing/share-dialog.tsx`) for folders and
//! templates: the sign-in and Pro gates, the Team library select, guest invites, who has
//! access, and Stop sharing.

use std::sync::Arc;

use desktop_runtime::{CancellationToken, RuntimeHandle, ServiceError};
use gpui::{
    AnyElement, Context, Div, Entity, EventEmitter, FontWeight, Render, SharedString, Window,
    anchored, deferred, div, prelude::*, px,
};
use serde_json::json;

use super::{
    SharingCloud,
    model::ShareWorkspace,
    popover::{button, icon, select_item, select_list, spinner},
};
use crate::ui::a11y::Accessible;
use crate::{
    product::{
        account,
        cloud::resources::{Guest, ResourceCommand, ResourceKind, SharedResource, is_team_upsell},
    },
    ui::{
        input::{InputEvent, TextInput},
        theme::{Theme, theme},
    },
    workspace::{
        navigation::Route,
        resource_list::{dialog_button, glass_dialog},
    },
};

const WORKSPACES_SQL: &str = "SELECT w.id, w.name FROM workspaces w
    JOIN workspace_memberships m ON m.workspace_id = w.id AND m.user_id = ? AND m.deleted_at IS NULL
    WHERE w.kind = 'shared' AND w.deleted_at IS NULL
    ORDER BY w.name COLLATE NOCASE, w.id";

pub enum ResourceDialogEvent {
    Close,
    Navigate(Route),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Upsell {
    Guest,
    Workspace,
}

#[derive(Clone, PartialEq, Eq)]
enum Pending {
    Publish,
    Invite,
    Revoke(Arc<str>),
    Stop,
}

pub struct ResourceShareDialog {
    runtime: RuntimeHandle,
    kind: ResourceKind,
    source: Arc<str>,
    title: Arc<str>,
    email: Entity<TextInput>,
    owned: Option<SharedResource>,
    guests: Vec<Guest>,
    workspaces: Arc<[ShareWorkspace]>,
    /// The Team picked in the select; `Some(None)` is "Not shared with a Team".
    draft: Option<Option<Arc<str>>>,
    select_open: bool,
    pending: Option<Pending>,
    error: Option<SharedString>,
    upsell: Option<Upsell>,
    cancel: CancellationToken,
    _subscriptions: Vec<gpui::Subscription>,
}

impl EventEmitter<ResourceDialogEvent> for ResourceShareDialog {}

impl ResourceShareDialog {
    pub fn new(
        runtime: RuntimeHandle,
        kind: ResourceKind,
        source: Arc<str>,
        title: Arc<str>,
        cx: &mut Context<Self>,
    ) -> Self {
        let email = cx.new(|cx| TextInput::new("guest@company.com", cx).inline());
        let subscriptions = vec![
            cx.subscribe(&email, |this, _, event, cx| match event {
                InputEvent::Submitted => this.invite(cx),
                _ => {
                    this.upsell = None;
                    this.error = None;
                    cx.notify();
                }
            }),
            cx.observe_global::<account::Account>(|this, cx| this.load(cx)),
        ];
        let mut this = Self {
            runtime,
            kind,
            source,
            title,
            email,
            owned: None,
            guests: Vec::new(),
            workspaces: Arc::from([]),
            draft: None,
            select_open: false,
            pending: None,
            error: None,
            upsell: None,
            cancel: CancellationToken::new(),
            _subscriptions: subscriptions,
        };
        this.load(cx);
        this
    }

    fn cloud(cx: &gpui::App) -> Option<crate::product::cloud::CloudServices> {
        cx.try_global::<SharingCloud>().map(|cloud| cloud.0.clone())
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        self.cancel.cancel();
        self.cancel = CancellationToken::new();
        let cancel = self.cancel.clone();
        if let Some(user) = account::current(cx).user.map(|user| user.id) {
            let reply = self
                .runtime
                .read(cancel.clone(), move |services| async move {
                    services
                        .executor
                        .execute(WORKSPACES_SQL.into(), vec![json!(user)])
                        .await
                        .map_err(|error| ServiceError::Failed(error.to_string().into()))
                });
            cx.spawn(async move |this, cx| {
                let Ok(reply) = reply else {
                    return;
                };
                if let Ok(rows) = reply.receive().await {
                    let _ = this.update(cx, |this, cx| {
                        this.workspaces = rows
                            .iter()
                            .map(|row| ShareWorkspace {
                                id: row["id"].as_str().unwrap_or_default().into(),
                                name: row["name"].as_str().unwrap_or_default().into(),
                            })
                            .collect();
                        cx.notify();
                    });
                }
            })
            .detach();
        }
        let Some(cloud) = Self::cloud(cx).filter(|_| account::current(cx).signed_in()) else {
            return;
        };
        let (kind, source) = (self.kind, self.source.clone());
        cx.spawn(async move |this, cx| {
            let owned = cloud
                .shared_resources(kind)
                .await
                .ok()
                .and_then(|resources| {
                    resources
                        .into_iter()
                        .find(|resource| resource.owned && resource.source_id == source)
                });
            let guests = match &owned {
                Some(owned) => cloud
                    .resource_guests(owned.share_id.clone())
                    .await
                    .unwrap_or_default(),
                None => Vec::new(),
            };
            if cancel.is_cancelled() {
                return;
            }
            let _ = this.update(cx, |this, cx| {
                this.owned = owned;
                this.guests = guests;
                cx.notify();
            });
        })
        .detach();
    }

    fn selected_workspace(&self) -> Option<Arc<str>> {
        match &self.draft {
            Some(choice) => choice.clone(),
            None => self
                .owned
                .as_ref()
                .and_then(|owned| owned.general_workspace_id.clone()),
        }
    }

    fn run(&mut self, pending: Pending, command: ResourceCommand, cx: &mut Context<Self>) {
        let Some(cloud) = Self::cloud(cx) else {
            self.error = Some("Shared resource request failed".into());
            cx.notify();
            return;
        };
        self.pending = Some(pending.clone());
        self.error = None;
        let (kind, source, title) = (self.kind, self.source.clone(), self.title.clone());
        cx.spawn(async move |this, cx| {
            let result = cloud.resource_command(kind, source, title, command).await;
            let _ = this.update(cx, |this, cx| {
                this.pending = None;
                match result {
                    Ok(()) => {
                        this.upsell = None;
                        if pending == Pending::Invite {
                            this.email
                                .update(cx, |input, cx| input.set_text(String::new(), cx));
                        }
                        if pending == Pending::Stop {
                            cx.emit(ResourceDialogEvent::Close);
                        }
                        this.load(cx);
                    }
                    Err(error) => {
                        if is_team_upsell(&error)
                            && matches!(pending, Pending::Publish | Pending::Invite)
                        {
                            this.upsell = Some(if pending == Pending::Invite {
                                Upsell::Guest
                            } else {
                                Upsell::Workspace
                            });
                        } else {
                            this.error = Some(error.to_string().into());
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn invite(&mut self, cx: &mut Context<Self>) {
        let email = self.email.read(cx).buffer.text.trim().to_owned();
        if email.is_empty() || self.pending.is_some() || !account::is_paid(cx) {
            return;
        }
        let workspace = self.selected_workspace();
        self.run(
            Pending::Invite,
            ResourceCommand::Invite { workspace, email },
            cx,
        );
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.pending, Some(Pending::Publish | Pending::Invite)) {
            cx.emit(ResourceDialogEvent::Close);
        }
    }
}

fn gate(description: &'static str, action: AnyElement, colors: Theme) -> Div {
    div()
        .p_3()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .bg(colors.accent.opacity(0.45))
        .child(
            div()
                .max_w(px(205.))
                .text_xs()
                .line_height(px(20.))
                .text_color(colors.muted_foreground)
                .child(description),
        )
        .child(div().flex_shrink_0().child(action))
}

fn label(text: &'static str) -> Div {
    div()
        .text_sm()
        .line_height(px(20.))
        .font_weight(crate::ui::theme::MEDIUM)
        .child(text)
}

impl Render for ResourceShareDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let account = account::current(cx);
        let paid = account.is_paid(account::now());
        let header = div()
            .flex()
            .flex_col()
            .gap_1()
            .text_center()
            .child(
                div()
                    .text_base()
                    .line_height(px(24.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(SharedString::from(format!("Share {}", self.title))),
            )
            // Shipping balances these short lines (`text-wrap: pretty`); the width reproduces
            // its break for this copy.
            .child(
                div()
                    .max_w(px(320.))
                    .mx_auto()
                    .text_sm()
                    .line_height(px(20.))
                    .text_color(colors.muted_foreground)
                    .child("Share one item with a guest, or add it to a Team for everyone."),
            );
        let body: AnyElement = if !account.signed_in() {
            gate(
                "Sign in to share folders, templates, and automations.",
                button("resource-share-sign-in", true, true, colors)
                    .a11y_button("Sign in")
                    .on_click(cx.listener(|_, _, _, cx| {
                        cx.emit(ResourceDialogEvent::Navigate(Route::settings("account")))
                    }))
                    .child("Sign in")
                    .into_any_element(),
                colors,
            )
            .into_any_element()
        } else if !paid && self.owned.is_none() {
            gate(
                "Folder access controls and reusable sharing are included with Pro.",
                button("resource-share-upgrade", true, true, colors)
                    .a11y_button("Upgrade to Pro")
                    .on_click(cx.listener(|_, _, _, cx| {
                        cx.emit(ResourceDialogEvent::Navigate(Route::settings("billing")))
                    }))
                    .child("Upgrade to Pro")
                    .into_any_element(),
                colors,
            )
            .into_any_element()
        } else {
            self.form(paid, colors, window, cx).into_any_element()
        };
        glass_dialog(colors)
            .w(px(420.))
            .text_color(colors.foreground)
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                if !this.select_open {
                    this.close(cx)
                }
            }))
            .child(header)
            .child(body)
    }
}

impl ResourceShareDialog {
    fn form(
        &mut self,
        paid: bool,
        colors: Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let workspace = self.selected_workspace();
        let workspace_label: SharedString = workspace
            .as_ref()
            .and_then(|id| self.workspaces.iter().find(|w| &w.id == id))
            .map(|w| w.name.to_string())
            .unwrap_or_else(|| "Not shared with a Team".into())
            .into();
        let publishing = self.pending == Some(Pending::Publish);
        let inviting = self.pending == Some(Pending::Invite);
        let typed = !self.email.read(cx).buffer.text.trim().is_empty();
        let field = |id: &'static str| {
            div()
                .id(id)
                .h(px(36.))
                .px_3()
                .flex()
                .items_center()
                .border_1()
                .border_color(colors.input)
                .bg(colors.card.opacity(0.6))
                .shadow_xs()
                .text_sm()
                .line_height(px(20.))
        };
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(label("Team library"))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                div()
                                    .relative()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        field("resource-share-team")
                                            .justify_between()
                                            .gap_2()
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.select_open = !this.select_open;
                                                cx.notify();
                                            }))
                                            .child(div().min_w_0().truncate().child(workspace_label))
                                            .child(icon(
                                                "sharing/ArrowDown01Icon.svg",
                                                16.,
                                                colors.foreground.opacity(0.5),
                                            )),
                                    )
                                    .when(self.select_open, |view| {
                                        view.child(self.team_menu(workspace.clone(), colors, cx))
                                    }),
                            )
                            .child(
                                field("resource-share-publish")
                                    .px_4()
                                    .justify_center()
                                    .font_weight(crate::ui::theme::MEDIUM)
                                    .when_else(
                                        paid && !publishing,
                                        |b| {
                                            b.cursor_pointer()
                                                .hover(|s| s.bg(colors.accent))
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    let workspace = this.selected_workspace();
                                                    this.run(
                                                        Pending::Publish,
                                                        ResourceCommand::Publish { workspace },
                                                        cx,
                                                    )
                                                }))
                                        },
                                        |b| b.opacity(0.5),
                                    )
                                    .child(if publishing {
                                        spinner("resource-publish-spin", 16., colors.foreground)
                                    } else if self.owned.is_some() {
                                        "Update".into_any_element()
                                    } else {
                                        "Add".into_any_element()
                                    }),
                            ),
                    )
                    .when(self.workspaces.is_empty(), |view| {
                        view.child(
                            div().flex().child(
                                div()
                                    .id("resource-share-create-team")
                                    .a11y_button("Create a Team to share a library")
                                    .text_xs()
                                    .line_height(px(16.))
                                    .text_color(colors.muted_foreground)
                                    .underline()
                                    .cursor_pointer()
                                    .hover(|s| s.text_color(colors.foreground))
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(ResourceDialogEvent::Navigate(Route::settings("team")))
                                    }))
                                    .child("Create a Team to share a library"),
                            ),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(label("Guest access"))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                field("resource-share-email")
                                    .flex_1()
                                    .min_w_0()
                                    .when(!paid || inviting, |f| f.opacity(0.5))
                                    .child(div().w_full().overflow_hidden().child(self.email.clone())),
                            )
                            .child(
                                button("resource-share-invite", true, typed && paid && !inviting, colors)
                                    .h(px(36.))
                                    .px_4()
                                    .text_sm()
                                    .line_height(px(20.))
                                    .when(typed && paid && !inviting, |b| {
                                        b.on_click(cx.listener(|this, _, _, cx| this.invite(cx)))
                                    })
                                    .child(if inviting {
                                        spinner("resource-invite-spin", 16., colors.primary_foreground)
                                    } else {
                                        icon("sharing/UserPlusIcon.svg", 16., colors.primary_foreground)
                                            .into_any_element()
                                    })
                                    .child("Invite"),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .line_height(px(16.))
                            .text_color(colors.muted_foreground)
                            .child(
                                "A guest can access one shared item for free. Add them to your Team when they need more.",
                            ),
                    ),
            )
            .when_some(self.upsell, |view, upsell| view.child(self.upsell_box(upsell, colors, cx)))
            .when_some(self.owned.clone(), |view, owned| {
                view.child(self.access_rows(&owned, colors, cx))
            })
            .when_some(self.error.clone().filter(|_| self.upsell.is_none()), |view, error| {
                view.child(
                    div()
                        .text_xs()
                        .line_height(px(16.))
                        .text_color(colors.destructive)
                        .child(error),
                )
            })
            .child(self.footer(colors, window, cx))
    }

    fn team_menu(
        &self,
        selected: Option<Arc<str>>,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mut options: Vec<(Option<Arc<str>>, String)> =
            vec![(None, "Not shared with a Team".into())];
        options.extend(
            self.workspaces
                .iter()
                .map(|workspace| (Some(workspace.id.clone()), workspace.name.to_string())),
        );
        div().absolute().top_full().left_0().right_0().child(
            deferred(
                anchored().snap_to_window_with_margin(px(8.)).child(
                    div()
                        .id("resource-share-team-menu")
                        .mt_1()
                        .occlude()
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.select_open = false;
                            cx.notify();
                        }))
                        .child(
                            select_list(220., colors).children(
                                options
                                    .into_iter()
                                    .enumerate()
                                    .map(|(index, (value, text))| {
                                        let chosen = value == selected;
                                        select_item(
                                            ("resource-team", index),
                                            text,
                                            chosen,
                                            true,
                                            None,
                                            colors,
                                        )
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                this.draft = Some(value.clone());
                                                this.select_open = false;
                                                cx.notify();
                                            }),
                                        )
                                    }),
                            ),
                        ),
                ),
            )
            .with_priority(4),
        )
    }

    fn upsell_box(
        &self,
        upsell: Upsell,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (title, detail) = match upsell {
            Upsell::Guest => (
                "This guest needs Team access",
                "They already have another shared item. Invite them to your Team for folders, templates, and automations.",
            ),
            Upsell::Workspace => (
                "Unlock the Team library",
                "Upgrade this workspace to share folders, templates, and automations with everyone on the Team.",
            ),
        };
        div()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .border_1()
            .border_color(colors.border)
            .bg(colors.accent.opacity(0.45))
            .child(label(title))
            .child(
                div()
                    .text_xs()
                    .line_height(px(20.))
                    .text_color(colors.muted_foreground)
                    .child(detail),
            )
            .child(
                div().flex().child(
                    button("resource-share-open-teams", true, true, colors)
                        .a11y_button("Open Teams")
                        .on_click(cx.listener(|_, _, _, cx| {
                            cx.emit(ResourceDialogEvent::Navigate(Route::settings("team")))
                        }))
                        .child("Open Teams"),
                ),
            )
    }

    fn access_rows(
        &self,
        owned: &SharedResource,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let row = |label: SharedString| {
            div()
                .min_h(px(32.))
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .text_sm()
                .line_height(px(20.))
                .child(div().min_w_0().truncate().child(label))
        };
        div()
            .pt_3()
            .flex()
            .flex_col()
            .gap_1()
            .border_t_1()
            .border_color(colors.border.opacity(0.6))
            .when(owned.general_workspace_id.is_some(), |view| {
                let name = owned
                    .workspace_name
                    .as_ref()
                    .map(|name| format!("Everyone in {name}"))
                    .unwrap_or_else(|| "Everyone in the Team".into());
                view.child(
                    row(name.into()).child(
                        div()
                            .text_xs()
                            .line_height(px(16.))
                            .text_color(colors.muted_foreground)
                            .child("Team"),
                    ),
                )
            })
            .children(self.guests.iter().map(|guest| {
                let id = guest.id.clone();
                let pending = self.pending == Some(Pending::Revoke(id.clone()));
                row(guest.email.to_string().into()).child(
                    div()
                        .id(SharedString::from(format!("resource-guest-{id}")))
                        .h(px(28.))
                        .px_2()
                        .flex()
                        .items_center()
                        .text_xs()
                        .font_weight(crate::ui::theme::MEDIUM)
                        .when_else(
                            pending,
                            |b| b.opacity(0.5),
                            |b| {
                                b.cursor_pointer().hover(|s| s.bg(colors.accent)).on_click(
                                    cx.listener(move |this, _, _, cx| {
                                        this.run(
                                            Pending::Revoke(id.clone()),
                                            ResourceCommand::Revoke(id.clone()),
                                            cx,
                                        )
                                    }),
                                )
                            },
                        )
                        .child("Remove"),
                )
            }))
    }

    fn footer(&self, colors: Theme, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let red = if crate::ui::theme::is_dark(window) {
            gpui::rgb(0xf87171)
        } else {
            gpui::rgb(0xdc2626)
        };
        let stopping = self.pending == Some(Pending::Stop);
        div()
            .flex()
            .items_center()
            .justify_between()
            .child(match &self.owned {
                Some(owned) => {
                    let share = owned.share_id.clone();
                    div()
                        .id("resource-share-stop")
                        .h(px(28.))
                        .px_2()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_xs()
                        .font_weight(crate::ui::theme::MEDIUM)
                        .text_color(red)
                        .when_else(
                            stopping,
                            |b| b.opacity(0.5),
                            |b| {
                                b.cursor_pointer().hover(|s| s.bg(colors.accent)).on_click(
                                    cx.listener(move |this, _, _, cx| {
                                        this.run(
                                            Pending::Stop,
                                            ResourceCommand::StopSharing(share.clone()),
                                            cx,
                                        )
                                    }),
                                )
                            },
                        )
                        .child(icon("sharing/TrashIcon.svg", 16., red.into()))
                        .child("Stop sharing")
                        .into_any_element()
                }
                None => div().into_any_element(),
            })
            .child(
                dialog_button("dialog-cancel", "Done", false, colors)
                    .on_click(cx.listener(|this, _, _, cx| this.close(cx))),
            )
    }
}
