//! The account-backed settings pages: `settings/general/account.tsx`, `billing.tsx`, and the
//! signed-out and plan-gated states of `settings/team` and `settings/sync`. Signed in, those
//! two are `cloud::teams_view` and `cloud::sync_view`.

use std::sync::Arc;

use chrono::{Local, TimeZone};
use desktop_runtime::CancellationToken;
use gpui::{
    AnyElement, Context, Div, FontWeight, Hsla, SharedString, Stateful, div, prelude::*, px, rgb,
};

use super::{
    ProductPane,
    account::{self, Account, Tier, TierAction, WorkspaceTier},
    controls,
    services::{Action, Mutation, Operation, Outcome, ProductServices, Request, Surface},
};
use crate::ui::a11y::Accessible;
use crate::ui::theme::Theme;

const UNCONFIGURED: &str =
    "Cloud sign-in needs VITE_SUPABASE_URL and VITE_SUPABASE_ANON_KEY at build or run time.";
const PRO_MONTHLY: u32 = 14;
const PRO_YEARLY: u32 = 140;

/// Page-local state, reset when the settings pane is rebuilt.
#[derive(Default)]
pub struct AccountUi {
    pub signing_in: bool,
    pub confirm_sign_out: bool,
    pub signing_out: bool,
    pub busy: bool,
    pub yearly: bool,
    pub error: Option<String>,
}

struct PlanTier {
    id: Tier,
    name: &'static str,
    price: &'static str,
    period: &'static str,
    subtitle: Option<&'static str>,
    description: &'static str,
    features: &'static [&'static str],
}

/// `PLAN_TIERS` from `@anlg/pricing`, with only the included features.
const PLAN_TIERS: [PlanTier; 3] = [
    PlanTier {
        id: Tier::Free,
        name: "Free",
        price: "$0",
        period: "/month",
        subtitle: None,
        description: "Private, local meeting notes with on-device models or your own API keys.",
        features: &[
            "Unlimited on-device transcription",
            "Local recordings and audio player",
            "Bring your own keys for STT and AI",
            "Notes, folders, and community templates",
            "Chat and exports",
            "Local API, CLI, MCP, and webhooks",
            "Manual Speaker Labeling",
        ],
    },
    PlanTier {
        id: Tier::Pro,
        name: "Pro",
        price: "$14",
        period: "/person/month",
        subtitle: Some("or $140/person/year"),
        description: "Hosted transcription, AI, dictation, sync, and collaboration for individuals and teams.",
        features: &[
            "Everything in Free",
            "Scheduled auto-recording and joining",
            "Create and edit custom templates",
            "Advanced AI context and web research",
            "System-wide dictation",
            "Cloud Transcription",
            "Cloud LLM",
            "Better Speaker Identification",
            "Cloud sync (3 personal devices or 5 per workspace member)",
            "End-to-end encryption (E2EE)",
            "Share individual notes",
            "Integrations and personal automations",
            "Folder sharing with access controls",
            "Custom dictionaries and summary formats",
            "Shared workspaces and notes",
            "Members, roles, and invitations",
            "Centralized per-seat billing",
            "Shared team folders",
            "Shared team templates",
            "Shared team automations",
        ],
    },
    PlanTier {
        id: Tier::Enterprise,
        name: "Enterprise",
        price: "Custom",
        period: "",
        subtitle: Some("Founder-led rollout"),
        description: "Organization-wide security, policy, and deployment controls with a founder-led rollout.",
        features: &[
            "Everything in Pro",
            "Domain SSO and SCIM",
            "Sharing, retention, and consent policies",
            "Usage and audit visibility",
            "Custom workspace subdomain",
            "Customer-hosted capture and data plane",
            "Founder-led security review and rollout",
        ],
    },
];

fn date(seconds: i64) -> String {
    Local
        .timestamp_opt(seconds, 0)
        .single()
        .map(|date| date.format("%b %-d, %Y").to_string())
        .unwrap_or_default()
}

fn days_left(days: i64) -> String {
    if days == 1 {
        "1 day left".into()
    } else {
        format!("{days} days left")
    }
}

fn text_sm(colors: &Theme, muted: bool) -> Div {
    div()
        .text_sm()
        .line_height(px(20.))
        .when(muted, |view| view.text_color(colors.muted_foreground))
}

fn heading(text: &'static str) -> Stateful<Div> {
    div()
        .id(text)
        .a11y_heading(text, 3)
        .text_sm()
        .line_height(px(20.))
        .font_weight(crate::ui::theme::MEDIUM)
        .child(text)
}

/// `rounded-pill px-3 py-1.5 text-xs` (or the `px-2 py-0.5 text-[10px]` chip).
fn pill(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    primary: bool,
    chip: bool,
    colors: &Theme,
) -> Stateful<Div> {
    let hover = if primary {
        colors.primary.opacity(0.9)
    } else {
        colors.muted
    };
    let foreground = colors.foreground;
    div()
        .id(id.into())
        .flex_shrink_0()
        .when(chip, |view| {
            view.px_2()
                .py(px(2.))
                .text_size(px(10.))
                .line_height(px(15.))
        })
        .when(!chip, |view| {
            view.px_3().py(px(6.)).text_xs().line_height(px(16.))
        })
        .font_weight(crate::ui::theme::MEDIUM)
        .cursor_pointer()
        .when(primary, |view| {
            view.bg(colors.primary)
                .text_color(colors.primary_foreground)
                .hover(move |style| style.bg(hover))
        })
        .when(!primary, |view| {
            view.bg(colors.muted)
                .text_color(colors.muted_foreground)
                .hover(move |style| style.text_color(foreground))
        })
        .map(|view| {
            let label = label.into();
            view.a11y_button(label.clone()).child(label)
        })
}

fn status_chip(label: &'static str, emphasis: bool, colors: &Theme) -> Stateful<Div> {
    div()
        .id(label)
        .a11y_text(label)
        .px_2()
        .py(px(2.))
        .text_size(px(10.))
        .line_height(px(15.))
        .font_weight(crate::ui::theme::MEDIUM)
        .when(emphasis, |view| {
            view.bg(colors.primary)
                .text_color(colors.primary_foreground)
        })
        .when(!emphasis, |view| {
            view.bg(colors.muted).text_color(colors.muted_foreground)
        })
        .child(label)
}

/// `Button` with the destructive variant (`h-9 px-4`).
fn destructive_button(
    id: &'static str,
    label: &'static str,
    enabled: bool,
    colors: &Theme,
) -> Stateful<Div> {
    let hover = colors.destructive.opacity(0.9);
    div()
        .id(id)
        .h(px(36.))
        .px_4()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .bg(colors.destructive)
        .text_color(colors.destructive_foreground)
        .shadow_xs()
        .text_sm()
        .font_weight(crate::ui::theme::MEDIUM)
        .a11y_button(label)
        .a11y_disabled(!enabled)
        .child(label)
        .when(!enabled, |view| view.opacity(0.5))
        .when(enabled, |view| {
            view.cursor_pointer().hover(move |style| style.bg(hover))
        })
}

/// The small outline `Button` (`size="sm"`: `h-7 px-2 text-xs`).
fn small_outline(
    id: &'static str,
    icon: Option<&'static str>,
    label: &'static str,
    colors: &Theme,
) -> Stateful<Div> {
    let hover = colors.accent;
    div()
        .id(id)
        .a11y_button(label)
        .h(px(28.))
        .px_2()
        .flex()
        .flex_shrink_0()
        .items_center()
        .gap_2()
        .border_1()
        .border_color(colors.input)
        .bg(colors.background)
        .shadow_xs()
        .text_xs()
        .font_weight(crate::ui::theme::MEDIUM)
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .when_some(icon, |view, icon| {
            view.child(controls::icon(icon, 14., colors.foreground))
        })
        .child(label)
}

/// `Container` from `account.tsx`: title and description with the action on the right.
fn container(
    title: &'static str,
    description: impl IntoElement,
    action: Option<AnyElement>,
) -> Div {
    div()
        .min_w_0()
        .flex()
        .items_start()
        .justify_between()
        .gap_4()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_2()
                .child(heading(title))
                .child(description),
        )
        .when_some(action, |view, action| {
            view.child(div().flex_shrink_0().child(action))
        })
}

fn page(title: &'static str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_8()
        .child(controls::page_title(title))
}

fn section(title: &'static str, description: Option<&'static str>, colors: &Theme) -> Div {
    div()
        .mb_2()
        .flex()
        .flex_col()
        .gap_1()
        .child(controls::section_title(title))
        .when_some(description, |view, description| {
            view.child(
                text_sm(colors, true)
                    .id(description)
                    .a11y_text(description)
                    .child(description),
            )
        })
}

/// `PlanGate`: visible at 60% opacity; pressing it shows the plan-required toast instead.
pub fn plan_gate(
    allowed: bool,
    content: impl IntoElement,
    on_blocked: impl Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> Div {
    let gate = div().relative().child(content);
    if allowed {
        return gate;
    }
    gate.opacity(0.6).child(
        div()
            .absolute()
            .inset_0()
            .occlude()
            .cursor_not_allowed()
            .on_mouse_down(gpui::MouseButton::Left, on_blocked),
    )
}

fn emerald(colors: &Theme) -> Hsla {
    // `text-emerald-600` / `dark:text-emerald-400`; the design tokens have no green.
    if colors.background.l < 0.5 {
        rgb(0x34d399).into()
    } else {
        rgb(0x059669).into()
    }
}

impl ProductPane {
    fn request(&self) -> Request {
        Request {
            scope: self.scope.clone(),
            generation: 0,
            cancel: CancellationToken::new(),
        }
    }

    /// Runs an account mutation through the cloud service and reloads the account after it.
    fn account_action(
        &mut self,
        surface: Surface,
        action: Action,
        fields: Vec<(&'static str, String)>,
        cx: &mut Context<Self>,
    ) {
        let Some(cloud) = self.cloud.clone() else {
            self.account_ui.error = Some(UNCONFIGURED.into());
            self.account_ui.signing_out = false;
            cx.notify();
            return;
        };
        let request = self.request();
        let mutation = Mutation {
            operation: Operation {
                id: format!("{action:?}").into(),
                label: "".into(),
                action: action.clone(),
                target_id: None,
                disabled_reason: None,
                confirmation: None,
            },
            fields: fields
                .into_iter()
                .map(|(key, value)| (Arc::from(key), Arc::from(value)))
                .collect(),
        };
        self.account_ui.busy = true;
        self.account_ui.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cloud.perform(surface, request, mutation).await;
            let _ = this.update(cx, |this, cx| {
                this.account_ui.busy = false;
                match result {
                    Ok(Outcome::IdentityChanged(scope)) => {
                        this.account_ui = Default::default();
                        this.set_scope(scope.clone(), cx);
                        cx.emit(scope);
                    }
                    Ok(_) => {
                        if action == Action::SignIn {
                            this.account_ui.signing_in = true;
                        }
                    }
                    Err(error) => {
                        this.account_ui.signing_out = false;
                        this.account_ui.error = Some(if action == Action::SignOut {
                            "Anarlog couldn't sign you out. Try again.".into()
                        } else {
                            error.to_string()
                        });
                    }
                }
                cloud.publish_account(cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// Opens an Anarlog web page (account, billing portal, checkout) in the browser.
    fn open_web(
        &mut self,
        path: &'static str,
        params: Vec<(&'static str, String)>,
        fragment: Option<&'static str>,
        cx: &mut Context<Self>,
    ) {
        let Some(cloud) = self.cloud.clone() else {
            self.account_ui.error = Some(UNCONFIGURED.into());
            cx.notify();
            return;
        };
        self.account_ui.busy = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cloud.open_web(path, params, fragment).await;
            let _ = this.update(cx, |this, cx| {
                this.account_ui.busy = false;
                this.account_ui.error = result.err().map(|error| error.to_string());
                cx.notify();
            });
        })
        .detach();
    }

    fn profile_card(&mut self, account: &Account, cx: &mut Context<Self>) -> AnyElement {
        let key = super::profile::key(account);
        if self
            .profile
            .as_ref()
            .is_none_or(|(current, _)| *current != key)
        {
            if let Some((_, card)) = &self.profile {
                card.update(cx, |card, cx| card.flush(cx));
            }
            let user = account
                .user
                .as_ref()
                .map(|user| (user.id.clone(), user.email.clone()));
            let runtime = self.context.runtime.clone();
            let card = cx.new(|cx| super::profile::ProfileCard::new(runtime, user, cx));
            self.profile = Some((key, card));
        }
        self.profile
            .as_ref()
            .map(|(_, card)| card.clone().into_any_element())
            .unwrap_or_else(|| div().into_any_element())
    }

    fn error_notice(&self, colors: &Theme) -> Option<Div> {
        self.account_ui.error.as_ref().map(|error| {
            div()
                .text_sm()
                .text_color(colors.destructive)
                .child(error.clone())
        })
    }

    /// Pages whose shipping content depends on the Anarlog account.
    pub(super) fn account_page(&mut self, colors: &Theme, cx: &mut Context<Self>) -> Option<Div> {
        let account = account::current(cx);
        let now = account::now();
        let signed_in = account.signed_in();
        let pro = account.is_pro(now);
        Some(match self.active.id {
            "account" => self.account(&account, colors, cx),
            "billing" => self.billing(&account, colors, cx),
            "team" if !signed_in => page("Teams").child(
                text_sm(colors, true).child("Sign in to create a shared workspace for your team."),
            ),
            "sync" if !signed_in => page("Sync").child(self.sync_signed_out(colors, cx)),
            "sync" if !pro => page("Sync").child(plan_gate(
                false,
                sync_preview(colors),
                cx.listener(|this, _, _, cx| this.notify_plan_required(cx)),
            )),
            "referrals" => self.referrals(&account, colors, cx),
            "crm" => {
                let view = self
                    .crm
                    .get_or_insert_with(|| cx.new(super::crm::CrmView::new))
                    .clone();
                div().child(view)
            }
            "imports" => {
                if self.imports.is_none() {
                    let runtime = self.context.runtime.clone();
                    let view =
                        cx.new(|cx| super::local::imports_view::ImportsView::new(runtime, cx));
                    let subscription = cx.subscribe(
                        &view,
                        |this, _, _: &super::local::imports_view::SignInRequested, cx| {
                            this.account_action(Surface::Account, Action::SignIn, vec![], cx)
                        },
                    );
                    self.imports = Some((view, subscription));
                }
                div().children(self.imports.as_ref().map(|(view, _)| view.clone()))
            }
            "insights" => {
                let user = account
                    .user
                    .as_ref()
                    .map_or("00000000-0000-0000-0000-000000000000".into(), |user| {
                        user.id.to_string()
                    });
                if self.insights.as_ref().is_none_or(|(id, _)| *id != user) {
                    let runtime = self.context.runtime.clone();
                    let id = user.clone();
                    let view = cx.new(|cx| super::insights::InsightsView::new(runtime, id, cx));
                    self.insights = Some((user, view));
                }
                div().children(self.insights.as_ref().map(|(_, view)| view.clone()))
            }
            _ => return None,
        })
    }

    fn account(&mut self, account: &Account, colors: &Theme, cx: &mut Context<Self>) -> Div {
        let profile = self.profile_card(account, cx);
        let error = self.error_notice(colors);
        let Some(user) = account.user.clone() else {
            if self.account_ui.signing_in {
                return page("Account")
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(container(
                                "Finish sign-in",
                                text_sm(colors, true).child("Finish in your browser, then return to Anarlog."),
                                Some(
                                    controls::outline_button(
                                        "reopen-sign-in",
                                        "Reopen sign-in page",
                                        !self.account_ui.busy,
                                        colors,
                                        Some(Box::new(cx.listener(|this, _, _, cx| {
                                            this.account_action(Surface::Account, Action::SignIn, vec![], cx)
                                        }))),
                                    )
                                    .into_any_element(),
                                ),
                            ))
                            .child(
                                div()
                                    .mt_4()
                                    .text_xs()
                                    .line_height(px(16.))
                                    .text_color(colors.muted_foreground)
                                    .child("If Anarlog stays closed, paste the link in the sign-in window."),
                            ),
                    )
                    .children(error);
            }
            let hover = colors.primary.opacity(0.9);
            return page("Account")
                .child(
                    div()
                        .pb_4()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .items_start()
                        .gap_4()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(heading("Sign in to Anarlog"))
                                .child(text_sm(colors, true).child(
                                    "Sign in for cloud transcription, AI models, and sharing.",
                                )),
                        )
                        .child(
                            div()
                                .id("account-get-started")
                                .a11y_button("Get started")
                                .h(px(40.))
                                .px_6()
                                .flex()
                                .items_center()
                                .border_2()
                                .border_color(colors.primary)
                                .bg(colors.primary)
                                .text_color(colors.primary_foreground)
                                .text_sm()
                                .font_weight(crate::ui::theme::MEDIUM)
                                // `shadow-[0_4px_14px_rgba(87,83,78,0.4)]`; GPUI's blur
                                // radius is the Gaussian sigma, half of CSS's blur.
                                .shadow(vec![gpui::BoxShadow {
                                    color: gpui::hsla(34. / 360., 0.05, 0.33, 0.4),
                                    offset: gpui::point(px(0.), px(4.)),
                                    blur_radius: px(7.),
                                    spread_radius: px(0.),
                                    inset: false,
                                }])
                                .cursor_pointer()
                                .hover(move |style| style.bg(hover))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.account_action(
                                        Surface::Account,
                                        Action::SignIn,
                                        vec![],
                                        cx,
                                    )
                                }))
                                .child("Get started"),
                        )
                        .children(error),
                )
                .child(profile);
        };
        let email_hover = colors.foreground;
        let user_id = user.id.clone();
        let busy = self.account_ui.busy;
        page("Account")
            .child(container(
                "Your Account",
                match user.email.clone() {
                    Some(email) => div()
                        .id("account-email")
                        .a11y_button(format!("{email}, Edit"))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .text_sm()
                        .line_height(px(20.))
                        .text_color(colors.muted_foreground)
                        .cursor_pointer()
                        .hover(move |style| style.text_color(email_hover))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.open_web("/app/account", vec![], None, cx)
                        }))
                        .child(email.to_string())
                        .child(controls::icon(
                            "product/PencilEdit01Icon.svg",
                            12.,
                            colors.muted_foreground,
                        ))
                        .into_any_element(),
                    None => text_sm(colors, true).child("Signed in").into_any_element(),
                },
                Some(
                    destructive_button(
                        "account-sign-out",
                        if self.account_ui.signing_out {
                            "Signing out..."
                        } else {
                            "Sign out"
                        },
                        !self.account_ui.signing_out,
                        colors,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.account_ui.confirm_sign_out = true;
                        cx.notify();
                    }))
                    .into_any_element(),
                ),
            ))
            .child(container(
                "Connected accounts",
                text_sm(colors, true).child("Manage your sign-in methods on the Anarlog website."),
                Some(
                    controls::outline_button(
                        "account-connected",
                        "Manage sign-in methods",
                        !busy,
                        colors,
                        Some(Box::new(cx.listener(move |this, _, _, cx| {
                            this.open_web(
                                "/app/account",
                                vec![
                                    ("flow", "web".into()),
                                    ("section", "connected-accounts".into()),
                                    ("account_user_id", user_id.to_string()),
                                ],
                                Some("connected-accounts"),
                                cx,
                            )
                        }))),
                    )
                    .into_any_element(),
                ),
            ))
            .children(error)
            .child(profile)
    }

    /// `DestructiveConfirmationDialog` for signing out, drawn over the whole pane.
    pub(super) fn sign_out_dialog(&self, colors: &Theme, cx: &mut Context<Self>) -> Option<Div> {
        if !self.account_ui.confirm_sign_out {
            return None;
        }
        let pending = self.account_ui.signing_out;
        let cancel_hover = colors.background.opacity(0.8);
        let confirm_hover = colors.destructive.opacity(0.9);
        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::black().opacity(0.4))
                .occlude()
                .child(
                    div()
                        .id("sign-out-dialog")
                        .a11y_dialog("Sign out of Anarlog?")
                        .aria_description(
                            "You'll need to sign in again to use cloud sync and account features.",
                        )
                        .w(px(320.))
                        .p_5()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .border_1()
                        .border_color(colors.border.opacity(0.45))
                        .bg(colors.card)
                        .shadow_xl()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_2()
                                .text_center()
                                .text_size(px(13.))
                                .child(div().line_height(px(20.)).font_weight(FontWeight::SEMIBOLD).child("Sign out of Anarlog?"))
                                .child(div().line_height(px(17.7)).child("You'll need to sign in again to use cloud sync and account features.")),
                        )
                        .child(
                            div()
                                .grid()
                                .grid_cols(2)
                                .gap_2()
                                .child(
                                    div()
                                        .id("sign-out-cancel")
                                        .a11y_button("Cancel")
                                        .h(px(32.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .border_1()
                                        .border_color(colors.border.opacity(0.7))
                                        .bg(colors.background.opacity(0.5))
                                        .text_xs()
                                        .font_weight(crate::ui::theme::MEDIUM)
                                        .shadow_xs()
                                        .when(pending, |view| view.opacity(0.5))
                                        .when(!pending, |view| {
                                            view.cursor_pointer()
                                                .hover(move |style| style.bg(cancel_hover))
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.account_ui.confirm_sign_out = false;
                                                    cx.notify();
                                                }))
                                        })
                                        .child("Cancel"),
                                )
                                .child(
                                    div()
                                        .id("sign-out-confirm")
                                        .a11y_button(if pending { "Signing out..." } else { "Sign out" })
                                        .h(px(32.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .bg(colors.destructive)
                                        .text_color(colors.destructive_foreground)
                                        .text_xs()
                                        .font_weight(crate::ui::theme::MEDIUM)
                                        .shadow_sm()
                                        .when(pending, |view| view.opacity(0.5))
                                        .when(!pending, |view| {
                                            view.cursor_pointer()
                                                .hover(move |style| style.bg(confirm_hover))
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.account_ui.signing_out = true;
                                                    this.account_action(Surface::Account, Action::SignOut, vec![], cx);
                                                }))
                                        })
                                        .child(if pending { "Signing out..." } else { "Sign out" }),
                                ),
                        ),
                ),
        )
    }

    fn billing(&mut self, account: &Account, colors: &Theme, cx: &mut Context<Self>) -> Div {
        if !account.signed_in() {
            let primary = colors.primary;
            return page("Billing")
                .child(
                    div().flex().child(
                        div()
                            .id("billing-sign-in")
                            .a11y_button("Sign in to Anarlog")
                            .text_sm()
                            .line_height(px(20.))
                            .font_weight(crate::ui::theme::MEDIUM)
                            .text_color(primary)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.select_page(super::navigation::page("account"), cx)
                            }))
                            .child("Sign in to Anarlog"),
                    ),
                )
                .child(
                    div()
                        .child(
                            div()
                                .mb_4()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(controls::section_title("Plans"))
                                .child(
                                    text_sm(colors, true)
                                        .child("Compare Free, Pro, Team, and Enterprise."),
                                ),
                        )
                        .child(self.plan_tiers(account, Some(Tier::Free), false, colors, cx)),
                );
        }
        let now = account::now();
        let billing = account.billing(now);
        let tier = account.tier(now);
        let workspace_plan = account.workspace_tier().is_some();
        let trialing = !workspace_plan && billing.is_trialing;
        let paused = !workspace_plan && billing.is_paused;
        page("Billing")
            .child(self.plan_card(account, colors, cx))
            .when_some(self.plan_limits(account, colors), |view, limits| {
                view.child(limits)
            })
            .child(
                div()
                    .child(section("Compare plans", None, colors))
                    .when(tier == Some(Tier::Free) && !paused, |view| {
                        view.child(self.period_toggle(colors, cx))
                    })
                    .child(self.plan_tiers(account, tier, trialing, colors, cx)),
            )
            .children(self.error_notice(colors))
    }

    fn plan_card(&mut self, account: &Account, colors: &Theme, cx: &mut Context<Self>) -> Div {
        let now = account::now();
        let billing = account.billing(now);
        let tier = account.tier(now);
        let workspace_plan = account.workspace_tier().is_some();
        let trialing = !workspace_plan && billing.is_trialing;
        let paused = !workspace_plan && billing.is_paused;
        let paid = workspace_plan || billing.is_paid;
        let busy = self.account_ui.busy;
        let label = match tier {
            Some(Tier::Pro) => "Pro",
            Some(Tier::Enterprise) => "Enterprise",
            _ => "Free",
        };
        let pro = tier == Some(Tier::Pro);
        let needs_payment = pro && trialing && !billing.has_payment_method;
        let price = match tier {
            Some(Tier::Pro) if trialing => Some(format!("${PRO_MONTHLY} /month after trial")),
            Some(Tier::Pro) if paid => None,
            Some(tier) => PLAN_TIERS
                .iter()
                .find(|plan| plan.id == tier)
                .map(|plan| format!("{}{}", plan.price, plan.period)),
            None => None,
        };
        let status: Option<AnyElement> = match tier {
            None => None,
            Some(Tier::Pro) if trialing => {
                let mut text = "Pro trial".to_owned();
                if let Some(days) = billing.trial_days_remaining {
                    text.push_str(&format!(" - {}", days_left(days)));
                }
                if let Some(end) = billing.trial_end {
                    text.push_str(&format!(" · ends {}", date(end)));
                }
                Some(div().child(text).into_any_element())
            }
            _ if paused => Some(div().child("Your Pro trial has ended").into_any_element()),
            _ => {
                let renewal = (!workspace_plan && paid)
                    .then_some(billing.current_period_end)
                    .flatten()
                    .map(|end| {
                        format!(
                            " · {} {}",
                            if billing.cancel_at_period_end {
                                "ends"
                            } else {
                                "renews"
                            },
                            date(end)
                        )
                    });
                Some(
                    div()
                        .flex()
                        .child("You're on the\u{a0}")
                        .child(div().font_weight(FontWeight::SEMIBOLD).child(label))
                        .child("\u{a0}plan")
                        .when_some(renewal, |view, renewal| view.child(renewal))
                        .into_any_element(),
                )
            }
        };
        let action: AnyElement = match tier {
            None => div()
                .w(px(96.))
                .h(px(28.))
                .bg(colors.muted)
                .into_any_element(),
            Some(_) if needs_payment => pill(
                "billing-add-payment",
                "Add payment method",
                true,
                false,
                colors,
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.open_web(
                    "/app/portal",
                    vec![("intent", "payment_method_update".into())],
                    None,
                    cx,
                )
            }))
            .when(busy, |view| view.opacity(0.5))
            .into_any_element(),
            Some(_) if paused => pill("billing-resume", "Resume", true, false, colors)
                .on_click(
                    cx.listener(|this, _, _, cx| this.open_web("/app/portal", vec![], None, cx)),
                )
                .when(busy, |view| view.opacity(0.5))
                .into_any_element(),
            Some(_) if workspace_plan => {
                pill("billing-open-teams", "Open Teams", false, false, colors)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.select_page(super::navigation::page("team"), cx)
                    }))
                    .into_any_element()
            }
            Some(Tier::Pro) if paid => {
                pill("billing-manage", "Manage billing", false, false, colors)
                    .on_click(
                        cx.listener(|this, _, _, cx| {
                            this.open_web("/app/portal", vec![], None, cx)
                        }),
                    )
                    .when(busy, |view| view.opacity(0.5))
                    .into_any_element()
            }
            Some(Tier::Free) => {
                match account::tier_action(Tier::Pro, Tier::Free, account.can_start_trial(now)) {
                    Some(action) => pill(
                        "billing-upgrade",
                        if action == TierAction::StartTrial {
                            "Start free trial"
                        } else {
                            "Get Pro"
                        },
                        true,
                        false,
                        colors,
                    )
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.run_tier_action(action, false, cx)),
                    )
                    .when(busy, |view| view.opacity(0.5))
                    .into_any_element(),
                    None => div().into_any_element(),
                }
            }
            _ => div().into_any_element(),
        };
        div()
            .child(section(
                "Your plan",
                Some("Manage or cancel your subscription in the billing portal."),
                colors,
            ))
            .child(
                div()
                    .px_4()
                    .py_4()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .border_1()
                    .border_color(colors.border.opacity(0.6))
                    .child(
                        div()
                            .min_w_0()
                            .when(tier.is_none(), |view| {
                                view.child(div().w(px(160.)).h(px(20.)).bg(colors.muted))
                            })
                            .when(tier.is_some(), |view| {
                                view.child(
                                    div()
                                        .text_base()
                                        .line_height(px(24.))
                                        .font_weight(crate::ui::theme::MEDIUM)
                                        .child(format!("Anarlog {label}")),
                                )
                                .child(
                                    text_sm(colors, true)
                                        .mt_1()
                                        .flex()
                                        .flex_col()
                                        .gap(px(2.))
                                        .when_some(price, |view, price| view.child(price))
                                        .when_some(status, |view, status| view.child(status)),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(action)
                            .child(
                                div()
                                    .id("billing-refresh")
                                    .cursor_pointer()
                                    .when(busy, |view| view.opacity(0.5))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.account_action(
                                            Surface::Billing,
                                            Action::RefreshAccount,
                                            vec![],
                                            cx,
                                        )
                                    }))
                                    .child(controls::icon(
                                        "product/RefreshIcon.svg",
                                        12.,
                                        colors.muted_foreground,
                                    )),
                            ),
                    ),
            )
    }

    fn plan_limits(&self, account: &Account, colors: &Theme) -> Option<Div> {
        let now = account::now();
        let billing = account.billing(now);
        let row = |label: &'static str,
                   sublabel: Option<String>,
                   metric: String,
                   fraction: Option<f32>| {
            div()
                .px_4()
                .py_3()
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .justify_between()
                        .gap_4()
                        .child(heading(label))
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_sm()
                                .line_height(px(20.))
                                .child(metric),
                        ),
                )
                .when_some(sublabel, |view, sublabel| {
                    view.child(
                        div()
                            .mt(px(2.))
                            .text_xs()
                            .line_height(px(16.))
                            .text_color(colors.muted_foreground)
                            .child(sublabel),
                    )
                })
                .when_some(fraction, |view, fraction| {
                    view.child(
                        div()
                            .mt_2()
                            .h(px(6.))
                            .w_full()
                            .overflow_hidden()
                            .bg(colors.muted)
                            .child(
                                div()
                                    .h_full()
                                    .w(gpui::relative(fraction.clamp(0., 1.)))
                                    .bg(colors.primary),
                            ),
                    )
                })
        };
        let mut rows = Vec::new();
        if account.workspace_tier().is_none()
            && billing.is_trialing
            && let Some(days) = billing.trial_days_remaining
        {
            rows.push(row(
                "Pro trial",
                billing.trial_end.map(|end| format!("Ends {}", date(end))),
                days_left(days.max(0)),
                None,
            ));
        }
        if let Some(devices) = &account.devices {
            rows.push(row(
                "Synced devices",
                (devices.used >= devices.max).then(|| "At your device limit".into()),
                format!("{} of {} used", devices.used, devices.max),
                Some(devices.used as f32 / devices.max as f32),
            ));
        }
        for workspace in account.workspaces.iter().flat_map(|w| w.iter()) {
            if workspace.tier == WorkspaceTier::Free {
                continue;
            }
            rows.push(row(
                "Team seats",
                Some(workspace.name.to_string()),
                match workspace.seat_limit {
                    Some(limit) => format!("{} of {limit} used", workspace.used_seats),
                    None => format!("{} in use", workspace.used_seats),
                },
                workspace
                    .seat_limit
                    .filter(|limit| *limit > 0)
                    .map(|limit| workspace.used_seats as f32 / limit as f32),
            ));
        }
        if rows.is_empty() {
            return None;
        }
        let count = rows.len();
        Some(
            div()
                .child(section(
                    "Plan limits",
                    Some("Shared across cloud sync and your workspaces."),
                    colors,
                ))
                .child(
                    div()
                        .border_1()
                        .border_color(colors.border.opacity(0.6))
                        .children(rows.into_iter().enumerate().map(|(index, row)| {
                            row.when(index + 1 < count, |view| {
                                view.border_b_1().border_color(colors.border.opacity(0.6))
                            })
                        })),
                ),
        )
    }

    fn period_toggle(&self, colors: &Theme, cx: &mut Context<Self>) -> Div {
        let savings = PRO_MONTHLY * 12 - PRO_YEARLY;
        let options = [
            (false, format!("Monthly · ${PRO_MONTHLY}/mo")),
            (true, format!("Yearly · ${PRO_YEARLY}/yr (save ${savings})")),
        ];
        let foreground = colors.foreground;
        div().flex().child(
            div()
                .mb_4()
                .p(px(2.))
                .flex()
                .items_center()
                .gap(px(2.))
                .bg(colors.muted)
                .children(options.into_iter().map(|(yearly, label)| {
                    let selected = self.account_ui.yearly == yearly;
                    div()
                        .id(if yearly {
                            "period-yearly"
                        } else {
                            "period-monthly"
                        })
                        .px(px(10.))
                        .py_1()
                        .text_xs()
                        .line_height(px(16.))
                        .font_weight(crate::ui::theme::MEDIUM)
                        .cursor_pointer()
                        .when(selected, |view| {
                            view.bg(colors.background)
                                .text_color(colors.foreground)
                                .shadow_sm()
                        })
                        .when(!selected, |view| {
                            view.text_color(colors.muted_foreground)
                                .hover(move |style| style.text_color(foreground))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.account_ui.yearly = yearly;
                            cx.notify();
                        }))
                        .child(label)
                })),
        )
    }

    fn run_tier_action(&mut self, action: TierAction, paused: bool, cx: &mut Context<Self>) {
        let period = if self.account_ui.yearly {
            "yearly"
        } else {
            "monthly"
        };
        match action {
            TierAction::Current => {}
            TierAction::StartTrial => self.open_web(
                "/app/checkout",
                vec![
                    ("period", period.into()),
                    ("trial", "true".into()),
                    ("source", "settings".into()),
                ],
                None,
                cx,
            ),
            TierAction::Checkout if paused => self.open_web("/app/portal", vec![], None, cx),
            TierAction::Checkout => self.open_web(
                "/app/checkout",
                vec![
                    ("plan", "pro".into()),
                    ("period", period.into()),
                    ("source", "settings".into()),
                ],
                None,
                cx,
            ),
        }
    }

    /// `PlanTierList`: the plans side by side with their status chips and actions.
    fn plan_tiers(
        &self,
        account: &Account,
        current: Option<Tier>,
        trialing: bool,
        colors: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let now = account::now();
        let billing = account.billing(now);
        let signed_in = account.signed_in();
        let paused = account.workspace_tier().is_none() && billing.is_paused;
        let can_start_trial = signed_in && account.can_start_trial(now);
        let highlight_pro = current == Some(Tier::Free);
        let needs_payment = current == Some(Tier::Pro) && trialing && !billing.has_payment_method;
        let check = emerald(colors);
        div()
            .grid()
            .grid_cols(2)
            .gap_x_10()
            .gap_y_8()
            .children(PLAN_TIERS.iter().map(|tier| {
                let is_current = Some(tier.id) == current;
                let is_pro = tier.id == Tier::Pro;
                let action = match (tier.id, current) {
                    (Tier::Free | Tier::Pro, Some(current @ (Tier::Free | Tier::Pro))) => {
                        account::tier_action(tier.id, current, can_start_trial)
                    }
                    _ => None,
                };
                let control: Option<AnyElement> = if !signed_in {
                    None
                } else if tier.id == Tier::Enterprise {
                    Some(
                        pill("tier-enterprise", "Talk to sales", false, true, colors)
                            .on_click(|_, _, cx| cx.open_url("https://anarlog.so/enterprise/"))
                            .into_any_element(),
                    )
                } else {
                    match action {
                        None => None,
                        Some(TierAction::Current) => needs_payment.then(|| {
                            pill("tier-add-payment", "Add payment method", true, true, colors)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.open_web(
                                        "/app/portal",
                                        vec![("intent", "payment_method_update".into())],
                                        None,
                                        cx,
                                    )
                                }))
                                .into_any_element()
                        }),
                        Some(action) => Some(
                            pill(
                                SharedString::from(format!("tier-action-{}", tier.name)),
                                if paused {
                                    "Resume"
                                } else if action == TierAction::StartTrial {
                                    "Start free trial"
                                } else {
                                    "Get Pro"
                                },
                                true,
                                true,
                                colors,
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_tier_action(action, paused, cx)
                            }))
                            .into_any_element(),
                        ),
                    }
                };
                let details = if highlight_pro && tier.id == Tier::Free {
                    div()
                        .text_xs()
                        .line_height(px(16.))
                        .text_color(colors.muted_foreground)
                        .child("On-device transcription, recordings, and your own keys.")
                } else {
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div()
                                .text_xs()
                                .line_height(px(20.))
                                .text_color(colors.muted_foreground)
                                .child(tier.description),
                        )
                        .child(div().flex().flex_col().gap(px(6.)).children(
                            tier.features.iter().map(|feature| {
                                div()
                                    .flex()
                                    .items_start()
                                    .gap(px(6.))
                                    .child(div().h_4().flex().flex_shrink_0().items_center().child(
                                        controls::icon(
                                            "product/CheckmarkCircle02Icon.svg",
                                            14.,
                                            check,
                                        ),
                                    ))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .min_h(px(16.))
                                            .text_xs()
                                            .line_height(px(16.))
                                            .child(*feature),
                                    )
                            }),
                        ))
                };
                div()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .mb_2()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_base()
                                    .line_height(px(24.))
                                    .font_weight(if is_pro && highlight_pro {
                                        FontWeight::SEMIBOLD
                                    } else {
                                        crate::ui::theme::MEDIUM
                                    })
                                    .child(tier.name),
                            )
                            .when(is_current, |view| {
                                view.child(status_chip("Current", false, colors))
                            })
                            .when(is_current && is_pro && trialing, |view| {
                                view.child(status_chip("Trial", true, colors))
                            })
                            .children(control),
                    )
                    .child(
                        div()
                            .mb_2()
                            .text_color(colors.muted_foreground)
                            .child(
                                div()
                                    .flex()
                                    .items_baseline()
                                    .child(div().text_xl().line_height(px(28.)).child(tier.price))
                                    .when(!tier.period.is_empty(), |view| {
                                        view.child(
                                            div()
                                                .ml_1()
                                                .text_sm()
                                                .line_height(px(20.))
                                                .child(tier.period),
                                        )
                                    }),
                            )
                            .when_some(tier.subtitle, |view, subtitle| {
                                view.child(
                                    div()
                                        .mt(px(2.))
                                        .text_xs()
                                        .line_height(px(16.))
                                        .child(subtitle),
                                )
                            }),
                    )
                    .child(details)
            }))
    }

    fn sync_signed_out(&self, colors: &Theme, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_start()
            .justify_between()
            .gap_4()
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        div()
                            .size(px(36.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(colors.muted)
                            .child(controls::icon(
                                "product/CloudOffIcon.svg",
                                16.,
                                colors.muted_foreground,
                            )),
                    )
                    .child(
                        div().child(heading("Sign in to use cloud sync")).child(
                            div()
                                .mt_1()
                                .text_xs()
                                .line_height(px(20.))
                                .text_color(colors.muted_foreground)
                                .child("Keep notes encrypted and synced across your devices."),
                        ),
                    ),
            )
            .child(
                small_outline("sync-sign-in", None, "Sign in", colors).on_click(cx.listener(
                    |this, _, _, cx| this.select_page(super::navigation::page("account"), cx),
                )),
            )
    }

    fn load_referrals(&mut self, user: Arc<str>, cx: &mut Context<Self>) {
        let Some(cloud) = self.cloud.clone() else {
            self.referrals = Some((user, Some(Err(UNCONFIGURED.into()))));
            return;
        };
        self.referrals = Some((user.clone(), None));
        cx.spawn(async move |this, cx| {
            let result = cloud.referrals().await.map_err(|error| error.to_string());
            let _ = this.update(cx, |this, cx| {
                if this.referrals.as_ref().is_some_and(|(id, _)| *id == user) {
                    this.referrals = Some((user, Some(result)));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// `settings/referrals.tsx`.
    fn referrals(&mut self, account: &Account, colors: &Theme, cx: &mut Context<Self>) -> Div {
        let user = account.user.as_ref().map(|user| user.id.clone());
        if let Some(user) = &user
            && self.referrals.as_ref().is_none_or(|(id, _)| id != user)
        {
            self.load_referrals(user.clone(), cx);
        }
        let muted = colors.muted_foreground;
        let text = |value: String| div().text_sm().line_height(px(20.)).child(value);
        let retry = |cx: &mut Context<Self>| {
            controls::outline_button(
                "referrals-retry",
                "Try again",
                true,
                colors,
                Some(Box::new(cx.listener(|this, _, _, cx| {
                    if let Some((user, _)) = this.referrals.take() {
                        this.load_referrals(user, cx);
                    }
                    cx.notify();
                }))),
            )
        };
        let body = match (
            &user,
            self.referrals.as_ref().and_then(|(_, state)| state.clone()),
        ) {
            (None, _) => div()
                .flex()
                .flex_col()
                .items_start()
                .gap_3()
                .child(text("Sign in to see your referral invites.".into()))
                .child(controls::outline_button(
                    "referrals-sign-in",
                    "Sign in",
                    true,
                    colors,
                    Some(Box::new(cx.listener(|this, _, _, cx| {
                        this.select_page(super::navigation::page("account"), cx)
                    }))),
                )),
            (Some(_), None) => text("Loading your invites...".into()),
            (Some(_), Some(Err(_))) => div()
                .flex()
                .flex_col()
                .items_start()
                .gap_3()
                .child(text(
                    "We couldn't load your invites. Please try again.".into(),
                ))
                .child(retry(cx)),
            (Some(_), Some(Ok(data))) => self.referral_summary(&data, colors, cx),
        };
        page("Invite friends")
            .gap_8()
            .child(body)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .text_sm()
                    .line_height(px(20.))
                    .text_color(muted)
                    .child(
                        div()
                            .text_color(colors.foreground)
                            .font_weight(crate::ui::theme::MEDIUM)
                            .child("Missing a referral reward?"),
                    )
                    .child("If your friend signed up through your link and started their free trial, but your free month hasn't appeared, email us and we'll help.")
                    .child(
                        div()
                            .id("referral-support")
                            .underline()
                            .cursor_pointer()
                            .child("Email us")
                            .on_click(|_, _, cx| {
                                cx.open_url("mailto:team@fastrepl.com?subject=Referral%20reward%20help")
                            }),
                    ),
            )
    }

    fn referral_summary(
        &self,
        data: &super::cloud::Referrals,
        colors: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let muted = colors.muted_foreground;
        let stat = |label: &'static str, value: String| {
            div()
                .flex_1()
                .p_4()
                .border_1()
                .border_color(colors.border)
                .child(
                    div()
                        .text_xs()
                        .line_height(px(16.))
                        .text_color(muted)
                        .child(label),
                )
                .child(
                    div()
                        .mt_2()
                        .text_xl()
                        .line_height(px(28.))
                        .font_weight(crate::ui::theme::MEDIUM)
                        .child(value),
                )
        };
        let copied = self.referral_copied;
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(if data.enabled {
                div()
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.muted.opacity(0.3))
                    .child(
                        div()
                            .text_xl()
                            .line_height(px(28.))
                            .font_weight(crate::ui::theme::MEDIUM)
                            .child("Refer friends, get a month free"),
                    )
                    .child(div().text_sm().line_height(px(22.75)).text_color(muted).child(
                        "Get one free month when a friend signs up through your link and starts their free trial. Invite up to 3 friends. Each friend gets 30 days of Pro free.",
                    ))
                    .child(div().text_xs().line_height(px(16.)).text_color(muted).child(
                        "Your trial or paid subscription is extended by one month. No payment from your friend is required.",
                    ))
            } else {
                div().text_sm().child(
                    "The new referral offer is not available yet. Your previous referrals are shown below.",
                )
            })
            .when(data.enabled, |view| {
                view.child(
                    div()
                        .flex()
                        .gap_3()
                        .child(stat("Accepted", format!("{} / 3", data.accepted)))
                        .child(stat("Free months earned", data.months_earned.to_string()))
                        .child(stat("Invites remaining", data.remaining.to_string())),
                )
                .child(if data.remaining == 0 {
                    div().text_sm().child("All 3 invites have been accepted.")
                } else if let (true, Some(link)) = (data.eligible, data.link.clone()) {
                    div()
                        .p_4()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_3()
                        .border_1()
                        .border_color(colors.border)
                        .child(div().flex_1().min_w_0().text_sm().child(link.clone()))
                        .child(
                            controls::outline_button(
                                "copy-referral-link",
                                if copied { "Copied" } else { "Copy link" },
                                true,
                                colors,
                                Some(Box::new(cx.listener(move |this, _, _, cx| {
                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                        link.clone(),
                                    ));
                                    this.referral_copied = true;
                                    cx.notify();
                                }))),
                            )
                            .gap_2()
                            .child(controls::icon(
                                if copied {
                                    "product/CheckIcon.svg"
                                } else {
                                    "product/CopyIcon.svg"
                                },
                                16.,
                                colors.foreground,
                            )),
                        )
                } else {
                    div()
                        .flex()
                        .flex_col()
                        .items_start()
                        .gap_3()
                        .child(div().text_sm().text_color(muted).child(
                            "New referral invites are available with an active personal Pro trial or paid subscription. Team-only subscriptions aren't eligible.",
                        ))
                        .child(controls::outline_button(
                            "referrals-billing",
                            "View billing",
                            true,
                            colors,
                            Some(Box::new(cx.listener(|this, _, _, cx| {
                                this.select_page(super::navigation::page("billing"), cx)
                            }))),
                        ))
                })
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().font_weight(crate::ui::theme::MEDIUM).child("Your referrals"))
                    .child(
                        div()
                            .id("refresh-referrals")
                            .h(px(28.))
                            .px_2()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_xs()
                            .font_weight(crate::ui::theme::MEDIUM)
                            .cursor_pointer()
                            .hover(|style| style.bg(colors.accent))
                            .child(controls::icon("product/RefreshIcon.svg", 16., colors.foreground))
                            .child("Refresh")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some((user, _)) = this.referrals.take() {
                                    this.load_referrals(user, cx);
                                }
                                cx.notify();
                            })),
                    ),
            )
            .child(if data.invites.is_empty() {
                div().text_sm().text_color(muted).child(
                    "No accepted invites yet. Copying your link doesn't count as an accepted invite.",
                )
            } else {
                div()
                    .border_1()
                    .border_color(colors.border)
                    .children(data.invites.iter().enumerate().map(|(index, invite)| {
                        let status = match invite.status.as_str() {
                            "accepted" => "Accepted · Waiting for trial to start",
                            "applying" => "Trial started · Applying your free month",
                            "applied" => "Reward applied · 1 free month added",
                            "pending" => "Reward pending · Contact us for help",
                            "legacy_pending" => "Previous referral offer · Waiting for first payment",
                            _ => "Previous referral offer · Reward applied",
                        };
                        let date = |value: &str| {
                            chrono::DateTime::parse_from_rfc3339(value)
                                .map(|date| date.format("%-m/%-d/%Y").to_string())
                                .unwrap_or_default()
                        };
                        div()
                            .p_4()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .when(index > 0, |view| view.border_t_1().border_color(colors.border))
                            .child(
                                div()
                                    .flex()
                                    .text_sm()
                                    .font_weight(crate::ui::theme::MEDIUM)
                                    .child(format!("Invite {}", invite.slot))
                                    .child(
                                        div()
                                            .ml_3()
                                            .font_weight(FontWeight::NORMAL)
                                            .text_color(muted)
                                            .child(date(&invite.accepted_at)),
                                    ),
                            )
                            .child(div().text_sm().text_color(muted).child(status))
                            .when(invite.policy == "legacy_payment", |view| {
                                view.child(div().text_xs().text_color(muted).child(format!(
                                    "Original offer: ${:.2} off your subscription after your friend's first payment.",
                                    invite.legacy_amount_cents as f64 / 100.
                                )))
                            })
                            .when_some(invite.extended_until.as_deref().map(date), |view, end| {
                                view.child(div().text_xs().text_color(muted).child(format!(
                                    "Extended through {end} when this reward was applied."
                                )))
                            })
                    }))
            })
            .when(data.invites.iter().any(|invite| invite.policy == "legacy_payment"), |view| {
                view.child(div().text_xs().text_color(muted).child(
                    "Previous referrals keep their original reward terms and count toward your 3-invite limit.",
                ))
            })
    }
}

/// `SyncSettingsPreview`: the Sync page as a free account sees it behind the plan gate.
fn sync_preview(colors: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .flex()
                .items_start()
                .justify_between()
                .gap_4()
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .gap_3()
                        .child(
                            div()
                                .size(px(36.))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(controls::icon(
                                    "product/CloudOffIcon.svg",
                                    16.,
                                    colors.muted_foreground,
                                )),
                        )
                        .child(
                            div().min_w_0().child(heading("Sync paused")).child(
                                div()
                                    .mt_1()
                                    .text_xs()
                                    .line_height(px(20.))
                                    .text_color(colors.muted_foreground)
                                    .child("Changes stay on this device until you resume sync."),
                            ),
                        ),
                )
                .child(
                    controls::switch("sync-preview-switch", false, true, colors, |_, _, _| {})
                        .aria_label("Cloud sync"),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_4()
                .child(
                    div()
                        .text_xs()
                        .line_height(px(16.))
                        .text_color(colors.muted_foreground)
                        .child("Keep notes current automatically."),
                )
                .child(small_outline(
                    "sync-preview-now",
                    Some("product/RefreshIcon.svg"),
                    "Sync now",
                    colors,
                )),
        )
        .child(
            div()
                .child(controls::section_title("Devices").mb_4())
                .child(
                    div()
                        .overflow_hidden()
                        .border_1()
                        .border_color(colors.border.opacity(0.6))
                        .child(
                            div()
                                .px_4()
                                .py_5()
                                .text_center()
                                .text_xs()
                                .line_height(px(16.))
                                .text_color(colors.muted_foreground)
                                .child("No devices registered yet."),
                        ),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::product::account::{Claims, User, Workspace, locked};

    fn account(workspaces: Vec<Workspace>, entitlements: Vec<String>) -> Account {
        Account {
            user: Some(User {
                id: "u".into(),
                email: None,
            }),
            claims: Some(Claims {
                entitlements,
                ..Claims::default()
            }),
            workspaces: Some(workspaces.into()),
            trial_eligible: false,
            devices: None,
        }
    }

    #[test]
    fn teams_unlocks_for_existing_workspaces_and_pro_unlocks_everything() {
        let member = Workspace {
            id: "w".into(),
            name: "Acme".into(),
            tier: WorkspaceTier::Free,
            seat_limit: None,
            used_seats: 1,
        };
        let free = account(vec![], vec![]);
        assert!(locked("team", true, &free) && locked("sync", true, &free));
        assert!(!locked("account", false, &free));

        let in_workspace = account(vec![member], vec![]);
        assert!(!locked("team", true, &in_workspace));
        assert!(locked("sync", true, &in_workspace));

        let pro = account(vec![], vec!["hyprnote_pro".into()]);
        assert!(!locked("team", true, &pro) && !locked("sync", true, &pro));
        assert!(locked("sync", true, &Account::default()));
    }
}
