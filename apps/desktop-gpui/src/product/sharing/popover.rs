//! Shipping's note share popover: `SessionShareButton` (`session-sharing/index.tsx`) with its
//! draft panel, signed-out gate, Pro upsell and management panel, the invite form, general
//! access select and copy-link split button.

use std::{sync::Arc, time::Duration};

use desktop_runtime::{CancellationToken, RuntimeHandle, ServiceError, SessionId};
use gpui::{
    Animation, AnimationExt, AnyElement, App, Bounds, ClickEvent, Context, Div, Entity,
    EventEmitter, FocusHandle, FontWeight, Hsla, Pixels, Render, SharedString, Stateful,
    Subscription, Transformation, Window, anchored, canvas, deferred, div, img, percentage,
    prelude::*, px, svg,
};
use serde_json::{Value, json};

use super::{
    SharingCloud,
    model::{
        AccessEntry, Capability, EntryKind, GeneralAccess, Invite, ManagedShare, Recipient,
        ShareWorkspace, default_general_access, is_invite_email, managed, remember,
    },
};
use crate::{
    contracts::ProductRoute,
    product::{account, cloud::ShareCommand, services::Scope},
    ui::{
        input::{InputEvent, TextInput},
        theme::{Theme, theme},
    },
    workspace::contacts::avatar::facehash,
};

const PARTICIPANTS_SQL: &str = "SELECT COALESCE(NULLIF(h.name, ''), p.display_name) AS name,
      COALESCE(NULLIF(h.email, ''), p.email) AS email
    FROM session_participants p LEFT JOIN humans h ON h.id = p.human_id AND h.deleted_at IS NULL
    WHERE p.session_id = ? AND p.source <> 'excluded' AND p.deleted_at IS NULL
    ORDER BY name, email, p.id LIMIT 256";
const HUMANS_SQL: &str = "SELECT name, email FROM humans
    WHERE deleted_at IS NULL AND email <> '' ORDER BY name COLLATE NOCASE, id LIMIT 2000";
const WORKSPACES_SQL: &str = "SELECT w.id, w.name FROM workspaces w
    JOIN workspace_memberships m ON m.workspace_id = w.id AND m.user_id = ? AND m.deleted_at IS NULL
    WHERE w.kind = 'shared' AND w.deleted_at IS NULL
    ORDER BY w.name COLLATE NOCASE, w.id";
const DEFAULT_ACCESS_SQL: &str = "SELECT COALESCE(
      (SELECT value_json FROM app_settings WHERE id = 'default_meeting_share_access'),
      (SELECT value_json FROM synced_preferences WHERE id = 'default_meeting_share_access')
    ) AS value_json";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Invite,
    Email,
    Slack,
}

#[derive(Clone, PartialEq, Eq)]
enum Menu {
    Access,
    More,
    Entry(Arc<str>),
}

#[derive(Clone, PartialEq, Eq)]
enum Pending {
    Invite,
    CopyLink,
    Access,
    Entry(Arc<str>),
    SignIn,
    OpenWeb,
    KeepDesktop,
}

pub enum SharePopoverEvent {
    Dismiss,
    Open(ProductRoute),
}

#[derive(Default)]
struct Areas {
    trigger: Option<Bounds<Pixels>>,
    panel: Option<Bounds<Pixels>>,
    menu: Option<Bounds<Pixels>>,
}

pub struct SharePopover {
    session: SessionId,
    runtime: RuntimeHandle,
    query: Entity<TextInput>,
    email_query: Entity<TextInput>,
    invite: Invite,
    email: Invite,
    humans: Arc<[Recipient]>,
    workspaces: Arc<[ShareWorkspace]>,
    default_access: Arc<str>,
    mode: Mode,
    menu: Option<Menu>,
    pending: Option<Pending>,
    /// The cloud read of this note's share is in flight.
    loading: bool,
    load_failed: bool,
    error: Option<SharedString>,
    /// The access the user just picked, shown until the refreshed share confirms it.
    optimistic: Option<GeneralAccess>,
    areas: Areas,
    focus: FocusHandle,
    cancel: CancellationToken,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<SharePopoverEvent> for SharePopover {}

impl SharePopover {
    pub fn new(
        session: SessionId,
        runtime: RuntimeHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let query = cx.new(|cx| TextInput::new("Email or name", cx).inline());
        let email_query = cx.new(|cx| TextInput::new("Email or name", cx).inline());
        let subscriptions = vec![
            cx.subscribe(&query, |this, _, event, cx| match event {
                InputEvent::Submitted => {
                    let typed = this.query.read(cx).buffer.text.clone();
                    if this.invite.commit(&typed) {
                        this.query.update(cx, |q, cx| q.set_text(String::new(), cx));
                    }
                    cx.notify();
                }
                _ => cx.notify(),
            }),
            cx.subscribe(&email_query, |this, _, event, cx| match event {
                InputEvent::Submitted => {
                    let typed = this.email_query.read(cx).buffer.text.clone();
                    if this.email.commit(&typed) {
                        this.email_query
                            .update(cx, |q, cx| q.set_text(String::new(), cx));
                    }
                    cx.notify();
                }
                _ => cx.notify(),
            }),
            cx.observe_global::<account::Account>(|this, cx| {
                this.load(cx);
                cx.notify();
            }),
        ];
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let mut this = Self {
            session,
            runtime,
            query,
            email_query,
            invite: Invite::default(),
            email: Invite::default(),
            humans: Arc::from([]),
            workspaces: Arc::from([]),
            default_access: "me".into(),
            mode: Mode::Invite,
            menu: None,
            pending: None,
            loading: false,
            load_failed: false,
            error: None,
            optimistic: None,
            areas: Areas::default(),
            focus,
            cancel: CancellationToken::new(),
            _subscriptions: subscriptions,
        };
        this.load(cx);
        this
    }

    /// Reads the note's people and the account's shared workspaces from the library, then
    /// the share itself from the cloud.
    fn load(&mut self, cx: &mut Context<Self>) {
        self.cancel.cancel();
        self.cancel = CancellationToken::new();
        let account = account::current(cx);
        let user = account.user.as_ref().map(|user| user.id.clone());
        let owner = account
            .user
            .as_ref()
            .and_then(|user| user.email.clone())
            .map(|email| email.to_string())
            .unwrap_or_default();
        let session = self.session.clone();
        let local = self
            .runtime
            .read(self.cancel.clone(), move |services| async move {
                let run = |sql: &'static str, params: Vec<Value>| {
                    let executor = services.executor.clone();
                    async move {
                        executor
                            .execute(sql.into(), params)
                            .await
                            .map_err(|error| ServiceError::Failed(error.to_string().into()))
                    }
                };
                let people = |rows: Vec<Value>| -> Vec<Recipient> {
                    rows.iter()
                        .map(|row| Recipient {
                            email: row["email"].as_str().unwrap_or_default().into(),
                            name: row["name"].as_str().unwrap_or_default().into(),
                        })
                        .collect()
                };
                let participants = people(run(PARTICIPANTS_SQL, vec![json!(session.0)]).await?);
                let humans = people(run(HUMANS_SQL, Vec::new()).await?);
                let workspaces = match user {
                    Some(user) => run(WORKSPACES_SQL, vec![json!(user)])
                        .await?
                        .iter()
                        .map(|row| ShareWorkspace {
                            id: row["id"].as_str().unwrap_or_default().into(),
                            name: row["name"].as_str().unwrap_or_default().into(),
                        })
                        .collect(),
                    None => Vec::new(),
                };
                let access = run(DEFAULT_ACCESS_SQL, Vec::new())
                    .await?
                    .first()
                    .and_then(|row| row["value_json"].as_str())
                    .and_then(|value| serde_json::from_str::<String>(value).ok())
                    .unwrap_or_else(|| "me".into());
                Ok((participants, humans, workspaces, access))
            });
        let cloud = cx
            .try_global::<SharingCloud>()
            .map(|cloud| cloud.0.clone())
            .filter(|_| account.signed_in());
        let scope = Scope {
            account_id: account.user.as_ref().map(|user| user.id.clone()),
            session_id: Some(self.session.clone()),
            ..Scope::default()
        };
        self.loading = cloud.is_some();
        self.load_failed = false;
        let cancel = self.cancel.clone();
        cx.spawn(async move |this, cx| {
            if let Ok(reply) = local
                && let Ok((participants, humans, workspaces, access)) = reply.receive().await
            {
                let _ = this.update(cx, |this, cx| {
                    let signed_in = !owner.is_empty() || account.signed_in();
                    let workspaces: Arc<[ShareWorkspace]> = if signed_in {
                        workspaces.into()
                    } else {
                        Arc::from([])
                    };
                    for invite in [&mut this.invite, &mut this.email] {
                        invite.participants = participants.clone();
                        invite.excluded = vec![owner.clone()];
                    }
                    this.humans = humans.into();
                    this.workspaces = workspaces;
                    this.default_access = access.into();
                    this.exclude_invited(cx);
                    cx.notify();
                });
            }
            let Some(cloud) = cloud else {
                return;
            };
            let result = cloud.share_overview(scope).await;
            if cancel.is_cancelled() {
                return;
            }
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(share) => remember(this.session.clone(), share, cx),
                    Err(_) => this.load_failed = true,
                }
                this.exclude_invited(cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// People who already have access are not suggested again.
    fn exclude_invited(&mut self, cx: &App) {
        let owner = account::current(cx)
            .user
            .and_then(|user| user.email)
            .map(|email| email.to_string())
            .unwrap_or_default();
        let mut excluded = vec![owner];
        if let Some(share) = managed(&self.session, cx) {
            excluded.extend(
                share
                    .access
                    .iter()
                    .filter_map(|entry| entry.email.as_ref().map(|email| email.to_string())),
            );
        }
        self.invite.excluded = excluded;
    }

    /// The trigger the popover hangs from; clicks on it toggle instead of dismissing.
    pub fn set_trigger(&mut self, bounds: Bounds<Pixels>) {
        self.areas.trigger = Some(bounds);
    }

    fn dismiss_if_outside(&mut self, position: gpui::Point<Pixels>, cx: &mut Context<Self>) {
        let areas = &self.areas;
        let inside = [areas.trigger, areas.panel, areas.menu]
            .into_iter()
            .flatten()
            .any(|bounds| bounds.contains(&position));
        if inside {
            return;
        }
        if self.menu.take().is_some() && areas.panel.is_some_and(|b| b.contains(&position)) {
            cx.notify();
            return;
        }
        cx.emit(SharePopoverEvent::Dismiss);
    }

    fn run(&mut self, pending: Pending, command: ShareCommand, cx: &mut Context<Self>) {
        let account = account::current(cx);
        let Some(cloud) = cx.try_global::<SharingCloud>().map(|cloud| cloud.0.clone()) else {
            self.error = Some(failure_message(&command).into());
            cx.notify();
            return;
        };
        let scope = Scope {
            account_id: account.user.map(|user| user.id),
            session_id: Some(self.session.clone()),
            ..Scope::default()
        };
        self.pending = Some(pending);
        self.error = None;
        self.menu = None;
        let cancel = self.cancel.clone();
        cx.spawn(async move |this, cx| {
            let result = cloud.share_command(scope, command.clone()).await;
            if cancel.is_cancelled() {
                return;
            }
            let _ = this.update(cx, |this, cx| {
                this.pending = None;
                this.optimistic = None;
                match result {
                    Ok(share) => {
                        if let ShareCommand::Invite(emails) = &command {
                            for email in emails {
                                this.invite.remove(email);
                            }
                            this.query.update(cx, |q, cx| q.set_text(String::new(), cx));
                        }
                        if let ShareCommand::Remove(entry) = &command
                            && let Some(email) = &entry.email
                        {
                            this.invite.restore(email);
                        }
                        remember(this.session.clone(), share, cx);
                        this.exclude_invited(cx);
                    }
                    Err(_) => this.error = Some(failure_message(&command).into()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn sign_in(&mut self, cx: &mut Context<Self>) {
        let Some(cloud) = cx.try_global::<SharingCloud>().map(|cloud| cloud.0.clone()) else {
            cx.emit(SharePopoverEvent::Open(ProductRoute::Account));
            return;
        };
        self.pending = Some(Pending::SignIn);
        cx.spawn(async move |this, cx| {
            let result = cloud.sign_in().await;
            let _ = this.update(cx, |this, cx| {
                this.pending = None;
                if result.is_err() {
                    this.error = Some("Could not start sign-in.".into());
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn submit_invite(&mut self, cx: &mut Context<Self>) {
        let typed = self.query.read(cx).buffer.text.clone();
        let emails = self.invite.emails(&typed);
        if !self.invite.can_submit(&typed) {
            return;
        }
        self.invite.commit(&typed);
        self.run(Pending::Invite, ShareCommand::Invite(emails), cx);
    }
}

fn failure_message(command: &ShareCommand) -> &'static str {
    match command {
        ShareCommand::Invite(emails) if emails.len() > 1 => "Could not create these invitations.",
        ShareCommand::Invite(_) => "Could not create this invitation.",
        ShareCommand::CopyLink => "Could not copy the share link.",
        ShareCommand::Access(_) => "Could not update general access.",
        ShareCommand::OpenWebCopy => "Could not open the web copy.",
        ShareCommand::KeepDesktop => {
            "Could not publish the desktop edits. Check the latest web copy and try again."
        }
        _ => "Could not update this person's access.",
    }
}

impl Render for SharePopover {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let account = account::current(cx);
        let share = managed(&self.session, cx).filter(|_| account.signed_in());
        let paid = account.is_paid(account::now());
        let entity = cx.entity();
        let content = if account.signed_in() && !paid && share.is_none() && !self.loading {
            self.upgrade(colors, cx).into_any_element()
        } else {
            self.panel(share, !account.signed_in(), paid, &account, window, cx)
                .into_any_element()
        };
        div()
            .id("share-popover")
            .track_focus(&self.focus)
            .occlude()
            .text_color(colors.popover_foreground)
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    if this.menu.take().is_none() {
                        cx.emit(SharePopoverEvent::Dismiss);
                    }
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(track(entity.clone(), |areas, bounds| {
                areas.panel = Some(bounds)
            }))
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        let entity = entity.clone();
                        window.on_mouse_event(move |event: &gpui::MouseDownEvent, phase, _, cx| {
                            if phase == gpui::DispatchPhase::Capture {
                                entity.update(cx, |this, cx| {
                                    this.dismiss_if_outside(event.position, cx)
                                });
                            }
                        });
                    },
                )
                .absolute()
                .size_0(),
            )
            .child(content)
    }
}

/// Records an element's bounds while it is laid out (place it as an absolute child).
fn track(entity: Entity<SharePopover>, slot: fn(&mut Areas, Bounds<Pixels>)) -> impl IntoElement {
    canvas(
        move |bounds, _, cx| entity.update(cx, |this, _| slot(&mut this.areas, bounds)),
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

/// Hangs `popover` under its trigger, right-aligned 8px below it (`align="end" sideOffset={8}`).
/// Render it as the last child of the trigger, which must be `relative`.
pub fn anchor(popover: Entity<SharePopover>) -> impl IntoElement {
    let trigger = popover.clone();
    div()
        .absolute()
        .inset_0()
        .child(
            canvas(
                move |bounds, _, cx| trigger.update(cx, |this, _| this.set_trigger(bounds)),
                |_, _, _, _| {},
            )
            .size_full(),
        )
        .child(
            div().absolute().top_full().right_0().child(
                deferred(
                    anchored()
                        .anchor(gpui::Anchor::TopRight)
                        .snap_to_window_with_margin(px(8.))
                        .child(div().pt(px(8.)).child(popover)),
                )
                .with_priority(2),
            ),
        )
}

pub(super) fn spinner(id: impl Into<gpui::ElementId>, size: f32, color: Hsla) -> AnyElement {
    icon("sharing/LoaderCircleIcon.svg", size, color)
        .with_animation(
            id,
            Animation::new(Duration::from_secs(1)).repeat(),
            |icon, delta| icon.with_transformation(Transformation::rotate(percentage(delta))),
        )
        .into_any_element()
}

pub(super) fn icon(path: &'static str, size: f32, color: Hsla) -> gpui::Svg {
    svg()
        .path(path)
        .size(px(size))
        .flex_shrink_0()
        .text_color(color)
}

/// `PopoverContent variant="app"` wrapping an `AppFloatingPanel`.
pub(super) fn floating(width: f32, colors: Theme, window: &Window) -> (Div, Div) {
    let max = (f32::from(window.viewport_size().height) - 74.).min(530.);
    (
        div()
            .w(px(width))
            .p(px(2.))
            .rounded(px(22.))
            .border_1()
            .border_color(colors.app_floating_border)
            .bg(colors.app_floating_chrome)
            .shadow_lg()
            .overflow_hidden(),
        div()
            .relative()
            .max_h(px(max))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(20.))
            .border_1()
            .border_color(colors.app_floating_border)
            .bg(colors.app_floating_panel),
    )
}

/// The `Button` variants at `size="sm"`: 28px tall, 12px medium text, 8px gaps.
pub(super) fn button(
    id: impl Into<gpui::ElementId>,
    primary: bool,
    enabled: bool,
    colors: Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(28.))
        .px_2()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .gap_2()
        .rounded(px(8.))
        .text_xs()
        .line_height(px(16.))
        .font_weight(crate::ui::theme::MEDIUM)
        .map(|button| {
            if primary {
                button
                    .bg(colors.primary)
                    .text_color(colors.primary_foreground)
                    .shadow_xs()
                    .when(enabled, |b| b.hover(|s| s.bg(colors.primary.opacity(0.9))))
            } else {
                button
                    .border_1()
                    .border_color(colors.input)
                    .bg(colors.background)
                    .shadow_xs()
                    .when(enabled, |b| b.hover(|s| s.bg(colors.accent)))
            }
        })
        .when_else(enabled, |b| b.cursor_pointer(), |b| b.opacity(0.5))
}

fn small_label(text: impl Into<SharedString>, colors: Theme) -> Div {
    div()
        .px(px(6.))
        .text_size(px(10.))
        .line_height(px(15.))
        .font_weight(crate::ui::theme::MEDIUM)
        .text_color(colors.muted_foreground)
        .child(text.into())
}

/// A person row: avatar, name over a muted detail, then trailing content.
fn person_row(
    id: impl Into<gpui::ElementId>,
    label: &str,
    name: AnyElement,
    detail: Option<SharedString>,
    hover: bool,
    colors: Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .min_h(px(36.))
        .px(px(6.))
        .py_1()
        .flex()
        .items_center()
        .gap_2()
        .rounded(px(8.))
        .when(hover, |row| row.hover(|s| s.bg(colors.accent.opacity(0.5))))
        .child(facehash(label, 24.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(name)
                .when_some(detail, |view, detail| {
                    view.child(
                        div()
                            .truncate()
                            .text_size(px(10.))
                            .line_height(px(15.))
                            .text_color(colors.muted_foreground)
                            .child(detail),
                    )
                }),
        )
}

fn name_line(text: impl Into<SharedString>) -> Div {
    div()
        .truncate()
        .text_xs()
        .line_height(px(16.))
        .font_weight(crate::ui::theme::MEDIUM)
        .child(text.into())
}

fn status(text: impl Into<SharedString>, colors: Theme) -> Div {
    div()
        .flex_shrink_0()
        .text_size(px(11.))
        .line_height(px(16.5))
        .text_color(colors.muted_foreground)
        .child(text.into())
}

/// A select's floating list (`SelectContent`): 18px corners, 4px padding.
pub(super) fn select_list(width: f32, colors: Theme) -> Div {
    div()
        .min_w(px(width.max(128.)))
        .p_1()
        .flex()
        .flex_col()
        .rounded(px(18.))
        .border_1()
        .border_color(colors.border)
        .bg(colors.popover)
        .text_color(colors.popover_foreground)
        .shadow_md()
}

/// A `SelectItem`: 14px corners, the check at the right for the selected value.
pub(super) fn select_item(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    enabled: bool,
    color: Option<Hsla>,
    colors: Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .relative()
        .w_full()
        .py(px(6.))
        .pl_2()
        .pr_8()
        .flex()
        .items_center()
        .rounded(px(14.))
        .text_sm()
        .line_height(px(20.))
        .when_some(color, |item, color| item.text_color(color))
        .when_else(
            enabled,
            |item| item.cursor_pointer().hover(|s| s.bg(colors.accent)),
            |item| item.opacity(0.5),
        )
        .child(label.into())
        .when(selected, |item| {
            item.child(
                div()
                    .absolute()
                    .right_2()
                    .top_0()
                    .bottom_0()
                    .flex()
                    .items_center()
                    .child(icon(
                        "sharing/CheckIcon.svg",
                        16.,
                        colors.popover_foreground,
                    )),
            )
        })
}

fn separator(colors: Theme) -> Div {
    div().mx(px(-4.)).my_1().h(px(1.)).bg(colors.muted)
}

impl SharePopover {
    fn menu_layer(&self, menu: Div, align_end: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        div()
            .absolute()
            .top_full()
            .when(align_end, |view| view.right_0())
            .when(!align_end, |view| view.left_0())
            .child(
                deferred(
                    anchored()
                        .anchor(if align_end {
                            gpui::Anchor::TopRight
                        } else {
                            gpui::Anchor::TopLeft
                        })
                        .snap_to_window_with_margin(px(8.))
                        .child(
                            div()
                                .id("share-menu")
                                .relative()
                                .mt_1()
                                .occlude()
                                .child(menu)
                                .child(track(entity, |areas, bounds| areas.menu = Some(bounds))),
                        ),
                )
                .with_priority(3),
            )
    }

    fn toggle_menu(&mut self, menu: Menu, cx: &mut Context<Self>) {
        self.menu = if self.menu.as_ref() == Some(&menu) {
            None
        } else {
            Some(menu)
        };
        if self.menu.is_none() {
            self.areas.menu = None;
        }
        cx.notify();
    }

    fn upgrade(&mut self, colors: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let (chrome, panel) = (
            div()
                .w(px(440.))
                .p(px(2.))
                .rounded(px(22.))
                .border_1()
                .border_color(colors.app_floating_border)
                .bg(colors.app_floating_chrome)
                .shadow_lg()
                .overflow_hidden(),
            div()
                .rounded(px(20.))
                .border_1()
                .border_color(colors.app_floating_border)
                .bg(colors.app_floating_panel),
        );
        chrome.child(
            panel
                .px_6()
                .py(px(28.))
                .flex()
                .flex_col()
                .items_center()
                .text_center()
                .child(
                    div()
                        .size_10()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(8.))
                        .bg(colors.accent)
                        .child(icon(
                            "sharing/UsersIcon.svg",
                            16.,
                            colors.popover_foreground,
                        )),
                )
                .child(
                    div()
                        .mt_3()
                        .text_sm()
                        .line_height(px(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Share notes with others"),
                )
                .child(
                    div()
                        .mt_1()
                        .text_xs()
                        .line_height(px(20.))
                        .text_color(colors.muted_foreground)
                        .child("Upgrade to Pro to invite people and share this note with them."),
                )
                .child(
                    button("share-upgrade", true, true, colors)
                        .mt_4()
                        .on_click(cx.listener(|_, _, _, cx| {
                            cx.emit(SharePopoverEvent::Open(ProductRoute::Billing))
                        }))
                        .child("Upgrade to Pro"),
                ),
        )
    }

    fn panel(
        &mut self,
        share: Option<ManagedShare>,
        gated: bool,
        paid: bool,
        account: &account::Account,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let colors = theme(window);
        let (chrome, panel) = floating(380., colors, window);
        // Draft: everything waits for the share read and needs Pro. Managed: invites and wider
        // access also wait while the web and desktop copies conflict.
        let can_publish = match &share {
            Some(share) => paid && !share.conflict,
            None => !gated && !self.loading && paid,
        };
        let busy = self.pending.is_some();
        let owner_email = account
            .user
            .as_ref()
            .and_then(|user| user.email.clone())
            .map(SharedString::from);
        let owner_name = owner_email.clone().unwrap_or_else(|| "You".into());
        let body = div()
            .id("share-body")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .when(self.load_failed && share.is_none() && !gated, |view| {
                view.child(self.load_error(colors, cx))
            })
            .when(share.as_ref().is_some_and(|share| share.conflict), |view| {
                view.child(self.conflict_banner(paid, colors, window, cx))
            })
            .map(|view| match self.mode {
                Mode::Invite => view.child(self.invite_section(
                    share.as_ref(),
                    !can_publish || busy,
                    owner_name.clone(),
                    owner_email.clone(),
                    paid,
                    colors,
                    cx,
                )),
                Mode::Email => view.child(self.email_section(!can_publish || busy, colors, cx)),
                Mode::Slack => view.child(self.slack_section(!can_publish || busy, colors, cx)),
            })
            .when_some(self.error.clone(), |view, error| {
                view.child(
                    div()
                        .px(px(6.))
                        .text_size(px(11.))
                        .line_height(px(16.))
                        .text_color(colors.destructive)
                        .child(error),
                )
            });
        let footer = self.footer(share.as_ref(), gated, can_publish, paid, colors, cx);
        let content = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .child(body)
            .child(footer);
        chrome.child(
            panel
                .child(if gated {
                    // Shipping blurs the inert draft under the gate; native views cannot blur,
                    // so it is faded instead.
                    content.opacity(0.4).into_any_element()
                } else {
                    content.into_any_element()
                })
                .when(gated, |panel| panel.child(self.gate(colors, cx))),
        )
    }

    fn gate(&self, colors: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let pending = self.pending == Some(Pending::SignIn);
        div()
            .id("share-gate")
            .absolute()
            .inset_0()
            .px_6()
            .flex()
            .items_center()
            .justify_center()
            .bg(colors.background.opacity(0.7))
            .occlude()
            .child(
                div()
                    .max_w(px(220.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .text_center()
                    .child(
                        div()
                            .text_sm()
                            .line_height(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Sign in to share"),
                    )
                    .child(
                        div()
                            .mt_1()
                            .text_xs()
                            .line_height(px(20.))
                            .text_color(colors.muted_foreground)
                            .child("Sign in to share this note with others."),
                    )
                    .child(
                        button("share-sign-in", true, !pending, colors)
                            .mt_4()
                            .when(!pending, |b| {
                                b.on_click(cx.listener(|this, _, _, cx| this.sign_in(cx)))
                            })
                            .child(if pending { "Opening…" } else { "Sign in" }),
                    ),
            )
    }

    fn load_error(&self, colors: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(10.))
            .py_2()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .rounded(px(8.))
            .border_1()
            .border_color(colors.destructive.opacity(0.3))
            .bg(colors.destructive.opacity(0.05))
            .child(
                div()
                    .text_xs()
                    .line_height(px(16.))
                    .text_color(colors.muted_foreground)
                    .child("Access settings could not be loaded."),
            )
            .child(
                button("share-retry", false, true, colors)
                    .on_click(cx.listener(|this, _, _, cx| this.load(cx)))
                    .child(icon("sharing/RefreshIcon.svg", 16., colors.foreground))
                    .child("Try again"),
            )
    }

    #[allow(clippy::too_many_arguments)]
    fn invite_section(
        &mut self,
        share: Option<&ManagedShare>,
        disabled: bool,
        owner_name: SharedString,
        owner_email: Option<SharedString>,
        paid: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let recipients = self.invite.recipients();
        let typed = self.query.read(cx).buffer.text.clone();
        let suggestions = self.suggestions(&self.invite, &typed);
        let pending = self.pending == Some(Pending::Invite);
        div()
            .flex()
            .flex_col()
            .child(self.invite_form(
                self.query.clone(),
                &typed,
                &self.invite,
                disabled,
                pending,
                "Invite",
                colors,
                cx.listener(|this, _, _, cx| this.submit_invite(cx)),
            ))
            .when(!suggestions.is_empty(), |view| {
                view.child(self.suggestion_list(suggestions, false, colors, cx))
            })
            .when(!recipients.is_empty(), |view| {
                view.child(
                    div()
                        .mt_2()
                        .flex()
                        .flex_col()
                        .child(small_label("Suggested attendees", colors))
                        .child(self.recipient_rows(
                            &recipients,
                            false,
                            disabled,
                            Some("Not invited"),
                            colors,
                            cx,
                        )),
                )
            })
            .child(
                div()
                    .mt_2()
                    .pt_2()
                    .flex()
                    .flex_col()
                    .child(small_label("People with access", colors).mb_1())
                    .child(
                        person_row(
                            "share-owner",
                            &owner_name,
                            div()
                                .truncate()
                                .text_xs()
                                .line_height(px(16.))
                                .font_weight(crate::ui::theme::MEDIUM)
                                .child(format!("{owner_name} "))
                                .child(div().text_color(colors.muted_foreground).child("(You)"))
                                .flex()
                                .into_any_element(),
                            owner_email,
                            false,
                            colors,
                        )
                        .child(status("Full access", colors)),
                    )
                    .children(share.into_iter().flat_map(|share| {
                        share
                            .access
                            .iter()
                            .map(|entry| self.access_row(entry, paid, colors, cx))
                            .collect::<Vec<_>>()
                    })),
            )
    }

    #[allow(clippy::too_many_arguments)]
    fn invite_form(
        &self,
        input: Entity<TextInput>,
        typed: &str,
        invite: &Invite,
        disabled: bool,
        pending: bool,
        label: &'static str,
        colors: Theme,
        on_submit: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let emails = invite.emails(typed);
        let enabled = !disabled && invite.can_submit(typed);
        div()
            .flex()
            .items_start()
            .gap_2()
            .child(
                div()
                    .h(px(32.))
                    .flex_1()
                    .min_w_0()
                    .px_3()
                    .flex()
                    .items_center()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(colors.input)
                    // GPUI paints shadows under the box, so it needs the panel's own fill.
                    .bg(colors.app_floating_panel)
                    .shadow_xs()
                    .text_xs()
                    .line_height(px(16.))
                    .when(disabled, |field| field.opacity(0.5))
                    .child(div().w_full().overflow_hidden().child(input)),
            )
            .child(
                button(
                    SharedString::from(format!("share-submit-{label}")),
                    true,
                    enabled,
                    colors,
                )
                .h(px(32.))
                .px_3()
                .when(enabled, |b| b.on_click(on_submit))
                .when(pending, |b| {
                    b.child(spinner(
                        "share-submit-spinner",
                        16.,
                        colors.primary_foreground,
                    ))
                })
                .child(label)
                .when(!emails.is_empty(), |b| {
                    b.child(format!("({})", emails.len()))
                }),
            )
    }

    /// Contacts whose name or email contains the typed text, while it is not an address.
    fn suggestions(&self, invite: &Invite, typed: &str) -> Vec<Recipient> {
        let normalized = typed.trim().to_lowercase();
        if normalized.is_empty() || is_invite_email(&normalized) {
            return Vec::new();
        }
        self.humans
            .iter()
            .filter(|human| {
                invite.is_selectable(&human.email)
                    && format!("{}\n{}", human.name, human.email)
                        .to_lowercase()
                        .contains(&normalized)
            })
            .take(4)
            .cloned()
            .collect()
    }

    fn suggestion_list(
        &self,
        suggestions: Vec<Recipient>,
        email: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .mt_1()
            .p_1()
            .flex()
            .flex_col()
            .gap(px(2.))
            .rounded(px(8.))
            .border_1()
            .border_color(colors.border)
            .children(suggestions.into_iter().map(|contact| {
                let label = if contact.name.is_empty() {
                    contact.email.clone()
                } else {
                    contact.name.clone()
                };
                let has_name = !contact.name.is_empty();
                let email_text = contact.email.clone();
                div()
                    .id(SharedString::from(format!(
                        "share-suggest-{}",
                        contact.email
                    )))
                    .w_full()
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(|s| s.bg(colors.accent))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let (invite, input) = if email {
                            (&mut this.email, this.email_query.clone())
                        } else {
                            (&mut this.invite, this.query.clone())
                        };
                        invite.add(contact.clone());
                        input.update(cx, |q, cx| q.set_text(String::new(), cx));
                        cx.notify();
                    }))
                    .child(facehash(&label, 22.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(name_line(label.to_string()))
                            .when(has_name, |view| {
                                view.child(
                                    div()
                                        .truncate()
                                        .text_size(px(10.))
                                        .line_height(px(15.))
                                        .text_color(colors.muted_foreground)
                                        .child(email_text.to_string()),
                                )
                            }),
                    )
            }))
    }

    fn recipient_rows(
        &self,
        recipients: &[Recipient],
        email: bool,
        disabled: bool,
        status_text: Option<&'static str>,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .mt_1()
            .flex()
            .flex_col()
            .children(recipients.iter().map(|recipient| {
                let label = if recipient.name.is_empty() {
                    recipient.email.to_string()
                } else {
                    recipient.name.to_string()
                };
                let key = recipient.email.to_lowercase();
                let remove = recipient.email.clone();
                person_row(
                    SharedString::from(format!("share-recipient-{key}")),
                    &label,
                    name_line(label.clone()).into_any_element(),
                    (!recipient.name.is_empty()).then(|| recipient.email.to_string().into()),
                    true,
                    colors,
                )
                .when_some(status_text, |row, text| row.child(status(text, colors)))
                .child(
                    div()
                        .id(SharedString::from(format!("share-remove-{key}")))
                        .group(SharedString::from(format!("share-remove-{key}")))
                        .size(px(28.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.))
                        .when(!disabled, |b| {
                            b.cursor_pointer()
                                .hover(|s| s.bg(colors.accent))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if email {
                                        this.email.remove(&remove);
                                    } else {
                                        this.invite.remove(&remove);
                                    }
                                    cx.notify();
                                }))
                        })
                        .child(
                            icon("sharing/XIcon.svg", 16., colors.muted_foreground).group_hover(
                                SharedString::from(format!("share-remove-{key}")),
                                |s| s.text_color(colors.foreground),
                            ),
                        ),
                )
            }))
    }

    fn access_row(
        &self,
        entry: &AccessEntry,
        paid: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let contact = entry.email.as_ref().and_then(|email| {
            self.humans
                .iter()
                .find(|human| human.email.eq_ignore_ascii_case(email))
                .map(|human| human.name.to_string())
        });
        let label = entry.label(contact.as_deref());
        let detail = entry.detail(contact.as_deref());
        let pending = self.pending == Some(Pending::Entry(entry.id.clone()));
        let id = entry.id.clone();
        let row = person_row(
            SharedString::from(format!("share-entry-{id}")),
            &label,
            name_line(label.clone()).into_any_element(),
            Some(detail.into()),
            true,
            colors,
        );
        if pending {
            return row
                .child(spinner(
                    SharedString::from(format!("share-entry-spin-{id}")),
                    16.,
                    colors.muted_foreground,
                ))
                .into_any_element();
        }
        if entry.kind == EntryKind::Request {
            let deny = entry.clone();
            let approve = entry.clone();
            return row
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .id(SharedString::from(format!("share-deny-{id}")))
                                .h(px(28.))
                                .px_2()
                                .flex()
                                .items_center()
                                .rounded(px(8.))
                                .text_xs()
                                .font_weight(crate::ui::theme::MEDIUM)
                                .cursor_pointer()
                                .hover(|s| s.bg(colors.accent))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.run(
                                        Pending::Entry(deny.id.clone()),
                                        ShareCommand::Review(deny.clone(), false),
                                        cx,
                                    )
                                }))
                                .child("Deny"),
                        )
                        .child(
                            button(
                                SharedString::from(format!("share-approve-{id}")),
                                true,
                                paid,
                                colors,
                            )
                            .when(paid, |b| {
                                b.on_click(cx.listener(move |this, _, _, cx| {
                                    this.run(
                                        Pending::Entry(approve.id.clone()),
                                        ShareCommand::Review(approve.clone(), true),
                                        cx,
                                    )
                                }))
                            })
                            .child("Approve"),
                        ),
                )
                .into_any_element();
        }
        let enabled = paid || entry.kind == EntryKind::Grant;
        let open = self.menu == Some(Menu::Entry(id.clone()));
        let trigger_id = SharedString::from(format!("share-capability-{id}"));
        row.child(
            div()
                .relative()
                .flex_shrink_0()
                .child(
                    div()
                        .id(trigger_id)
                        .h(px(28.))
                        .min_w(px(84.))
                        .px(px(6.))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_1()
                        .rounded(px(6.))
                        .text_size(px(11.))
                        .line_height(px(16.5))
                        .text_color(colors.muted_foreground)
                        .when_else(
                            enabled,
                            |b| {
                                let menu = Menu::Entry(id.clone());
                                b.cursor_pointer()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.toggle_menu(menu.clone(), cx)
                                    }))
                            },
                            |b| b.opacity(0.5),
                        )
                        .child(entry.capability.label())
                        .child(icon(
                            "sharing/ArrowDown01Icon.svg",
                            16.,
                            colors.muted_foreground.opacity(0.5),
                        )),
                )
                .when(open, |view| {
                    view.child(self.menu_layer(
                        self.capability_menu(entry, paid, colors, cx),
                        true,
                        cx,
                    ))
                }),
        )
        .into_any_element()
    }

    fn capability_menu(
        &self,
        entry: &AccessEntry,
        paid: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let maximum = if paid {
            Capability::Editor.rank()
        } else {
            entry.capability.rank()
        };
        let invitation = entry.kind == EntryKind::Invitation;
        let resend = entry.clone();
        let remove = entry.clone();
        select_list(84., colors)
            .children(Capability::ALL.into_iter().map(|capability| {
                let enabled = capability.rank() <= maximum;
                let target = entry.clone();
                select_item(
                    SharedString::from(format!("share-cap-{}-{}", entry.id, capability.as_str())),
                    capability.label(),
                    capability == entry.capability,
                    enabled,
                    None,
                    colors,
                )
                .when(enabled, |item| {
                    item.on_click(cx.listener(move |this, _, _, cx| {
                        if capability == target.capability {
                            this.menu = None;
                            cx.notify();
                            return;
                        }
                        this.run(
                            Pending::Entry(target.id.clone()),
                            ShareCommand::SetCapability(target.clone(), capability),
                            cx,
                        )
                    }))
                })
            }))
            .child(separator(colors))
            .when(invitation, |list| {
                list.child(
                    select_item("share-resend", "Resend invite", false, paid, None, colors).when(
                        paid,
                        |item| {
                            item.on_click(cx.listener(move |this, _, _, cx| {
                                this.run(
                                    Pending::Entry(resend.id.clone()),
                                    ShareCommand::Resend(resend.clone()),
                                    cx,
                                )
                            }))
                        },
                    ),
                )
            })
            .child(
                select_item(
                    "share-remove",
                    "Remove",
                    false,
                    true,
                    Some(colors.destructive),
                    colors,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.run(
                        Pending::Entry(remove.id.clone()),
                        ShareCommand::Remove(remove.clone()),
                        cx,
                    )
                })),
            )
    }

    fn recap_heading(
        &self,
        title: &'static str,
        detail: &'static str,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .id("share-recap-back")
                            .group("share-recap-back")
                            .size_6()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(6.))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.mode = Mode::Invite;
                                cx.notify();
                            }))
                            .child(
                                icon("sharing/ArrowLeft01Icon.svg", 16., colors.muted_foreground)
                                    .group_hover("share-recap-back", |s| {
                                        s.text_color(colors.foreground)
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .line_height(px(16.))
                            .font_weight(crate::ui::theme::MEDIUM)
                            .child(title),
                    ),
            )
            .child(
                div()
                    .mt(px(2.))
                    .text_size(px(11.))
                    .line_height(px(16.))
                    .text_color(colors.muted_foreground)
                    .child(detail),
            )
    }

    fn email_section(
        &mut self,
        disabled: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let recipients = self.email.recipients();
        let typed = self.email_query.read(cx).buffer.text.clone();
        let suggestions = self.suggestions(&self.email, &typed);
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.recap_heading(
                "Email meeting notes",
                "Send the summary in the email. Replies go directly to you.",
                colors,
                cx,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(self.invite_form(
                        self.email_query.clone(),
                        &typed,
                        &self.email,
                        disabled,
                        false,
                        "Send",
                        colors,
                        cx.listener(|this, _, _, cx| {
                            this.error = Some("Could not email the meeting notes.".into());
                            cx.notify();
                        }),
                    ))
                    .when(!suggestions.is_empty(), |view| {
                        view.child(self.suggestion_list(suggestions, true, colors, cx))
                    }),
            )
            .when(!recipients.is_empty(), |view| {
                view.child(self.recipient_rows(&recipients, true, disabled, None, colors, cx))
            })
    }

    fn slack_section(
        &mut self,
        disabled: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.recap_heading(
                "Send to Slack",
                "Connect Slack to choose a channel for this recap.",
                colors,
                cx,
            ))
            .child(
                div().flex().child(
                    button("share-connect-slack", false, !disabled, colors)
                        .h(px(32.))
                        .when(!disabled, |b| {
                            b.on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(SharePopoverEvent::Open(ProductRoute::Integrations))
                            }))
                        })
                        .child(img("sharing/SlackLogo.svg").size(px(16.)))
                        .child("Connect Slack"),
                ),
            )
    }

    fn footer(
        &mut self,
        share: Option<&ManagedShare>,
        gated: bool,
        can_publish: bool,
        paid: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let workspaces = self.workspaces.clone();
        let value = match (&self.optimistic, share) {
            (Some(value), _) => value.clone(),
            (None, Some(share)) => share.general.clone(),
            (None, None) => default_general_access(&self.default_access, &workspaces),
        };
        // The draft's select waits for the share read and Pro; a managed share's needs only
        // its management data.
        let disabled = match share {
            Some(_) => false,
            None => gated || self.loading || !paid,
        };
        let access_pending = self.pending == Some(Pending::Access);
        let copy_pending =
            self.pending == Some(Pending::CopyLink) || (share.is_some() && access_pending);
        let copy_enabled =
            !disabled && !copy_pending && !(share.is_none() && self.pending.is_some());
        let select_enabled = !disabled && !access_pending;
        let open_access = self.menu == Some(Menu::Access);
        let open_more = self.menu == Some(Menu::More);
        div()
            .p_3()
            .flex()
            .items_center()
            .gap_1()
            .border_t_1()
            .border_color(colors.border.opacity(0.6))
            .child(
                div().flex_1().min_w_0().flex().items_center().child(
                    div()
                        .relative()
                        .min_w_0()
                        .child(
                            div()
                                .id("share-general-access")
                                .h(px(28.))
                                .max_w_full()
                                .px(px(6.))
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .rounded(px(6.))
                                .text_xs()
                                .line_height(px(16.))
                                .when_else(
                                    select_enabled,
                                    |b| {
                                        b.cursor_pointer().on_click(cx.listener(
                                            |this, _, _, cx| this.toggle_menu(Menu::Access, cx),
                                        ))
                                    },
                                    |b| b.opacity(0.5),
                                )
                                .child(if access_pending {
                                    spinner("share-access-spin", 16., colors.foreground)
                                } else {
                                    icon(value.icon(), 16., colors.foreground).into_any_element()
                                })
                                .child(div().min_w_0().truncate().child(value.label(&workspaces)))
                                .child(
                                    icon(
                                        "sharing/ArrowDown01Icon.svg",
                                        16.,
                                        colors.foreground.opacity(0.5),
                                    )
                                    .mr(px(-4.)),
                                ),
                        )
                        .when(open_access, |view| {
                            view.child(self.menu_layer(
                                self.access_menu(&value, share.is_some(), can_publish, colors, cx),
                                false,
                                cx,
                            ))
                        }),
                ),
            )
            .child(
                div()
                    .relative()
                    .flex_shrink_0()
                    .child(
                        // Shipping draws this border as a squircle overlay, so it takes no space.
                        div()
                            .relative()
                            .h(px(28.))
                            .flex()
                            .items_center()
                            .overflow_hidden()
                            .rounded(px(8.))
                            .bg(colors.background)
                            .shadow_xs()
                            .child(
                                div()
                                    .id("share-copy-link")
                                    .h_full()
                                    .px(px(10.))
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .text_xs()
                                    .line_height(px(16.))
                                    .font_weight(crate::ui::theme::MEDIUM)
                                    .when_else(
                                        copy_enabled,
                                        |b| {
                                            b.cursor_pointer()
                                                .hover(|s| s.bg(colors.accent))
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.run(
                                                        Pending::CopyLink,
                                                        ShareCommand::CopyLink,
                                                        cx,
                                                    )
                                                }))
                                        },
                                        |b| b.opacity(0.5),
                                    )
                                    .child(if copy_pending {
                                        spinner("share-copy-spin", 16., colors.foreground)
                                    } else {
                                        icon("sharing/CopyIcon.svg", 16., colors.foreground)
                                            .into_any_element()
                                    })
                                    .child("Copy link"),
                            )
                            .child(
                                div()
                                    .id("share-more-options")
                                    .group("share-more-options")
                                    .h_full()
                                    .w(px(24.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(colors.accent))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.toggle_menu(Menu::More, cx)
                                    }))
                                    .child(
                                        icon(
                                            "sharing/ArrowDown01Icon.svg",
                                            12.,
                                            colors.muted_foreground,
                                        )
                                        .group_hover("share-more-options", |s| {
                                            s.text_color(colors.foreground)
                                        }),
                                    ),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .rounded(px(8.))
                                    .border_1()
                                    .border_color(colors.input),
                            ),
                    )
                    .when(open_more, |view| {
                        view.child(self.menu_layer(self.more_menu(colors, cx), true, cx))
                    }),
            )
    }

    fn access_menu(
        &self,
        value: &GeneralAccess,
        managed: bool,
        can_expand: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let pick = |target: GeneralAccess| {
            cx.listener(move |this: &mut Self, _: &ClickEvent, _, cx| {
                // The draft keeps "Only people invited" as its starting point until an action.
                if !managed && target == GeneralAccess::Restricted {
                    this.menu = None;
                    cx.notify();
                    return;
                }
                this.optimistic = Some(target.clone());
                this.run(Pending::Access, ShareCommand::Access(target.clone()), cx);
            })
        };
        let mut options = vec![(
            GeneralAccess::Restricted,
            "Only people invited".to_string(),
            true,
        )];
        options.extend(self.workspaces.iter().map(|workspace| {
            (
                GeneralAccess::Workspace(workspace.id.clone()),
                format!("Everyone in {}", workspace.name),
                can_expand,
            )
        }));
        options.push((
            GeneralAccess::Link,
            "Anyone with the link".into(),
            can_expand,
        ));
        select_list(160., colors)
            .children(
                options
                    .into_iter()
                    .enumerate()
                    .map(|(index, (target, label, enabled))| {
                        select_item(
                            ("share-access-option", index),
                            label,
                            &target == value,
                            enabled,
                            None,
                            colors,
                        )
                        .when(enabled, |item| item.on_click(pick(target)))
                    }),
            )
            .when(*value == GeneralAccess::Public, |list| {
                list.child(separator(colors)).child(select_item(
                    "share-access-public",
                    "Public on the web",
                    true,
                    true,
                    None,
                    colors,
                ))
            })
    }

    fn more_menu(&self, colors: Theme, cx: &mut Context<Self>) -> Div {
        let item = |id: &'static str, label: &'static str, glyph: AnyElement| {
            div()
                .id(id)
                .px_2()
                .py(px(6.))
                .flex()
                .items_center()
                .gap_2()
                .rounded(px(14.))
                .text_sm()
                .line_height(px(20.))
                .cursor_pointer()
                .hover(|s| s.bg(colors.accent))
                .child(glyph)
                .child(label)
        };
        div()
            .min_w(px(128.))
            .p(px(2.))
            .rounded(px(22.))
            .border_1()
            .border_color(colors.app_floating_border)
            .bg(colors.app_floating_chrome)
            .shadow_lg()
            .child(
                div()
                    .p(px(6.))
                    .flex()
                    .flex_col()
                    .rounded(px(20.))
                    .border_1()
                    .border_color(colors.app_floating_border)
                    .bg(colors.app_floating_panel)
                    .text_color(colors.popover_foreground)
                    .child(
                        item(
                            "share-recap-email",
                            "Email",
                            icon("sharing/Mail02Icon.svg", 16., colors.popover_foreground)
                                .into_any_element(),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.mode = Mode::Email;
                            this.menu = None;
                            cx.notify();
                        })),
                    )
                    .child(
                        item(
                            "share-recap-slack",
                            "Slack",
                            img("sharing/SlackLogo.svg")
                                .size(px(16.))
                                .into_any_element(),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.mode = Mode::Slack;
                            this.menu = None;
                            cx.notify();
                        })),
                    ),
            )
    }
}

impl SharePopover {
    /// Shown while web and desktop edits conflict; inviting waits until one copy wins.
    fn conflict_banner(
        &self,
        paid: bool,
        colors: Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let amber: Hsla = if crate::ui::theme::is_dark(window) {
            gpui::rgb(0xfbbf24).into()
        } else {
            gpui::rgb(0xd97706).into()
        };
        let base: Hsla = gpui::rgb(0xf59e0b).into();
        let opening = self.pending == Some(Pending::OpenWeb);
        let keeping = self.pending == Some(Pending::KeepDesktop);
        div()
            .px(px(10.))
            .py_2()
            .rounded(px(8.))
            .border_1()
            .border_color(base.opacity(0.35))
            .bg(base.opacity(0.1))
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(10.))
                    .child(icon("sharing/TriangleAlertIcon.svg", 16., amber).mt(px(2.)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_xs()
                                    .line_height(px(16.))
                                    .font_weight(crate::ui::theme::MEDIUM)
                                    .child("Sharing paused to protect your edits"),
                            )
                            .child(
                                div()
                                    .mt(px(2.))
                                    .text_size(px(11.))
                                    .line_height(px(16.))
                                    .text_color(colors.muted_foreground)
                                    .child(
                                        "Resolve the web and desktop edits before inviting anyone.",
                                    ),
                            )
                            .child(
                                div()
                                    .mt(px(10.))
                                    .flex()
                                    .flex_wrap()
                                    .gap_2()
                                    .child(
                                        button("share-open-web", false, !opening, colors)
                                            .when(!opening, |b| {
                                                b.on_click(cx.listener(|this, _, _, cx| {
                                                    this.run(
                                                        Pending::OpenWeb,
                                                        ShareCommand::OpenWebCopy,
                                                        cx,
                                                    )
                                                }))
                                            })
                                            .child(if opening {
                                                spinner(
                                                    "share-open-web-spin",
                                                    16.,
                                                    colors.foreground,
                                                )
                                            } else {
                                                icon(
                                                    "sharing/ExternalLinkIcon.svg",
                                                    16.,
                                                    colors.foreground,
                                                )
                                                .into_any_element()
                                            })
                                            .child("Open web copy"),
                                    )
                                    .child(
                                        button(
                                            "share-keep-desktop",
                                            true,
                                            paid && !keeping,
                                            colors,
                                        )
                                        .when(paid && !keeping, |b| {
                                            b.on_click(cx.listener(|this, _, _, cx| {
                                                this.run(
                                                    Pending::KeepDesktop,
                                                    ShareCommand::KeepDesktop,
                                                    cx,
                                                )
                                            }))
                                        })
                                        .when(keeping, |b| {
                                            b.child(spinner(
                                                "share-keep-spin",
                                                16.,
                                                colors.primary_foreground,
                                            ))
                                        })
                                        .child("Keep desktop edits"),
                                    ),
                            ),
                    ),
            )
    }
}
