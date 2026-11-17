//! `settings/team/index.tsx` for a signed-in account: pending invitations, workspace tabs, the
//! create form, and the selected workspace's billing, members, invitations, admin policies,
//! sharing domain, usage, ownership transfer and leave/delete.

use std::{future::Future, sync::Arc, time::Duration};

use desktop_runtime::Result;
use gpui::{
    AnyElement, App, ClickEvent, Context, Div, Entity, EventEmitter, Focusable, FontWeight,
    KeyDownEvent, Render, SharedString, Stateful, Subscription, Task, Window, div, prelude::*, px,
};
use serde_json::json;

use super::{
    calls::CloudCalls,
    teams::client::{
        self, Access, AutoJoin, Delivery, Invitation, Member, MyInvitation, MyWorkspace,
        OwnershipRequest, Policy, Role, SlugAvailability, Usage,
    },
};
use crate::{
    application::{team_invitations, toasts},
    product::{
        PlanRequired,
        account::{self, WorkspaceTier},
        account_view::plan_gate,
        controls,
    },
    ui::{
        input::{InputEvent, TextInput},
        theme::{MEDIUM, Theme, theme},
    },
    workspace::{contacts::facehash, resource_list as list},
};

type Load<T> = Option<std::result::Result<T, String>>;

const POLL: Duration = Duration::from_secs(30);

/// What a manager can do in the selected workspace, from its role and plan capabilities.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Caps {
    pub manager: bool,
    pub manage_workspace: bool,
    pub manage_members: bool,
    pub manage_policies: bool,
    pub view_usage: bool,
    pub custom_subdomain: bool,
    pub sso: bool,
    pub scim: bool,
    pub retention: bool,
    pub paid: bool,
}

impl Caps {
    pub fn new(role: Role, access: Option<&Access>) -> Self {
        let manager = matches!(role, Role::Owner | Role::Admin);
        let has = |capability| manager && access.is_some_and(|access| access.has(capability));
        let enterprise = access.is_some_and(|access| access.tier == WorkspaceTier::Enterprise);
        Self {
            manager,
            manage_workspace: has("team.manage_workspace"),
            manage_members: has("team.manage_members"),
            manage_policies: enterprise && has("team.manage_policies"),
            view_usage: enterprise && has("team.view_usage"),
            custom_subdomain: enterprise && has("team.custom_subdomain"),
            sso: has("enterprise.sso"),
            scim: has("enterprise.scim"),
            retention: has("enterprise.retention"),
            paid: access.is_some_and(|access| access.tier != WorkspaceTier::Free),
        }
    }
}

/// `MemberRow`'s permissions: whether the viewer may change this member's role, remove them,
/// or offer them primary ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemberControls {
    pub edit_role: bool,
    pub remove: bool,
    pub transfer: bool,
}

pub fn member_controls(
    member: Role,
    member_is_primary: bool,
    member_is_viewer: bool,
    viewer: Option<Role>,
    viewer_is_primary: bool,
    manage_members: bool,
) -> MemberControls {
    let owner = member == Role::Owner;
    let by_role = |allow_self: bool| match viewer {
        Some(Role::Owner) => !owner || viewer_is_primary || (allow_self && member_is_viewer),
        Some(Role::Admin) => member == Role::Member,
        _ => false,
    };
    MemberControls {
        edit_role: manage_members && by_role(true),
        remove: manage_members && !member_is_primary && !member_is_viewer && by_role(false),
        transfer: manage_members && viewer_is_primary && !member_is_primary,
    }
}

/// `WorkspacePolicyForm`'s save: only what the plan lets this manager configure changes.
pub fn policy_to_save(
    current: &Policy,
    allow_link: bool,
    allow_public: bool,
    retention: &str,
    require_sso: bool,
    caps: Caps,
) -> Policy {
    let mut scopes = vec!["restricted".to_owned(), "workspace".to_owned()];
    if allow_link {
        scopes.push("link".into());
    }
    if allow_public {
        scopes.push("public".into());
    }
    Policy {
        allowed_share_scopes: scopes,
        retention_days: if caps.retention {
            retention.trim().parse().ok()
        } else {
            current.retention_days
        },
        require_sso: if caps.sso {
            require_sso
        } else {
            current.require_sso
        },
        ..current.clone()
    }
}

fn valid_slug(slug: &str) -> bool {
    (3..=63).contains(&slug.len())
        && slug
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !slug.starts_with('-')
        && !slug.ends_with('-')
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Busy {
    Invite,
    Role,
    Remove,
    CancelInvite,
    Resend,
    Transfer,
    Respond,
    Rename,
    Logo,
    Leave,
    Delete,
    Policy,
    Domain,
    Scim,
    Slug,
    AutoJoin,
    Billing,
}

#[derive(Clone)]
enum Dialog {
    Invite,
    Remove(Member),
    CancelInvite(Invitation),
    Transfer(Member),
    AcceptTransfer,
    Delete,
    Leave,
}

#[derive(Clone, PartialEq)]
enum Menu {
    Role(Arc<str>),
    Member(Arc<str>),
    Invitation(Arc<str>),
}

struct PolicyForm {
    allow_link: bool,
    allow_public: bool,
    require_sso: bool,
    retention: Entity<TextInput>,
    domain: Entity<TextInput>,
    scim: Entity<TextInput>,
}

struct SlugForm {
    input: Entity<TextInput>,
    current: String,
    /// The last value checked, and its answer (`None` while checking).
    checked: Option<(
        String,
        Option<std::result::Result<SlugAvailability, String>>,
    )>,
    error: Option<String>,
}

struct Panel {
    workspace: MyWorkspace,
    generation: u64,
    access: Load<Access>,
    members: Load<Arc<[Member]>>,
    ownership: Load<Arc<[OwnershipRequest]>>,
    invitations: Arc<[Invitation]>,
    usage: Option<Usage>,
    policy: Option<Policy>,
    policy_form: Option<PolicyForm>,
    policy_error: Option<String>,
    auto_join: Load<AutoJoin>,
    auto_join_error: Option<String>,
    name: Entity<TextInput>,
    invite_email: Entity<TextInput>,
    slug: Option<SlugForm>,
    menu: Option<Menu>,
    dialog: Option<Dialog>,
    busy: Option<Busy>,
    error: Option<String>,
    dialog_error: Option<String>,
    subscriptions: Vec<Subscription>,
    /// The name saves on blur as well as Enter; registered on first render (needs a window).
    blur: Option<Subscription>,
}

pub struct TeamsView {
    calls: Arc<dyn CloudCalls>,
    user: Option<(Arc<str>, Option<Arc<str>>)>,
    workspaces: Load<Arc<[MyWorkspace]>>,
    invitations: Arc<[MyInvitation]>,
    responding: Option<Arc<str>>,
    selected: Option<Arc<str>>,
    creating: bool,
    create_name: Entity<TextInput>,
    create_busy: bool,
    create_error: Option<String>,
    panel: Option<Panel>,
    generation: u64,
    _subscriptions: Vec<Subscription>,
    _poll: Task<()>,
}

impl EventEmitter<PlanRequired> for TeamsView {}

impl TeamsView {
    pub fn new(calls: Arc<dyn CloudCalls>, cx: &mut Context<Self>) -> Self {
        let create_name = cx.new(|cx| TextInput::new("Acme", cx).inline());
        let subscriptions = vec![
            cx.subscribe(&create_name, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Submitted) {
                    this.create(cx);
                }
                cx.notify();
            }),
            cx.observe_global::<account::Account>(|this, cx| {
                if this.user != viewer(cx) {
                    this.user = viewer(cx);
                    this.panel = None;
                    this.selected = None;
                    this.workspaces = None;
                    this.refresh(cx);
                }
            }),
        ];
        let poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                if this.update(cx, |this, cx| this.refresh(cx)).is_err() {
                    break;
                }
            }
        });
        let mut this = Self {
            calls,
            user: viewer(cx),
            workspaces: None,
            invitations: Arc::from([]),
            responding: None,
            selected: None,
            creating: false,
            create_name,
            create_busy: false,
            create_error: None,
            panel: None,
            generation: 0,
            _subscriptions: subscriptions,
            _poll: poll,
        };
        this.refresh(cx);
        this
    }

    pub fn has_unsaved(&self) -> bool {
        self.create_busy
            || self
                .panel
                .as_ref()
                .is_some_and(|panel| panel.busy.is_some())
    }

    /// Reloads the workspace list, the account's invitations and the open workspace.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.user.is_none() {
            return;
        }
        let calls = self.calls.clone();
        cx.spawn(async move |this, cx| {
            let (workspaces, invitations) = futures::join!(
                client::list_my_workspaces(&*calls),
                client::list_my_invitations(&*calls)
            );
            let _ = this.update(cx, |this, cx| {
                if let Ok(invitations) = invitations {
                    this.invitations = invitations.into();
                }
                match workspaces {
                    Ok(workspaces) => this.workspaces = Some(Ok(workspaces.into())),
                    // A failed refresh keeps the last list, like a refetching query.
                    Err(error) if this.workspaces.is_none() => {
                        this.workspaces = Some(Err(error.to_string()))
                    }
                    Err(_) => {}
                }
                this.sync_panel(cx);
                cx.notify();
            });
        })
        .detach();
        if self.panel.is_some() {
            self.load_panel(cx);
        }
    }

    /// The workspace shown: the selected one, else the first, unless creating.
    fn selected_workspace(&self) -> Option<MyWorkspace> {
        if self.creating {
            return None;
        }
        let workspaces = self.workspaces.as_ref()?.as_ref().ok()?;
        workspaces
            .iter()
            .find(|workspace| Some(&workspace.id) == self.selected.as_ref())
            .or_else(|| workspaces.first())
            .cloned()
    }

    /// Opens a fresh panel when the shown workspace changes (`key={workspaceId}`), or updates
    /// the open one's name, logo and role.
    fn sync_panel(&mut self, cx: &mut Context<Self>) {
        let Some(workspace) = self.selected_workspace() else {
            self.panel = None;
            return;
        };
        if let Some(panel) = &mut self.panel
            && panel.workspace.id == workspace.id
        {
            panel.workspace = workspace;
            return;
        }
        self.generation += 1;
        let name = cx.new(|cx| {
            let mut input = TextInput::new("", cx).inline();
            input.set_text(workspace.name.to_string(), cx);
            input
        });
        let invite_email = cx.new(|cx| TextInput::new("teammate@company.com", cx).inline());
        let subscriptions = vec![
            cx.subscribe(&name, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Submitted) {
                    this.submit_rename(cx);
                }
            }),
            cx.subscribe(&invite_email, |this, _, event: &InputEvent, cx| {
                match event {
                    InputEvent::Submitted => this.invite(cx),
                    InputEvent::Changed => {
                        if let Some(panel) = &mut this.panel {
                            panel.dialog_error = None;
                        }
                    }
                    InputEvent::Rejected => {}
                }
                cx.notify();
            }),
        ];
        let slug = workspace
            .share_slug
            .as_deref()
            .unwrap_or_default()
            .to_owned();
        self.panel = Some(Panel {
            workspace,
            generation: self.generation,
            access: None,
            members: None,
            ownership: None,
            invitations: Arc::from([]),
            usage: None,
            policy: None,
            policy_form: None,
            policy_error: None,
            auto_join: None,
            auto_join_error: None,
            name,
            invite_email,
            slug: Some(SlugForm {
                input: cx.new(|cx| {
                    let mut input = TextInput::new("company", cx).inline();
                    input.set_text(slug.clone(), cx);
                    input
                }),
                current: slug,
                checked: None,
                error: None,
            }),
            menu: None,
            dialog: None,
            busy: None,
            error: None,
            dialog_error: None,
            subscriptions,
            blur: None,
        });
        if let Some(slug) = self.panel.as_ref().and_then(|panel| panel.slug.as_ref()) {
            let input = slug.input.clone();
            let subscription = cx.subscribe(&input, |this, _, event: &InputEvent, cx| {
                match event {
                    InputEvent::Changed => this.slug_changed(cx),
                    InputEvent::Submitted => this.save_slug(cx),
                    InputEvent::Rejected => {}
                }
                cx.notify();
            });
            if let Some(panel) = &mut self.panel {
                panel.subscriptions.push(subscription);
            }
        }
        self.load_panel(cx);
    }

    fn load_panel(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = &self.panel else {
            return;
        };
        let calls = self.calls.clone();
        let id = panel.workspace.id.clone();
        let role = panel.workspace.role;
        let primary = self.viewer_id().as_ref() == Some(&panel.workspace.owner_user_id);
        let generation = panel.generation;
        cx.spawn(async move |this, cx| {
            let (access, members, ownership) = futures::join!(
                client::access(&*calls, &id),
                client::members(&*calls, &id),
                client::ownership_requests(&*calls, &id)
            );
            let caps = Caps::new(role, access.as_ref().ok());
            let invitations = if caps.manager {
                client::invitations(&*calls, &id).await.ok()
            } else {
                None
            };
            let usage = if caps.view_usage {
                client::usage(&*calls, &id).await.ok()
            } else {
                None
            };
            let policy = if caps.manage_policies {
                client::policy(&*calls, &id).await.ok()
            } else {
                None
            };
            let auto_join = if primary && caps.manage_members {
                Some(client::auto_join(&*calls, &id).await)
            } else {
                None
            };
            let _ = this.update(cx, |this, cx| {
                let Some(panel) = this.panel.as_mut().filter(|p| p.generation == generation) else {
                    return;
                };
                let text = |error: desktop_runtime::ServiceError| error.to_string();
                // Refetch errors keep the data already shown.
                if access.is_ok() || panel.access.is_none() {
                    panel.access = Some(access.map_err(text));
                }
                if members.is_ok() || panel.members.is_none() {
                    panel.members = Some(members.map(Into::into).map_err(text));
                }
                if ownership.is_ok() || panel.ownership.is_none() {
                    panel.ownership = Some(ownership.map(Into::into).map_err(text));
                }
                if let Some(invitations) = invitations {
                    panel.invitations = invitations.into();
                }
                if usage.is_some() {
                    panel.usage = usage;
                }
                if let Some(policy) = policy {
                    if panel.policy_form.is_none() {
                        panel.policy_form = Some(policy_form(&policy, cx));
                    }
                    panel.policy = Some(policy);
                }
                if let Some(auto_join) = auto_join {
                    panel.auto_join = Some(auto_join.map_err(text));
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn viewer_id(&self) -> Option<Arc<str>> {
        self.user.as_ref().map(|(id, _)| id.clone())
    }

    /// Runs a workspace mutation for the open panel; results for another workspace are dropped.
    fn mutate<T: 'static>(
        &mut self,
        busy: Busy,
        work: impl Future<Output = Result<T>> + 'static,
        done: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let Some(panel) = &mut self.panel else {
            return;
        };
        if panel.busy.is_some() {
            return;
        }
        panel.busy = Some(busy);
        panel.error = None;
        panel.dialog_error = None;
        panel.menu = None;
        let generation = panel.generation;
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                let Some(panel) = this.panel.as_mut().filter(|p| p.generation == generation) else {
                    return;
                };
                panel.busy = None;
                match result {
                    Ok(value) => {
                        done(this, value, cx);
                        this.refresh(cx);
                    }
                    Err(error) => {
                        if panel.dialog.is_some() {
                            panel.dialog_error = Some(error.to_string());
                        } else {
                            panel.error = Some(error.to_string());
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn workspace_call(
        &mut self,
        busy: Busy,
        rpc: &'static str,
        body: serde_json::Value,
        done: impl FnOnce(&mut Self, (), &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let calls = self.calls.clone();
        self.mutate(
            busy,
            async move { client::call(&*calls, rpc, body).await },
            done,
            cx,
        );
    }

    fn close_dialog(this: &mut Self, _: (), _: &mut Context<Self>) {
        if let Some(panel) = &mut this.panel {
            panel.dialog = None;
        }
    }

    fn create(&mut self, cx: &mut Context<Self>) {
        let name = self.create_name.read(cx).buffer.text.trim().to_owned();
        if name.is_empty() || self.create_busy || !account::is_pro(cx) {
            return;
        }
        self.create_busy = true;
        self.create_error = None;
        let calls = self.calls.clone();
        cx.spawn(async move |this, cx| {
            let result = client::create_workspace(&*calls, &name).await;
            let _ = this.update(cx, |this, cx| {
                this.create_busy = false;
                match result {
                    Ok(id) => {
                        this.creating = false;
                        this.selected = Some(id);
                        this.create_name
                            .update(cx, |input, cx| input.set_text(String::new(), cx));
                        this.calls.refresh_account(cx);
                        this.refresh(cx);
                    }
                    Err(error) => this.create_error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn respond(&mut self, invitation: MyInvitation, accept: bool, cx: &mut Context<Self>) {
        if self.responding.is_some() {
            return;
        }
        self.responding = Some(invitation.invitation_id.clone());
        let calls = self.calls.clone();
        cx.spawn(async move |this, cx| {
            let result = if accept {
                client::accept_invitation(&*calls, &invitation.invitation_id)
                    .await
                    .map(Some)
            } else {
                client::call(
                    &*calls,
                    "decline_my_workspace_invitation",
                    json!({"p_invitation_id": invitation.invitation_id.as_ref()}),
                )
                .await
                .map(|()| None)
            };
            let _ = this.update(cx, |this, cx| {
                this.responding = None;
                match result {
                    Ok(joined) => {
                        if let Some(workspace) = joined {
                            this.creating = false;
                            this.selected = Some(workspace);
                            this.calls.refresh_account(cx);
                            toasts::success(format!("Joined {}", invitation.workspace_name), cx);
                        }
                        toasts::dismiss(&team_invitations::toast_id(&invitation.invitation_id), cx);
                        this.refresh(cx);
                    }
                    Err(error) => toasts::error(error.to_string(), cx),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn submit_rename(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = &self.panel else {
            return;
        };
        let draft = panel.name.read(cx).buffer.text.trim().to_owned();
        let current = panel.workspace.name.to_string();
        if draft.is_empty() || draft == current {
            panel
                .name
                .clone()
                .update(cx, |input, cx| input.set_text(current, cx));
            return;
        }
        let id = panel.workspace.id.clone();
        let calls = self.calls.clone();
        self.mutate(
            Busy::Rename,
            async move {
                client::call(
                    &*calls,
                    "rename_workspace",
                    json!({"p_workspace_id": id.as_ref(), "p_name": draft}),
                )
                .await
            },
            |this, (), cx| this.calls.refresh_account(cx),
            cx,
        );
    }

    fn set_logo(&mut self, logo: Option<String>, cx: &mut Context<Self>) {
        let Some(panel) = &self.panel else {
            return;
        };
        let id = panel.workspace.id.clone();
        self.workspace_call(
            Busy::Logo,
            "set_workspace_logo",
            json!({"p_workspace_id": id.as_ref(), "p_logo_data": logo}),
            |_, (), _| {},
            cx,
        );
    }

    /// `WorkspaceLogoButton`: picks an image, then crops and scales it to a 128px PNG.
    fn pick_logo(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let encoded = cx
                .background_spawn(async move { logo_data_url(&std::fs::read(path)?) })
                .await;
            let _ = this.update(cx, |this, cx| match encoded {
                Ok(url) => this.set_logo(Some(url), cx),
                Err(error) => tracing::error!(%error, "could not process the workspace logo"),
            });
        })
        .detach();
    }

    fn invite(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = &self.panel else {
            return;
        };
        let email = panel.invite_email.read(cx).buffer.text.trim().to_owned();
        if email.is_empty() {
            return;
        }
        let input = panel.invite_email.clone();
        self.deliver(Busy::Invite, email, Some(input), cx);
    }

    fn deliver(
        &mut self,
        busy: Busy,
        email: String,
        input: Option<Entity<TextInput>>,
        cx: &mut Context<Self>,
    ) {
        let Some(panel) = &self.panel else {
            return;
        };
        let calls = self.calls.clone();
        let id = panel.workspace.id.clone();
        let name = panel.workspace.name.clone();
        let sender = self
            .user
            .as_ref()
            .and_then(|(_, email)| email.clone())
            .map_or_else(|| "An Anarlog user".to_owned(), |email| email.to_string());
        self.mutate(
            busy,
            async move { client::deliver_invitation(&*calls, &id, &name, &email, &sender).await },
            move |this, delivery, cx| {
                if let Some(input) = input {
                    input.update(cx, |input, cx| input.set_text(String::new(), cx));
                    Self::close_dialog(this, (), cx);
                }
                toasts::success(
                    match delivery {
                        Delivery::Email => "Invitation sent.",
                        Delivery::Clipboard => "Email unavailable. Invite link copied instead.",
                    },
                    cx,
                );
            },
            cx,
        );
    }

    fn set_role(&mut self, user: Arc<str>, role: Role, cx: &mut Context<Self>) {
        let Some(panel) = &self.panel else {
            return;
        };
        let id = panel.workspace.id.clone();
        self.workspace_call(
            Busy::Role,
            "set_workspace_membership_role",
            json!({"p_workspace_id": id.as_ref(), "p_user_id": user.as_ref(), "p_role": role.as_str()}),
            |this, (), cx| this.calls.refresh_account(cx),
            cx,
        );
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = &self.panel else {
            return;
        };
        let Some(dialog) = panel.dialog.clone() else {
            return;
        };
        let id = panel.workspace.id.clone();
        let workspace = json!(id.as_ref());
        let request = panel
            .ownership
            .as_ref()
            .and_then(|requests| requests.as_ref().ok())
            .and_then(|requests| requests.first().cloned());
        match dialog {
            Dialog::Invite => self.invite(cx),
            Dialog::Remove(member) => self.workspace_call(
                Busy::Remove,
                "revoke_workspace_membership",
                json!({"p_workspace_id": workspace, "p_user_id": member.user_id.as_ref()}),
                Self::close_dialog,
                cx,
            ),
            Dialog::CancelInvite(invitation) => self.workspace_call(
                Busy::CancelInvite,
                "revoke_workspace_invitation",
                json!({"p_invitation_id": invitation.invitation_id.as_ref()}),
                Self::close_dialog,
                cx,
            ),
            Dialog::Transfer(member) => self.workspace_call(
                Busy::Transfer,
                "transfer_workspace_ownership",
                json!({"p_workspace_id": workspace, "p_user_id": member.user_id.as_ref()}),
                Self::close_dialog,
                cx,
            ),
            Dialog::AcceptTransfer => {
                if let Some(request) = request {
                    self.respond_transfer(&request, "accept", cx)
                }
            }
            Dialog::Delete => self.workspace_call(
                Busy::Delete,
                "delete_workspace",
                json!({"p_workspace_id": workspace}),
                Self::left,
                cx,
            ),
            Dialog::Leave => self.workspace_call(
                Busy::Leave,
                "leave_workspace",
                json!({"p_workspace_id": workspace}),
                Self::left,
                cx,
            ),
        }
    }

    /// Leaving or deleting drops the selection: the workspace is gone.
    fn left(this: &mut Self, _: (), cx: &mut Context<Self>) {
        this.selected = None;
        this.panel = None;
        this.calls.refresh_account(cx);
    }

    fn respond_transfer(
        &mut self,
        request: &OwnershipRequest,
        action: &'static str,
        cx: &mut Context<Self>,
    ) {
        let Some(panel) = &self.panel else {
            return;
        };
        let id = panel.workspace.id.clone();
        self.workspace_call(
            Busy::Respond,
            "respond_workspace_ownership_request",
            json!({"p_workspace_id": id.as_ref(), "p_request_id": request.id.as_ref(), "p_action": action}),
            |this, (), cx| {
                Self::close_dialog(this, (), cx);
                this.calls.refresh_account(cx);
            },
            cx,
        );
    }

    fn open_billing(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = &self.panel else {
            return;
        };
        let seats = panel
            .access
            .as_ref()
            .and_then(|access| access.as_ref().ok())
            .map_or(1, |access| access.used_seats.max(1));
        let params = vec![
            ("workspace_id", panel.workspace.id.to_string()),
            ("period", "monthly".to_owned()),
            ("quantity", seats.to_string()),
        ];
        let open = self.calls.open_web("/app/team-checkout", params, None);
        self.mutate(Busy::Billing, open, |_, (), _| {}, cx);
    }

    fn set_auto_join(&mut self, enabled: bool, cx: &mut Context<Self>) {
        let Some(panel) = &mut self.panel else {
            return;
        };
        panel.auto_join_error = None;
        let id = panel.workspace.id.clone();
        let calls = self.calls.clone();
        let generation = panel.generation;
        panel.busy = Some(Busy::AutoJoin);
        cx.spawn(async move |this, cx| {
            let result = client::call(
                &*calls,
                "set_workspace_email_auto_join",
                json!({"p_workspace_id": id.as_ref(), "p_enabled": enabled}),
            )
            .await;
            let _ = this.update(cx, |this, cx| {
                let Some(panel) = this.panel.as_mut().filter(|p| p.generation == generation) else {
                    return;
                };
                panel.busy = None;
                if let Err(error) = result {
                    panel.auto_join_error = Some(error.to_string());
                }
                this.load_panel(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn save_policy(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = &mut self.panel else {
            return;
        };
        let (Some(policy), Some(form)) = (&panel.policy, &panel.policy_form) else {
            return;
        };
        let caps = Caps::new(
            panel.workspace.role,
            panel
                .access
                .as_ref()
                .and_then(|access| access.as_ref().ok()),
        );
        let next = policy_to_save(
            policy,
            form.allow_link,
            form.allow_public,
            &form.retention.read(cx).buffer.text,
            form.require_sso,
            caps,
        );
        panel.policy_error = None;
        let id = panel.workspace.id.clone();
        let calls = self.calls.clone();
        self.mutate(
            Busy::Policy,
            async move {
                let result = client::set_policy(&*calls, &id, &next).await;
                Ok(result.err().map(|error| error.to_string()))
            },
            |this, error, _| {
                if let Some(panel) = &mut this.panel {
                    panel.policy_error = error;
                }
            },
            cx,
        );
    }

    fn claim_domain(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = &self.panel else {
            return;
        };
        let Some(form) = &panel.policy_form else {
            return;
        };
        let domain = form.domain.read(cx).buffer.text.trim().to_owned();
        if domain.is_empty() {
            return;
        }
        let id = panel.workspace.id.clone();
        self.workspace_call(
            Busy::Domain,
            "claim_workspace_domain",
            json!({"p_workspace_id": id.as_ref(), "p_domain": domain}),
            |_, (), _| {},
            cx,
        );
    }

    fn save_scim(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = &self.panel else {
            return;
        };
        let Some(form) = &panel.policy_form else {
            return;
        };
        let domain = form.domain.read(cx).buffer.text.trim().to_owned();
        let token = form.scim.read(cx).buffer.text.trim().to_owned();
        if domain.is_empty() || token.len() < 32 {
            return;
        }
        let id = panel.workspace.id.clone();
        let scim = form.scim.clone();
        self.workspace_call(
            Busy::Scim,
            "rotate_workspace_scim_token",
            json!({"p_workspace_id": id.as_ref(), "p_domain": domain, "p_token": token}),
            move |_, (), cx| scim.update(cx, |input, cx| input.set_text(String::new(), cx)),
            cx,
        );
    }

    fn slug_value(&self, cx: &App) -> Option<String> {
        let slug = self.panel.as_ref()?.slug.as_ref()?;
        Some(slug.input.read(cx).buffer.text.trim().to_lowercase())
    }

    /// Debounces the availability check by 300 ms, like `useDebounceValue`.
    fn slug_changed(&mut self, cx: &mut Context<Self>) {
        let Some(value) = self.slug_value(cx) else {
            return;
        };
        let Some(panel) = &mut self.panel else {
            return;
        };
        let Some(slug) = &mut panel.slug else {
            return;
        };
        slug.error = None;
        if !valid_slug(&value) || value == slug.current {
            slug.checked = None;
            return;
        }
        slug.checked = Some((value.clone(), None));
        let calls = self.calls.clone();
        let id = panel.workspace.id.clone();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            let still = this
                .update(cx, |this, cx| {
                    this.slug_value(cx).as_deref() == Some(&value)
                })
                .unwrap_or(false);
            if !still {
                return;
            }
            let result = client::slug_availability(&*calls, &id, &value).await;
            let _ = this.update(cx, |this, cx| {
                if let Some(slug) = this.panel.as_mut().and_then(|p| p.slug.as_mut())
                    && slug
                        .checked
                        .as_ref()
                        .is_some_and(|(checked, _)| *checked == value)
                {
                    slug.checked = Some((value, Some(result.map_err(|error| error.to_string()))));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn slug_status(&self, cx: &App) -> Option<SlugStatus> {
        let value = self.slug_value(cx)?;
        let slug = self.panel.as_ref()?.slug.as_ref()?;
        Some(if value.is_empty() {
            return None;
        } else if !valid_slug(&value) {
            SlugStatus::Invalid
        } else if value == slug.current {
            SlugStatus::Current
        } else {
            match &slug.checked {
                Some((checked, Some(Ok(answer)))) if *checked == value => match answer {
                    SlugAvailability::Available => SlugStatus::Available,
                    SlugAvailability::Taken => SlugStatus::Taken,
                    SlugAvailability::Invalid => SlugStatus::Invalid,
                },
                Some((checked, Some(Err(_)))) if *checked == value => SlugStatus::Error,
                _ => SlugStatus::Checking,
            }
        })
    }

    fn save_slug(&mut self, cx: &mut Context<Self>) {
        let status = self.slug_status(cx);
        let Some(value) = self.slug_value(cx) else {
            return;
        };
        if !valid_slug(&value)
            || matches!(
                status,
                Some(
                    SlugStatus::Current
                        | SlugStatus::Checking
                        | SlugStatus::Taken
                        | SlugStatus::Invalid
                )
            )
        {
            return;
        }
        let Some(panel) = &self.panel else {
            return;
        };
        let id = panel.workspace.id.clone();
        let calls = self.calls.clone();
        self.mutate(
            Busy::Slug,
            async move {
                let value_json = json!({"p_workspace_id": id.as_ref(), "p_slug": value});
                let result = calls.rpc("set_workspace_share_slug", value_json).await;
                Ok(result.map_err(|error| error.to_string()).and_then(|value| {
                    value[0]["workspace_share_slug"]
                        .as_str()
                        .map(String::from)
                        .ok_or_else(|| "Workspace request failed".to_owned())
                }))
            },
            |this, saved, cx| {
                if let Some(slug) = this.panel.as_mut().and_then(|p| p.slug.as_mut()) {
                    match saved {
                        Ok(saved) => {
                            slug.current = saved.clone();
                            slug.checked = None;
                            slug.input.update(cx, |input, cx| input.set_text(saved, cx));
                        }
                        Err(error) => slug.error = Some(error),
                    }
                }
            },
            cx,
        );
    }

    fn open_menu(&mut self, menu: Menu, cx: &mut Context<Self>) {
        if let Some(panel) = &mut self.panel {
            panel.menu = (panel.menu.as_ref() != Some(&menu)).then_some(menu);
            cx.notify();
        }
    }

    fn open_dialog(&mut self, dialog: Dialog, cx: &mut Context<Self>) {
        if let Some(panel) = &mut self.panel {
            panel.menu = None;
            panel.dialog_error = None;
            panel.dialog = Some(dialog);
            cx.notify();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SlugStatus {
    Checking,
    Available,
    Taken,
    Invalid,
    Current,
    Error,
}

fn viewer(cx: &App) -> Option<(Arc<str>, Option<Arc<str>>)> {
    account::current(cx)
        .user
        .map(|user| (user.id.clone(), user.email.clone()))
}

fn policy_form(policy: &Policy, cx: &mut Context<TeamsView>) -> PolicyForm {
    let input = |placeholder: &str, value: String, cx: &mut Context<TeamsView>| {
        cx.new(|cx| {
            let mut input = TextInput::new(placeholder, cx).inline();
            input.set_text(value, cx);
            input
        })
    };
    PolicyForm {
        allow_link: policy.allowed_share_scopes.iter().any(|s| s == "link"),
        allow_public: policy.allowed_share_scopes.iter().any(|s| s == "public"),
        require_sso: policy.require_sso,
        retention: input(
            "Keep forever",
            policy
                .retention_days
                .map(|days| days.to_string())
                .unwrap_or_default(),
            cx,
        ),
        domain: input("company.com", String::new(), cx),
        scim: cx.new(|cx| TextInput::new("", cx).inline().secret()),
    }
}

/// `compressWorkspaceLogo`: a centre-cropped 128px PNG data URL, at most 120,000 characters.
fn logo_data_url(bytes: &[u8]) -> anyhow::Result<String> {
    use base64::Engine;
    let image = image::load_from_memory(bytes)?;
    let side = image.width().min(image.height());
    anyhow::ensure!(side > 0, "image has no pixels");
    let square = image
        .crop_imm(
            (image.width() - side) / 2,
            (image.height() - side) / 2,
            side,
            side,
        )
        .resize_exact(128, 128, image::imageops::FilterType::Lanczos3);
    let mut png = std::io::Cursor::new(Vec::new());
    square.write_to(&mut png, image::ImageFormat::Png)?;
    let url = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png.into_inner())
    );
    anyhow::ensure!(url.len() <= 120_000, "compressed logo is too large");
    Ok(url)
}

// ---- Building blocks shared with the Sync page -------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Variant {
    Primary,
    Outline,
    Ghost,
    Destructive,
    /// Outline that turns destructive on hover (Sync's Disconnect).
    OutlineDestructive,
}

/// `Button`: `sm` is `h-7 px-2 text-xs`, the default `h-9 px-4 text-sm`.
pub(super) fn button(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    variant: Variant,
    small: bool,
    icon: Option<&'static str>,
    enabled: bool,
    colors: &Theme,
) -> Stateful<Div> {
    let (background, foreground, hover) = match variant {
        Variant::Primary => (
            Some(colors.primary),
            colors.primary_foreground,
            colors.primary.opacity(0.9),
        ),
        Variant::Destructive => (
            Some(colors.destructive),
            colors.destructive_foreground,
            colors.destructive.opacity(0.9),
        ),
        Variant::Outline => (Some(colors.background), colors.foreground, colors.accent),
        Variant::OutlineDestructive => (
            Some(colors.background),
            colors.foreground,
            colors.destructive.opacity(0.1),
        ),
        Variant::Ghost => (None, colors.foreground, colors.accent),
    };
    let outline = matches!(variant, Variant::Outline | Variant::OutlineDestructive);
    let destructive = colors.destructive;
    div()
        .id(id)
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .gap_2()
        .font_weight(MEDIUM)
        .text_color(foreground)
        .when(small, |view| view.h(px(28.)).px_2().text_xs())
        .when(!small, |view| view.h(px(36.)).px_4().text_sm())
        .when_some(background, |view, background| view.bg(background))
        .when(variant != Variant::Ghost, |view| view.shadow_xs())
        .when(outline, |view| view.border_1().border_color(colors.input))
        .when_some(icon, |view, icon| {
            view.child(controls::icon(
                icon,
                if small { 14. } else { 16. },
                foreground,
            ))
        })
        .child(label.into())
        .when(!enabled, |view| view.opacity(0.5))
        .when(enabled, |view| {
            view.cursor_pointer().hover(move |style| {
                let style = style.bg(hover);
                if variant == Variant::OutlineDestructive {
                    style.border_color(destructive).text_color(destructive)
                } else {
                    style
                }
            })
        })
}

/// A bordered single-line field (`Input` with `bg-card h-9 shadow-none`).
pub(super) fn field(input: &Entity<TextInput>, colors: &Theme) -> Div {
    div()
        .h(px(36.))
        .w_full()
        .px_3()
        .flex()
        .items_center()
        .border_1()
        .border_color(colors.input)
        .bg(colors.card)
        .text_sm()
        .child(div().w_full().child(input.clone()))
}

pub(super) fn spinner(size: f32, color: gpui::Hsla) -> gpui::Svg {
    controls::icon("product/LoaderCircleIcon.svg", size, color)
}

/// `DestructiveConfirmationDialog` (and the invite dialog) inside a glass panel.
#[allow(clippy::too_many_arguments)]
pub(super) fn confirm_dialog(
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    error: Option<String>,
    confirm: &'static str,
    destructive: bool,
    pending: bool,
    on_cancel: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    on_confirm: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    body: Option<AnyElement>,
    window: &Window,
    colors: &Theme,
) -> AnyElement {
    let description = description.into();
    list::modal(
        window,
        // Opaque: without a backdrop blur, the glass panel's 60% card lets page text through.
        list::glass_dialog(*colors)
            .bg(colors.card)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .text_center()
                    .text_size(px(13.))
                    .child(
                        div()
                            .line_height(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title.into()),
                    )
                    .when(!description.is_empty(), |view| {
                        view.child(div().w_full().line_height(px(17.7)).child(description))
                    })
                    .when_some(error.clone().filter(|_| body.is_none()), |view, error| {
                        view.child(
                            div()
                                .w_full()
                                .line_height(px(17.7))
                                .text_color(colors.destructive)
                                .child(error),
                        )
                    }),
            )
            .when_some(body, |view, body| {
                view.child(div().child(body).when_some(error, |view, error| {
                    view.child(
                        div()
                            .pt_2()
                            .text_center()
                            .text_xs()
                            .text_color(colors.destructive)
                            .child(error),
                    )
                }))
            })
            .child(
                div()
                    .grid()
                    .grid_cols(2)
                    .gap_2()
                    .child(
                        list::dialog_button("dialog-cancel", "Cancel", false, *colors)
                            .when(pending, |view| view.opacity(0.5))
                            .when(!pending, |view| view.on_click(on_cancel)),
                    )
                    .child(
                        list::dialog_button("dialog-confirm", confirm, destructive, *colors)
                            .when(pending, |view| view.opacity(0.5))
                            .when(!pending, |view| view.on_click(on_confirm)),
                    ),
            )
            .into_any_element(),
    )
}

fn logo_mark(logo: Option<&str>, size: f32, colors: &Theme) -> Div {
    if let Some(logo) = logo {
        return crate::workspace::contacts::avatar::contact_image(logo, "", size);
    }
    div()
        .size(px(size))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(colors.primary.opacity(0.1))
        .child(controls::icon(
            "product/Building03Icon.svg",
            size / 2.,
            colors.primary,
        ))
}

fn menu_item(
    id: impl Into<gpui::ElementId>,
    icon: Option<&'static str>,
    label: &'static str,
    destructive: bool,
    colors: &Theme,
) -> Stateful<Div> {
    let color = if destructive {
        colors.destructive
    } else {
        colors.popover_foreground
    };
    div()
        .id(id)
        .w_full()
        .px_2()
        .py(px(6.))
        .flex()
        .items_center()
        .gap_2()
        .text_sm()
        .text_color(color)
        .cursor_pointer()
        .hover(|style| style.bg(colors.accent))
        .when_some(icon, |view, icon| {
            view.child(controls::icon(icon, 16., color))
        })
        .child(label)
}

fn skeleton(colors: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .children((0..3).map(|_| div().h(px(44.)).bg(colors.muted)))
}

// ---- Rendering ----------------------------------------------------------------------------

impl TeamsView {
    fn render_invitations(&self, colors: &Theme, cx: &mut Context<Self>) -> Option<Div> {
        if self.invitations.is_empty() {
            return None;
        }
        let busy = self.responding.is_some();
        Some(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(controls::section_title("Invitations"))
                .children(self.invitations.iter().map(|invitation| {
                    let responding = self.responding.as_ref() == Some(&invitation.invitation_id);
                    let accept = invitation.clone();
                    let decline = invitation.clone();
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(logo_mark(invitation.logo.as_deref(), 32., colors))
                        .child(
                            div()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_sm()
                                        .line_height(px(20.))
                                        .font_weight(MEDIUM)
                                        .child(invitation.workspace_name.to_string()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .line_height(px(16.))
                                        .text_color(colors.muted_foreground)
                                        .child(match &invitation.invited_by_email {
                                            Some(email) => format!("Invited by {email}"),
                                            None => "Invited to join".into(),
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .ml_auto()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    button(
                                        SharedString::from(format!(
                                            "accept-{}",
                                            invitation.invitation_id
                                        )),
                                        "Accept",
                                        Variant::Primary,
                                        true,
                                        responding.then_some("product/LoaderCircleIcon.svg"),
                                        !busy,
                                        colors,
                                    )
                                    .when(!busy, |view| {
                                        view.on_click(cx.listener(move |this, _, _, cx| {
                                            this.respond(accept.clone(), true, cx)
                                        }))
                                    }),
                                )
                                .child(
                                    button(
                                        SharedString::from(format!(
                                            "decline-{}",
                                            invitation.invitation_id
                                        )),
                                        "Decline",
                                        Variant::Ghost,
                                        true,
                                        None,
                                        !busy,
                                        colors,
                                    )
                                    .when(!busy, |view| {
                                        view.on_click(cx.listener(move |this, _, _, cx| {
                                            this.respond(decline.clone(), false, cx)
                                        }))
                                    }),
                                ),
                        )
                })),
        )
    }

    fn render_tabs(
        &self,
        workspaces: &[MyWorkspace],
        colors: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let shown = self.selected_workspace().map(|workspace| workspace.id);
        let muted = colors.muted;
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(6.))
            .children(workspaces.iter().map(|workspace| {
                let selected = shown.as_ref() == Some(&workspace.id);
                let id = workspace.id.clone();
                div()
                    .id(SharedString::from(format!(
                        "workspace-tab-{}",
                        workspace.id
                    )))
                    .flex()
                    .items_center()
                    .gap_2()
                    .py(px(6.))
                    .pr_3()
                    .pl(px(6.))
                    .text_sm()
                    .line_height(px(20.))
                    .cursor_pointer()
                    .when(selected, |view| {
                        view.bg(colors.muted)
                            .text_color(colors.foreground)
                            .font_weight(MEDIUM)
                    })
                    .when(!selected, |view| {
                        view.text_color(colors.muted_foreground)
                            .hover(move |style| style.bg(muted.opacity(0.6)))
                    })
                    .child(logo_mark(workspace.logo.as_deref(), 20., colors))
                    .child(workspace.name.to_string())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.has_unsaved() {
                            return;
                        }
                        this.creating = false;
                        this.selected = Some(id.clone());
                        this.sync_panel(cx);
                        cx.notify();
                    }))
            }))
            .child(
                div()
                    .id("create-workspace-tab")
                    .size(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .when(self.creating, |view| view.bg(colors.muted))
                    .when(!self.creating, |view| {
                        view.hover(move |style| style.bg(muted.opacity(0.6)))
                    })
                    .child(controls::icon(
                        "product/PlusIcon.svg",
                        16.,
                        if self.creating {
                            colors.foreground
                        } else {
                            colors.muted_foreground
                        },
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.has_unsaved() {
                            return;
                        }
                        this.creating = true;
                        this.sync_panel(cx);
                        cx.notify();
                    })),
            )
    }

    /// `CreateWorkspaceForm` behind the Pro plan gate.
    fn render_create(&self, colors: &Theme, cx: &mut Context<Self>) -> Div {
        let pro = account::is_pro(cx);
        let name = self.create_name.read(cx).buffer.text.trim().to_owned();
        let enabled = !name.is_empty() && !self.create_busy;
        let form = div()
            .max_w(px(576.))
            .flex()
            .flex_col()
            .gap_4()
            .child(controls::section_title("Create a shared workspace"))
            .child(
                div()
                    .text_xs()
                    .line_height(px(20.))
                    .text_color(colors.muted_foreground)
                    .child("Invite teammates, share notes across the workspace, and manage who has access. Your personal notes stay private."),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(div().w(px(320.)).child(field(&self.create_name, colors)))
                    .child(
                        button(
                            "create-workspace",
                            "Create",
                            Variant::Primary,
                            true,
                            Some(if self.create_busy {
                                "product/LoaderCircleIcon.svg"
                            } else {
                                "product/PlusIcon.svg"
                            }),
                            enabled,
                            colors,
                        )
                        .when(enabled, |view| {
                            view.on_click(cx.listener(|this, _, _, cx| this.create(cx)))
                        }),
                    ),
            )
            .children(self.create_error.as_ref().map(|error| {
                div()
                    .text_xs()
                    .text_color(colors.destructive)
                    .child(error.clone())
            }));
        plan_gate(pro, form, cx.listener(|_, _, _, cx| cx.emit(PlanRequired)))
    }

    fn render_panel(
        &self,
        window: &mut Window,
        colors: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let panel = self.panel.as_ref()?;
        let workspace = &panel.workspace;
        let access = panel
            .access
            .as_ref()
            .and_then(|access| access.as_ref().ok());
        let caps = Caps::new(workspace.role, access);
        let viewer = self.viewer_id();
        let primary = viewer.as_ref() == Some(&workspace.owner_user_id);
        let busy = panel.busy;
        let ownership = panel
            .ownership
            .as_ref()
            .and_then(|requests| requests.as_ref().ok())
            .and_then(|requests| requests.first().cloned());

        let header = div()
            .flex()
            .items_center()
            .gap_4()
            .child(self.render_logo(caps.manage_workspace, busy == Some(Busy::Logo), colors, cx))
            .child(if caps.manage_workspace {
                div()
                    .w_full()
                    .max_w(px(384.))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        if event.keystroke.key == "escape"
                            && let Some(panel) = &this.panel
                        {
                            let name = panel.workspace.name.to_string();
                            panel
                                .name
                                .clone()
                                .update(cx, |input, cx| input.set_text(name, cx));
                            window.blur(cx);
                        }
                    }))
                    .child(field(&panel.name, colors))
            } else {
                div()
                    .truncate()
                    .text_sm()
                    .font_weight(MEDIUM)
                    .child(workspace.name.to_string())
            })
            .when(busy == Some(Busy::Rename), |view| {
                view.child(spinner(16., colors.muted_foreground))
            });

        let billing = caps.manager.then(|| {
            let pending = panel.access.is_none();
            div()
                .max_w(px(576.))
                .flex()
                .flex_col()
                .gap_3()
                .border_1()
                .border_color(colors.border)
                .p_4()
                .child(
                    div()
                        .child(if pending {
                            div().h(px(20.)).w(px(80.)).bg(colors.muted)
                        } else {
                            div()
                                .text_sm()
                                .line_height(px(20.))
                                .font_weight(MEDIUM)
                                .child(if caps.paid { "Pro plan" } else { "Start Pro" })
                        })
                        .child(
                            div()
                                .mt_1()
                                .text_xs()
                                .line_height(px(20.))
                                .text_color(colors.muted_foreground)
                                .child("Pro for every member, shared workspaces, roles, and centralized billing. $14 per person monthly or $140 yearly."),
                        ),
                )
                .child(
                    div().flex().child(
                        button(
                            "team-billing",
                            if pending {
                                ""
                            } else if caps.paid {
                                "Manage workspace billing"
                            } else {
                                "Continue to Pro checkout"
                            },
                            Variant::Primary,
                            true,
                            (pending || busy == Some(Busy::Billing))
                                .then_some("product/LoaderCircleIcon.svg"),
                            !pending && busy.is_none(),
                            colors,
                        )
                        .when(!pending && busy.is_none(), |view| {
                            view.on_click(cx.listener(|this, _, _, cx| this.open_billing(cx)))
                        }),
                    ),
                )
        });

        let members = self.render_members(panel, caps, primary, &ownership, colors, cx);
        let admin = self.render_admin(panel, caps, window, colors, cx);
        let footer = div()
            .min_w_0()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .child(
                div()
                    .min_w_0()
                    .text_xs()
                    .line_height(px(16.))
                    .text_color(colors.muted_foreground)
                    .child(if primary {
                        "Deleting removes the workspace for everyone. Transfer ownership first if you only want to leave."
                    } else {
                        "Leaving gives up your access to shared notes here."
                    }),
            )
            .child(
                button(
                    "leave-or-delete-workspace",
                    if primary {
                        "Delete workspace"
                    } else {
                        "Leave workspace"
                    },
                    Variant::Destructive,
                    true,
                    None,
                    busy.is_none(),
                    colors,
                )
                .when(busy.is_none(), |view| {
                    view.on_click(cx.listener(move |this, _, _, cx| {
                        this.open_dialog(if primary { Dialog::Delete } else { Dialog::Leave }, cx)
                    }))
                }),
            );

        Some(
            div()
                .flex()
                .flex_col()
                .gap_8()
                .child(header)
                .children(panel.error.as_ref().map(|error| {
                    div()
                        .text_xs()
                        .text_color(colors.destructive)
                        .child(error.clone())
                }))
                .children(billing)
                .child(members)
                .children(admin)
                .child(footer),
        )
    }

    fn render_logo(
        &self,
        manage: bool,
        pending: bool,
        colors: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let logo = self
            .panel
            .as_ref()
            .and_then(|panel| panel.workspace.logo.clone());
        let mark = logo_mark(logo.as_deref(), 40., colors);
        if !manage {
            return div().flex_shrink_0().child(mark);
        }
        div()
            .relative()
            .flex_shrink_0()
            .child(
                div()
                    .id("workspace-logo")
                    .group("workspace-logo")
                    .relative()
                    .cursor_pointer()
                    .child(mark)
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(gpui::black().opacity(0.4))
                            .when(!pending, |view| {
                                view.opacity(0.)
                                    .group_hover("workspace-logo", |style| style.opacity(1.))
                            })
                            .child(controls::icon(
                                if pending {
                                    "product/LoaderCircleIcon.svg"
                                } else {
                                    "product/Camera01Icon.svg"
                                },
                                16.,
                                gpui::white(),
                            )),
                    )
                    .when(!pending, |view| {
                        view.on_click(cx.listener(|this, _, _, cx| this.pick_logo(cx)))
                    }),
            )
            .when(logo.is_some() && !pending, |view| {
                view.child(
                    div()
                        .id("remove-workspace-logo")
                        .absolute()
                        .top(px(-4.))
                        .right(px(-4.))
                        .size(px(16.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .border_1()
                        .border_color(colors.border)
                        .bg(colors.background)
                        .shadow_xs()
                        .cursor_pointer()
                        .child(controls::icon(
                            "product/XIcon.svg",
                            10.,
                            colors.muted_foreground,
                        ))
                        .on_click(cx.listener(|this, _, _, cx| this.set_logo(None, cx))),
                )
            })
    }

    fn render_members(
        &self,
        panel: &Panel,
        caps: Caps,
        primary: bool,
        ownership: &Option<OwnershipRequest>,
        colors: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let viewer = self.viewer_id();
        let busy = panel.busy;
        let header = div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_3()
            .child(controls::section_title("Members"))
            .when(caps.manage_members, |view| {
                view.child(
                    button(
                        "add-members",
                        "Add members",
                        Variant::Primary,
                        true,
                        Some("product/PlusIcon.svg"),
                        true,
                        colors,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(panel) = &this.panel {
                            panel
                                .invite_email
                                .clone()
                                .update(cx, |input, cx| input.set_text(String::new(), cx));
                        }
                        this.open_dialog(Dialog::Invite, cx)
                    })),
                )
            });

        let auto_join = (primary && caps.manage_members).then(|| {
            let setting = panel.auto_join.as_ref().and_then(|value| value.as_ref().ok());
            let domain = setting.and_then(|setting| setting.domain.clone());
            let detail: Option<SharedString> = match (&domain, &panel.auto_join) {
                (Some(domain), _) => Some(format!("People with a verified @{domain} email join this team when they sign in. Billing starts when they join and is prorated for the remaining billing period.").into()),
                (None, Some(Ok(_))) => Some("Use a verified work email to enable automatic joining. Personal email providers such as Gmail are excluded.".into()),
                _ => None,
            };
            let enabled = setting.is_some_and(|setting| setting.enabled);
            let allowed = setting.is_some()
                && busy != Some(Busy::AutoJoin)
                && (domain.is_some() || enabled);
            let error = panel.auto_join_error.clone().or_else(|| {
                panel
                    .auto_join
                    .as_ref()
                    .and_then(|value| value.as_ref().err().cloned())
            });
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(controls::setting_row(
                    "Join automatically with a work email",
                    detail,
                    false,
                    controls::switch(
                        "auto-join-switch",
                        enabled,
                        allowed,
                        colors,
                        cx.listener(move |this, _, _, cx| this.set_auto_join(!enabled, cx)),
                    )
                    .aria_label("Join automatically with a work email"),
                    colors,
                ))
                .children(error.map(|error| {
                    div()
                        .text_sm()
                        .text_color(colors.destructive)
                        .child(error)
                }))
        });

        let transfer_status = ownership.as_ref().map(|request| {
            let request = request.clone();
            let target = viewer.as_ref() == Some(&request.target_user_id);
            let owner = viewer.as_ref() == Some(&request.owner_user_id);
            let enabled = busy.is_none();
            let decline = request.clone();
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_3()
                .border_1()
                .border_color(colors.border)
                .p_3()
                .text_sm()
                .child("Ownership transfer Pending. Current ownership and permissions remain unchanged until the proposed owner accepts.")
                .when(target, |view| {
                    view.child(
                        button("review-transfer", "Review transfer", Variant::Primary, true, None, enabled, colors)
                            .when(enabled, |view| view.on_click(cx.listener(|this, _, _, cx| this.open_dialog(Dialog::AcceptTransfer, cx)))),
                    )
                    .child(
                        button("decline-transfer", "Decline", Variant::Outline, true, None, enabled, colors)
                            .when(enabled, |view| view.on_click(cx.listener(move |this, _, _, cx| this.respond_transfer(&decline, "decline", cx)))),
                    )
                })
                .when(!target && owner, |view| {
                    view.child(
                        button("cancel-transfer", "Cancel transfer", Variant::Outline, true, None, enabled, colors)
                            .when(enabled, |view| view.on_click(cx.listener(move |this, _, _, cx| this.respond_transfer(&request, "cancel", cx)))),
                    )
                })
        });

        let table = match &panel.members {
            None => skeleton(colors),
            Some(Err(_)) => div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.muted_foreground)
                        .child("Could not load workspace members."),
                )
                .child(
                    button(
                        "members-retry",
                        "Try again",
                        Variant::Outline,
                        true,
                        None,
                        true,
                        colors,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.load_panel(cx))),
                ),
            Some(Ok(members)) => {
                self.render_table(panel, members, caps, primary, ownership, colors, cx)
            }
        };

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(header)
            .children(auto_join)
            .children(transfer_status)
            .child(table)
    }

    #[allow(clippy::too_many_arguments)]
    fn render_table(
        &self,
        panel: &Panel,
        members: &[Member],
        caps: Caps,
        primary: bool,
        ownership: &Option<OwnershipRequest>,
        colors: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let viewer = self.viewer_id();
        let border = colors.border;
        // Like an auto-layout table, columns never shrink below their content.
        let cell = || {
            div()
                .px_4()
                .py_3()
                .whitespace_nowrap()
                // Table cells centre their content vertically.
                .flex()
                .flex_col()
                .justify_center()
        };
        let row = |cells: [Div; 4]| {
            let [name, email, permissions, actions] = cells;
            div()
                .flex()
                .items_stretch()
                .child(name.flex_1().border_r_1().border_color(border))
                .child(email.flex_1().border_r_1().border_color(border))
                .child(
                    permissions
                        .w(px(192.))
                        .flex_shrink_0()
                        .whitespace_normal()
                        .border_r_1()
                        .border_color(border),
                )
                .child(actions.w(px(64.)).flex_shrink_0())
        };
        let head = |label: &'static str| {
            cell()
                .text_xs()
                .line_height(px(16.))
                .font_weight(MEDIUM)
                .child(label)
        };
        let transfer_disabled = panel.ownership.as_ref().is_none_or(|value| value.is_err())
            || ownership.is_some()
            || panel.busy == Some(Busy::Transfer);
        let mut rows = vec![
            row([head("Name"), head("Email"), head("Permissions"), cell()])
                .bg(colors.muted.opacity(0.4))
                .text_color(colors.muted_foreground)
                .border_b_1()
                .border_color(border),
        ];
        for member in members {
            let member_primary = member.user_id == panel.workspace.owner_user_id;
            let is_viewer = viewer.as_ref() == Some(&member.user_id);
            let controls = member_controls(
                member.role,
                member_primary,
                is_viewer,
                caps.manager.then_some(panel.workspace.role),
                primary,
                caps.manage_members,
            );
            let label = member.name.clone().unwrap_or_else(|| member.email.clone());
            let name = cell().child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(facehash(&label, 32.))
                    .child(
                        div()
                            .min_w_0()
                            .child(
                                div()
                                    .text_sm()
                                    .line_height(px(20.))
                                    .font_weight(MEDIUM)
                                    .whitespace_nowrap()
                                    .child(member.name.as_deref().unwrap_or("—").to_owned()),
                            )
                            .when(is_viewer, |view| {
                                view.child(
                                    div()
                                        .text_xs()
                                        .line_height(px(16.))
                                        .text_color(colors.muted_foreground)
                                        .child("You"),
                                )
                            }),
                    ),
            );
            let email = cell()
                .text_sm()
                .line_height(px(20.))
                .text_color(colors.muted_foreground)
                .child(member.email.to_string());
            let permissions = cell()
                .child(self.render_role(
                    panel,
                    member,
                    member_primary,
                    controls,
                    transfer_disabled,
                    colors,
                    cx,
                ))
                .when(member_primary && controls.edit_role, |view| {
                    view.child(
                        div()
                            .mt_1()
                            .text_xs()
                            .line_height(px(16.))
                            .text_color(colors.muted_foreground)
                            .child("Transfer primary ownership before changing your role."),
                    )
                })
                .when(
                    ownership
                        .as_ref()
                        .is_some_and(|request| request.target_user_id == member.user_id),
                    |view| {
                        view.child(
                            div()
                                .mt_1()
                                .text_xs()
                                .line_height(px(16.))
                                .text_color(colors.muted_foreground)
                                .child("Pending"),
                        )
                    },
                );
            let actions = cell().when(controls.remove, |view| {
                let menu = Menu::Member(member.user_id.clone());
                let open = panel.menu.as_ref() == Some(&menu);
                let target = member.clone();
                view.child(
                    div()
                        .relative()
                        .child(
                            self.dots(
                                SharedString::from(format!("member-actions-{}", member.user_id)),
                                true,
                                colors,
                            )
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.open_menu(menu.clone(), cx)),
                            ),
                        )
                        .when(open, |view| {
                            view.child(self.menu(
                                vec![
                                        menu_item(
                                            "remove-member",
                                            Some("product/TrashIcon.svg"),
                                            "Remove member",
                                            true,
                                            colors,
                                        )
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                this.open_dialog(Dialog::Remove(target.clone()), cx)
                                            }),
                                        ),
                                    ],
                                colors,
                                cx,
                            ))
                        }),
                )
            });
            rows.push(row([name, email, permissions, actions]));
        }
        if caps.manager {
            for invitation in panel.invitations.iter() {
                let menu = Menu::Invitation(invitation.invitation_id.clone());
                let open = panel.menu.as_ref() == Some(&menu);
                let enabled = !matches!(panel.busy, Some(Busy::Resend | Busy::CancelInvite));
                let resend = invitation.email.to_string();
                let cancel = invitation.clone();
                let mut items = Vec::new();
                if caps.manage_members {
                    items.push(
                        menu_item(
                            "resend-invitation",
                            Some("product/SentIcon.svg"),
                            "Resend invitation",
                            false,
                            colors,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.deliver(Busy::Resend, resend.clone(), None, cx)
                        })),
                    );
                }
                items.push(
                    menu_item(
                        "cancel-invitation",
                        Some("product/TrashIcon.svg"),
                        "Cancel invitation",
                        true,
                        colors,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_dialog(Dialog::CancelInvite(cancel.clone()), cx)
                    })),
                );
                rows.push(row([
                    cell().child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(facehash(&invitation.email, 32.))
                            .child(
                                div()
                                    .text_sm()
                                    .whitespace_nowrap()
                                    .text_color(colors.muted_foreground)
                                    .child("Invitation pending"),
                            ),
                    ),
                    cell()
                        .text_sm()
                        .line_height(px(20.))
                        .text_color(colors.muted_foreground)
                        .child(invitation.email.to_string()),
                    cell()
                        .text_sm()
                        .text_color(colors.muted_foreground)
                        .child("—"),
                    cell().child(
                        div()
                            .relative()
                            .child(
                                self.dots(
                                    SharedString::from(format!(
                                        "invitation-actions-{}",
                                        invitation.invitation_id
                                    )),
                                    enabled,
                                    colors,
                                )
                                .when(enabled, |view| {
                                    view.on_click(cx.listener(move |this, _, _, cx| {
                                        this.open_menu(menu.clone(), cx)
                                    }))
                                }),
                            )
                            .when(open, |view| view.child(self.menu(items, colors, cx))),
                    ),
                ]));
            }
        }
        let count = rows.len();
        div()
            .overflow_hidden()
            .border_1()
            .border_color(border)
            .children(rows.into_iter().enumerate().map(|(index, row)| {
                row.when(index > 0 && index + 1 < count, |row| {
                    row.border_b_1().border_color(border)
                })
            }))
    }

    #[allow(clippy::too_many_arguments)]
    fn render_role(
        &self,
        panel: &Panel,
        member: &Member,
        member_primary: bool,
        controls: MemberControls,
        transfer_disabled: bool,
        colors: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let label = if member_primary {
            "Primary owner"
        } else {
            member.role.label()
        };
        if !controls.edit_role {
            return div()
                .text_xs()
                .line_height(px(16.))
                .text_color(colors.muted_foreground)
                .child(label);
        }
        let menu = Menu::Role(member.user_id.clone());
        let open = panel.menu.as_ref() == Some(&menu);
        let enabled = panel.busy != Some(Busy::Role);
        let mut items = Vec::new();
        let mut choice = |key: &'static str,
                          label: &'static str,
                          selected: bool,
                          disabled: bool,
                          action: Option<Role>| {
            let user = member.user_id.clone();
            let target = member.clone();
            items.push(
                controls::menu_item(
                    SharedString::from(format!("role-{key}")),
                    label,
                    None,
                    selected,
                    colors,
                )
                .when(disabled, |view| view.opacity(0.5))
                .when(!disabled, |view| {
                    view.on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(panel) = &mut this.panel {
                            panel.menu = None;
                        }
                        match action {
                            _ if selected => cx.notify(),
                            Some(role) => this.set_role(user.clone(), role, cx),
                            None => this.open_dialog(Dialog::Transfer(target.clone()), cx),
                        }
                    }))
                }),
            );
        };
        if controls.transfer || member_primary {
            choice(
                "primary",
                "Primary owner",
                member_primary,
                transfer_disabled && !member_primary,
                None,
            );
        }
        let current = |role| !member_primary && member.role == role;
        if panel.workspace.role == Role::Owner {
            choice(
                "owner",
                "Owner",
                current(Role::Owner),
                member_primary,
                Some(Role::Owner),
            );
        }
        choice(
            "admin",
            "Admin",
            current(Role::Admin),
            member_primary,
            Some(Role::Admin),
        );
        choice(
            "member",
            "Member",
            current(Role::Member),
            member_primary,
            Some(Role::Member),
        );
        div()
            .relative()
            .w(px(160.))
            .child(
                controls::select_trigger(
                    SharedString::from(format!("role-select-{}", member.user_id)),
                    Some(label.into()),
                    "",
                    enabled,
                    colors,
                )
                .aria_label("Role")
                .h(px(32.))
                .when(enabled, |view| {
                    view.on_click(
                        cx.listener(move |this, _, _, cx| this.open_menu(menu.clone(), cx)),
                    )
                }),
            )
            .when(open, |view| view.child(self.select_menu(items, colors, cx)))
    }

    fn dots(&self, id: SharedString, enabled: bool, colors: &Theme) -> Stateful<Div> {
        div()
            .id(id)
            .size(px(32.))
            .flex()
            .items_center()
            .justify_center()
            .child(controls::icon(
                "product/MoreHorizontalIcon.svg",
                16.,
                colors.foreground,
            ))
            .when(!enabled, |view| view.opacity(0.5))
            .when(enabled, |view| {
                view.cursor_pointer().hover(|style| style.bg(colors.accent))
            })
    }

    fn menu(&self, items: Vec<Stateful<Div>>, colors: &Theme, cx: &mut Context<Self>) -> Div {
        controls::dropdown(
            controls::menu_panel(192., colors)
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    if let Some(panel) = &mut this.panel {
                        panel.menu = None;
                        cx.notify();
                    }
                }))
                .children(items),
            true,
        )
    }

    fn select_menu(
        &self,
        items: Vec<Stateful<Div>>,
        colors: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        controls::dropdown(
            controls::menu_panel(160., colors)
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    if let Some(panel) = &mut this.panel {
                        panel.menu = None;
                        cx.notify();
                    }
                }))
                .children(items),
            false,
        )
    }

    fn render_admin(
        &self,
        panel: &Panel,
        caps: Caps,
        _window: &mut Window,
        colors: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        // Upcoming bot attendance needs an enterprise capture server, which the native app
        // has no configuration for, so it never counts towards the Admin section.
        if !(caps.manage_policies || caps.custom_subdomain || caps.view_usage) {
            return None;
        }
        let small = |label: &'static str| {
            div()
                .text_sm()
                .line_height(px(20.))
                .font_weight(MEDIUM)
                .child(label)
        };
        let note = |text: &'static str| {
            div()
                .text_xs()
                .line_height(px(20.))
                .text_color(colors.muted_foreground)
                .child(text)
        };
        let busy = panel.busy;
        let policy = panel.policy_form.as_ref().filter(|_| panel.policy.is_some()).map(|form| {
            let gated = caps.sso || caps.scim || caps.retention;
            let domain = form.domain.read(cx).buffer.text.trim().to_owned();
            let token = form.scim.read(cx).buffer.text.trim().len();
            let allow_link = form.allow_link;
            let allow_public = form.allow_public;
            let require_sso = form.require_sso;
            let enterprise = div()
                .flex()
                .flex_col()
                .gap_6()
                .child(controls::setting_row(
                    "Require SSO",
                    Some("Members on a claimed email domain must sign in with SSO instead of Google, GitHub, or email.".into()),
                    false,
                    controls::switch("require-sso", require_sso, true, colors, cx.listener(move |this, _, _, cx| {
                        if let Some(form) = this.panel.as_mut().and_then(|p| p.policy_form.as_mut()) {
                            form.require_sso = !require_sso;
                            cx.notify();
                        }
                    })).aria_label("Require SSO"),
                    colors,
                ))
                .child(
                    div()
                        .max_w(px(384.))
                        .flex()
                        .flex_col()
                        .gap_1()
                        .text_sm()
                        .child("Retention (days)")
                        .child(field(&form.retention, colors)),
                )
                .child(
                    div()
                        .grid()
                        .grid_cols(2)
                        .gap_6()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(div().flex().flex_col().gap_1().text_sm().child("Claim email domain").child(field(&form.domain, colors)))
                                .child(div().flex().child(
                                    button("verify-domain", "Verify domain", Variant::Outline, true, None, !domain.is_empty() && busy != Some(Busy::Domain), colors)
                                        .when(!domain.is_empty() && busy.is_none(), |view| view.on_click(cx.listener(|this, _, _, cx| this.claim_domain(cx)))),
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(div().flex().flex_col().gap_1().text_sm().child("SCIM bearer token").child(field(&form.scim, colors)))
                                .child(div().flex().child({
                                    let enabled = !domain.is_empty() && token >= 32 && busy != Some(Busy::Scim);
                                    button("save-scim", "Save SCIM token", Variant::Outline, true, None, enabled, colors)
                                        .when(enabled && busy.is_none(), |view| view.on_click(cx.listener(|this, _, _, cx| this.save_scim(cx))))
                                })),
                        ),
                );
            div()
                .flex()
                .flex_col()
                .gap_6()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(small("Policies"))
                        .child(note("These rules apply to every member. Sharing changes fail closed on the server.")),
                )
                .child(controls::setting_row(
                    "Allow anyone-with-the-link sharing",
                    None,
                    false,
                    controls::switch("allow-link", allow_link, true, colors, cx.listener(move |this, _, _, cx| {
                        if let Some(form) = this.panel.as_mut().and_then(|p| p.policy_form.as_mut()) {
                            form.allow_link = !allow_link;
                            cx.notify();
                        }
                    })).aria_label("Allow anyone-with-the-link sharing"),
                    colors,
                ))
                .child(controls::setting_row(
                    "Allow public indexing",
                    None,
                    false,
                    controls::switch("allow-public", allow_public, true, colors, cx.listener(move |this, _, _, cx| {
                        if let Some(form) = this.panel.as_mut().and_then(|p| p.policy_form.as_mut()) {
                            form.allow_public = !allow_public;
                            cx.notify();
                        }
                    })).aria_label("Allow public indexing"),
                    colors,
                ))
                .child(plan_gate(gated, enterprise, cx.listener(|_, _, _, cx| cx.emit(PlanRequired))))
                .child(div().flex().child(
                    button(
                        "save-policies",
                        "Save policies",
                        Variant::Primary,
                        true,
                        (busy == Some(Busy::Policy)).then_some("product/LoaderCircleIcon.svg"),
                        busy.is_none(),
                        colors,
                    )
                    .when(busy.is_none(), |view| view.on_click(cx.listener(|this, _, _, cx| this.save_policy(cx)))),
                ))
                .children(panel.policy_error.as_ref().map(|error| div().text_xs().text_color(colors.destructive).child(error.clone())))
        });

        let domain = caps.custom_subdomain.then(|| {
            let status = self.slug_status(cx);
            let slug = panel.slug.as_ref();
            let can_save = matches!(status, Some(SlugStatus::Available | SlugStatus::Error));
            let emerald = if colors.background.l < 0.5 {
                gpui::rgb(0x34d399)
            } else {
                gpui::rgb(0x059669)
            };
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(small("Sharing domain"))
                .child(
                    div()
                        .text_xs()
                        .line_height(px(16.))
                        .text_color(colors.muted_foreground)
                        .child("Use this domain for links shared from this workspace."),
                )
                .child(
                    div()
                        .h(px(36.))
                        .flex()
                        .items_center()
                        .overflow_hidden()
                        .border_1()
                        .border_color(colors.input)
                        .bg(colors.card)
                        .text_sm()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .px_3()
                                .children(slug.map(|slug| slug.input.clone())),
                        )
                        .child(
                            div()
                                .h_full()
                                .px_3()
                                .flex()
                                .items_center()
                                .border_l_1()
                                .border_color(colors.input)
                                .bg(colors.muted)
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child(".anarlog.so"),
                        ),
                )
                .children(status.map(|status| {
                    let (text, color) = match status {
                        SlugStatus::Checking => ("Checking availability…", colors.muted_foreground),
                        SlugStatus::Available => ("Available", emerald.into()),
                        SlugStatus::Taken => ("Already taken", colors.destructive),
                        SlugStatus::Invalid => (
                            "Use 3–63 lowercase letters, numbers, or hyphens.",
                            colors.destructive,
                        ),
                        SlugStatus::Current => ("Current domain", colors.muted_foreground),
                        SlugStatus::Error => (
                            "Couldn’t check availability. You can still try Save.",
                            colors.muted_foreground,
                        ),
                    };
                    div()
                        .text_xs()
                        .line_height(px(16.))
                        .text_color(color)
                        .child(text)
                }))
                .child(
                    div().flex().child(
                        button(
                            "save-subdomain",
                            "Save subdomain",
                            Variant::Outline,
                            true,
                            (busy == Some(Busy::Slug)).then_some("product/LoaderCircleIcon.svg"),
                            can_save && busy.is_none(),
                            colors,
                        )
                        .when(can_save && busy.is_none(), |view| {
                            view.on_click(cx.listener(|this, _, _, cx| this.save_slug(cx)))
                        }),
                    ),
                )
                .children(
                    slug.and_then(|slug| slug.error.clone())
                        .map(|error| div().text_xs().text_color(colors.destructive).child(error)),
                )
        });

        let usage = panel.usage.as_ref().filter(|_| caps.view_usage).map(|usage| {
            let seats = match usage.seat_limit {
                Some(limit) => format!("{} / {limit}", usage.used_seats),
                None => usage.used_seats.to_string(),
            };
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(small("Usage"))
                .child(note("Workspace activity from metadata only. Note content stays unreadable on the server."))
                .child(
                    div()
                        .max_w(px(512.))
                        .grid()
                        .grid_cols(2)
                        .gap_x_6()
                        .gap_y_3()
                        .text_sm()
                        .children(
                            [
                                ("Members", usage.member_count.to_string()),
                                ("Seats", seats),
                                ("Devices", usage.enrolled_devices.to_string()),
                                ("Shares (30d)", usage.shares_created_30d.to_string()),
                            ]
                            .into_iter()
                            .map(|(label, value)| {
                                div()
                                    .child(div().text_xs().line_height(px(16.)).text_color(colors.muted_foreground).child(label))
                                    .child(div().mt_1().font_weight(MEDIUM).child(value))
                            }),
                        ),
                )
        });

        Some(
            div()
                .flex()
                .flex_col()
                .gap_8()
                .child(controls::section_title("Admin"))
                .children(policy)
                .children(domain)
                .children(usage),
        )
    }

    fn render_dialog(
        &self,
        window: &Window,
        colors: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let panel = self.panel.as_ref()?;
        let dialog = panel.dialog.as_ref()?;
        let name = panel.workspace.name.to_string();
        let pending = panel.busy.is_some();
        let cancel = cx.listener(|this, _, _, cx| {
            if let Some(panel) = &mut this.panel
                && panel.busy.is_none()
            {
                panel.dialog = None;
                cx.notify();
            }
        });
        let confirm = cx.listener(|this, _, _, cx| this.confirm(cx));
        let error = panel.dialog_error.clone();
        Some(match dialog {
            Dialog::Invite => {
                let email = panel.invite_email.read(cx).buffer.text.trim().is_empty();
                confirm_dialog(
                    "Add members",
                    "",
                    error,
                    "Add members",
                    false,
                    pending || email,
                    cancel,
                    confirm,
                    Some(
                        field(&panel.invite_email, colors)
                            .bg(colors.background)
                            .into_any_element(),
                    ),
                    window,
                    colors,
                )
            }
            Dialog::Remove(member) => confirm_dialog(
                "Remove member?",
                format!(
                    "Remove {} from {name}? They will lose access to this workspace.",
                    member.email
                ),
                error,
                "Remove member",
                true,
                pending,
                cancel,
                confirm,
                None,
                window,
                colors,
            ),
            Dialog::CancelInvite(invitation) => confirm_dialog(
                "Cancel invitation?",
                format!(
                    "Cancel the invitation for {}? They will no longer be able to join using this invitation.",
                    invitation.email
                ),
                error,
                "Cancel invitation",
                true,
                pending,
                cancel,
                confirm,
                None,
                window,
                colors,
            ),
            Dialog::Transfer(member) => confirm_dialog(
                "Request ownership transfer?",
                format!(
                    "{} must accept before becoming Primary owner. Until then, ownership and permissions stay the same. After acceptance, you remain an Owner and they can delete the workspace.",
                    member.email
                ),
                error,
                "Request transfer",
                true,
                pending,
                cancel,
                confirm,
                None,
                window,
                colors,
            ),
            Dialog::AcceptTransfer => confirm_dialog(
                "Accept workspace ownership?",
                format!(
                    "You will become the Primary owner of {name}, with control over its members, billing, and deletion. The current Primary owner will remain an Owner."
                ),
                error,
                "Accept ownership",
                true,
                pending,
                cancel,
                confirm,
                None,
                window,
                colors,
            ),
            Dialog::Delete => confirm_dialog(
                format!("Delete {name} for everyone?"),
                "Deleting removes the workspace for everyone. Transfer ownership first if you only want to leave.",
                error,
                "Delete workspace",
                true,
                pending,
                cancel,
                confirm,
                None,
                window,
                colors,
            ),
            Dialog::Leave => confirm_dialog(
                format!("Leave {name}?"),
                "",
                error,
                "Leave workspace",
                true,
                pending,
                cancel,
                confirm,
                None,
                window,
                colors,
            ),
        })
    }
}

impl Render for TeamsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        if let Some(panel) = &self.panel
            && panel.blur.is_none()
        {
            let focus = panel.name.focus_handle(cx);
            let blur = cx.on_blur(&focus, window, |this, _, cx| this.submit_rename(cx));
            if let Some(panel) = &mut self.panel {
                panel.blur = Some(blur);
            }
        }
        let content = match &self.workspaces {
            None => Some(skeleton(&colors)),
            Some(Ok(workspaces)) if !workspaces.is_empty() => {
                let workspaces = workspaces.clone();
                Some(
                    div()
                        .flex()
                        .flex_col()
                        .gap_8()
                        .child(self.render_tabs(&workspaces, &colors, cx))
                        .children(if self.creating {
                            Some(self.render_create(&colors, cx))
                        } else {
                            self.render_panel(window, &colors, cx)
                        }),
                )
            }
            _ => Some(self.render_create(&colors, cx)),
        };
        let dialog = self.render_dialog(window, &colors, cx);
        div()
            .flex()
            .flex_col()
            .gap_8()
            .child(controls::page_title("Teams"))
            .children(self.render_invitations(&colors, cx))
            .children(content)
            .children(dialog)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn access(tier: WorkspaceTier, capabilities: &[&str]) -> Access {
        Access {
            role: Role::Owner,
            tier,
            capabilities: capabilities.iter().map(|c| (*c).to_owned()).collect(),
            used_seats: 1,
        }
    }

    #[test]
    fn admins_cannot_change_or_remove_owners_and_only_the_primary_owner_transfers() {
        let admin = |member, primary| {
            member_controls(member, primary, false, Some(Role::Admin), false, true)
        };
        assert_eq!(
            admin(Role::Member, false),
            MemberControls {
                edit_role: true,
                remove: true,
                transfer: false
            }
        );
        assert!(!admin(Role::Owner, false).edit_role && !admin(Role::Owner, false).remove);
        assert!(!admin(Role::Admin, false).edit_role);

        let owner = |member, primary, viewer_primary| {
            member_controls(
                member,
                primary,
                false,
                Some(Role::Owner),
                viewer_primary,
                true,
            )
        };
        // A secondary owner cannot touch another owner; the primary owner can, but never
        // removes themselves or loses primary status by removal.
        assert!(!owner(Role::Owner, false, false).edit_role);
        assert!(owner(Role::Owner, false, true).remove && owner(Role::Owner, false, true).transfer);
        assert!(!owner(Role::Owner, true, false).remove);
        let viewer = member_controls(Role::Owner, true, true, Some(Role::Owner), true, true);
        assert!(viewer.edit_role && !viewer.remove && !viewer.transfer);
        // Without the plan's member management, nothing is editable.
        assert_eq!(
            member_controls(Role::Member, false, false, Some(Role::Owner), true, false),
            MemberControls {
                edit_role: false,
                remove: false,
                transfer: false
            }
        );
        // Enterprise-only admin controls stay hidden on a Team plan.
        let team = Caps::new(
            Role::Owner,
            Some(&access(
                WorkspaceTier::Team,
                &["team.manage_members", "team.manage_policies"],
            )),
        );
        assert!(team.manage_members && team.paid && !team.manage_policies);
        assert!(
            !Caps::new(
                Role::Member,
                Some(&access(WorkspaceTier::Enterprise, &["team.manage_members"]))
            )
            .manage_members
        );
    }

    #[test]
    fn saving_policies_keeps_settings_the_plan_cannot_configure() {
        let current = Policy {
            allowed_share_scopes: vec!["restricted".into(), "workspace".into(), "public".into()],
            default_share_scope: "workspace".into(),
            retention_days: Some(30),
            model_training_opt_out: true,
            consent_notification_enabled: false,
            require_sso: true,
        };
        let limited = Caps::default();
        let saved = policy_to_save(&current, true, false, "", false, limited);
        assert_eq!(
            saved.allowed_share_scopes,
            ["restricted", "workspace", "link"]
        );
        assert_eq!((saved.retention_days, saved.require_sso), (Some(30), true));
        assert!(!saved.consent_notification_enabled);

        let enterprise = Caps {
            retention: true,
            sso: true,
            ..Caps::default()
        };
        let saved = policy_to_save(&current, false, false, " ", false, enterprise);
        assert_eq!((saved.retention_days, saved.require_sso), (None, false));
        assert_eq!(
            policy_to_save(&current, false, false, "90", true, enterprise).retention_days,
            Some(90)
        );
    }
}
