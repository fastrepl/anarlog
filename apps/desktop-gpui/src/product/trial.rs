//! The personal Pro trial dialogs from `auth/billing.tsx`: trial started, payment reminder
//! (7 and 3 days before the end, without a payment method) and trial ended. Each shows once
//! per account; shipping keeps that in localStorage, the native app in `app_settings`. A channel
//! run also counts the dialogs shipping showed, from its bridge release's mirror.
//! The reminder ends with shipping's referral nudge when the account can still refer friends.

use std::collections::BTreeSet;

use anlg_desktop_identity::webview_storage::{
    Snapshot, TRIAL_ENDED_SEEN, TRIAL_PAYMENT_REMINDER_SEEN, TRIAL_STARTED_SEEN, WebviewStorage,
};
use desktop_runtime::{RuntimeHandle, ServiceError};
use gpui::{
    Context, EventEmitter, FontWeight, Hsla, Render, SharedString, Window, div, prelude::*, px,
};
use serde_json::json;

use crate::ui::theme::{Theme, theme};

use super::{
    account::{self, Account},
    cloud::{CloudServices, Referrals},
};
use crate::ui::a11y::Accessible;

const SEEN_KEY: &str = "gpui_trial_dialogs_seen";
const PRO_TRIAL_DAYS: i64 = 14;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dialog {
    Started { days: i64, has_payment_method: bool },
    Reminder { days: i64 },
    Ended,
}

/// Shipping's `canShowPersonalTrialDialogs`: signed in, account loaded, and every workspace
/// seat on the free tier.
fn personal(account: &Account) -> bool {
    account.user.is_some()
        && account.claims.is_some()
        && account.workspaces.as_ref().is_some_and(|workspaces| {
            workspaces
                .iter()
                .all(|workspace| workspace.tier == account::WorkspaceTier::Free)
        })
}

/// The dialog to show now and the key that marks it seen.
pub fn next(account: &Account, now: i64, seen: &BTreeSet<String>) -> Option<(Dialog, String)> {
    if !personal(account) {
        return None;
    }
    let user = &account.user.as_ref()?.id;
    let billing = account.billing(now);
    let unseen = |key: String| (!seen.contains(&key)).then_some(key);
    if billing.is_trialing {
        if let Some(key) = unseen(format!("trial_started_seen:{user}")) {
            let dialog = Dialog::Started {
                days: billing.trial_days_remaining.unwrap_or(PRO_TRIAL_DAYS),
                has_payment_method: billing.has_payment_method,
            };
            return Some((dialog, key));
        }
        let days = billing.trial_days_remaining?;
        let threshold = if days <= 3 {
            3
        } else if days <= 7 {
            7
        } else {
            return None;
        };
        if billing.has_payment_method {
            return None;
        }
        let end = billing.trial_end.unwrap_or_default() * 1000;
        let key = unseen(format!(
            "trial_payment_reminder_seen:{user}:{end}:{threshold}"
        ))?;
        return Some((Dialog::Reminder { days }, key));
    }
    if billing.trial_end.is_some() && !billing.is_paid {
        let key = unseen(format!("trial_ended_seen:{user}"))?;
        return Some((Dialog::Ended, key));
    }
    None
}

/// The dialogs shipping marked seen (`anarlog:<key>` = `"1"`); the keys here are the same without
/// the `anarlog:` prefix.
fn shipping_seen(mirror: &Snapshot) -> impl Iterator<Item = String> + '_ {
    [
        TRIAL_STARTED_SEEN,
        TRIAL_ENDED_SEEN,
        TRIAL_PAYMENT_REMINDER_SEEN,
    ]
    .into_iter()
    .flat_map(move |prefix| {
        let key = prefix.strip_prefix("anarlog:").unwrap_or(prefix);
        mirror
            .with_prefix(prefix)
            .filter(|(_, value)| *value == "1")
            .map(move |(rest, _)| format!("{key}{rest}"))
    })
}

/// Shipping's `canRefer`: referrals are on, the account is eligible and has friends left.
pub fn can_refer(referrals: &Referrals) -> bool {
    referrals.enabled && referrals.eligible && referrals.remaining > 0
}

/// The reminder's "Invite friends" link; the host opens Settings › Invite friends in a new tab.
pub struct ReferFriend;

pub struct TrialDialogs {
    runtime: RuntimeHandle,
    cloud: Option<CloudServices>,
    seen: Option<BTreeSet<String>>,
    open: Option<Dialog>,
    can_refer: bool,
    _account: gpui::Subscription,
}

impl EventEmitter<ReferFriend> for TrialDialogs {}

impl TrialDialogs {
    /// `shipping`: the bridge release's mirror, in a channel run.
    pub fn new(
        runtime: RuntimeHandle,
        shipping: Option<WebviewStorage>,
        cx: &mut Context<Self>,
    ) -> Self {
        let account = cx.observe_global::<Account>(|this, cx| this.evaluate(cx));
        let this = Self {
            runtime: runtime.clone(),
            cloud: None,
            seen: None,
            open: None,
            can_refer: false,
            _account: account,
        };
        cx.spawn(async move |this, cx| {
            // Before the runtime is ready, reads fail as busy and every dialog would show again; a
            // channel run is ready only after the replaced app released its locks, so its mirror
            // below holds that app's last writes too.
            if !crate::runtime_bridge::ready(&runtime).await {
                return;
            }
            let stored = super::local::data::setting_value(&runtime, SEEN_KEY.into()).await;
            let mut seen = stored
                .ok()
                .flatten()
                .and_then(|value| serde_json::from_value::<BTreeSet<String>>(value).ok())
                .unwrap_or_default();
            // Showing a dialog saves the merged set to `app_settings`.
            if let Some(shipping) = shipping {
                match cx.background_spawn(async move { shipping.read() }).await {
                    Ok(Some(mirror)) => seen.extend(shipping_seen(&mirror)),
                    Ok(None) => {}
                    Err(error) => {
                        tracing::warn!(%error, "shipping's trial dialog state is unreadable")
                    }
                }
            }
            let _ = this.update(cx, |this, cx| {
                this.seen = Some(seen);
                this.evaluate(cx);
            });
        })
        .detach();
        this
    }

    pub fn set_cloud(&mut self, cloud: CloudServices) {
        self.cloud = Some(cloud);
    }

    /// For previews without a cloud account: whether the reminder offers referrals.
    pub fn set_can_refer(&mut self, can_refer: bool, cx: &mut Context<Self>) {
        self.can_refer = can_refer;
        cx.notify();
    }

    fn evaluate(&mut self, cx: &mut Context<Self>) {
        let Some(seen) = &mut self.seen else {
            return;
        };
        if self.open.is_some() {
            return;
        }
        let Some((dialog, key)) = next(&account::current(cx), account::now(), seen) else {
            return;
        };
        seen.insert(key);
        let stored = json!(seen).to_string();
        if matches!(dialog, Dialog::Reminder { .. }) {
            self.load_referrals(cx);
        }
        self.open = Some(dialog);
        let write = self.runtime.submit(move |services| async move {
            services
                .executor
                .execute(
                    "INSERT INTO app_settings (id, value_json, updated_at) VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at".into(),
                    vec![json!(SEEN_KEY), json!(stored)],
                )
                .await
                .map(|_| ())
                .map_err(|error| ServiceError::Failed(error.to_string().into()))
        });
        cx.background_executor()
            .spawn(async move {
                if let Ok(reply) = write
                    && let Err(error) = reply.receive().await
                {
                    tracing::warn!(%error, "trial dialog state was not saved");
                }
            })
            .detach();
        cx.notify();
    }

    /// Shipping's referral summary query; the nudge appears once it says the account can refer.
    fn load_referrals(&mut self, cx: &mut Context<Self>) {
        let Some(cloud) = self.cloud.clone() else {
            return;
        };
        self.can_refer = false;
        cx.spawn(async move |this, cx| {
            let can = cloud
                .referrals()
                .await
                .is_ok_and(|summary| can_refer(&summary));
            let _ = this.update(cx, |this, cx| {
                this.can_refer = can;
                cx.notify();
            });
        })
        .detach();
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        self.open = None;
        cx.notify();
        self.evaluate(cx);
    }

    fn open_web(
        &mut self,
        path: &'static str,
        params: Vec<(&'static str, String)>,
        cx: &mut Context<Self>,
    ) {
        if let Some(cloud) = self.cloud.clone() {
            cx.background_executor()
                .spawn(async move {
                    if let Err(error) = cloud.open_web(path, params, None).await {
                        tracing::warn!(%error, "billing page did not open");
                    }
                })
                .detach();
        }
        self.close(cx);
    }
}

fn button(
    id: &'static str,
    label: &'static str,
    primary: bool,
    colors: &Theme,
) -> gpui::Stateful<gpui::Div> {
    let base = div()
        .id(id)
        .a11y_button(label)
        .h(px(32.))
        .px_4()
        .flex()
        .items_center()
        .justify_center()
        .text_xs()
        .font_weight(crate::ui::theme::MEDIUM)
        .cursor_pointer()
        .child(label);
    if primary {
        let dark = colors.background.l < 0.5;
        let (bg, fg): (Hsla, Hsla) = if dark {
            (gpui::white(), gpui::black())
        } else {
            (colors.primary, colors.primary_foreground)
        };
        base.bg(bg)
            .text_color(fg)
            .shadow_sm()
            .hover(move |style| style.bg(bg.opacity(0.9)))
    } else {
        let hover = colors.background.opacity(0.8);
        base.border_1()
            .border_color(colors.border.opacity(0.7))
            .bg(colors.background.opacity(0.5))
            .shadow_xs()
            .hover(move |style| style.bg(hover))
    }
}

/// `TrialDialogIcon`: the app icon, warm for an active trial and dimmed once it ended.
fn icon(started: bool) -> impl IntoElement {
    div()
        .size(px(56.))
        .overflow_hidden()
        .shadow(vec![gpui::BoxShadow {
            color: if started {
                gpui::rgba(0xb4530938).into()
            } else {
                gpui::rgba(0x0000002e).into()
            },
            offset: gpui::point(px(0.), px(14.)),
            blur_radius: px(22.),
            spread_radius: px(0.),
            inset: false,
        }])
        .child(
            gpui::img("product/AnarlogIcon.png")
                .size_full()
                .when(!started, |image| image.grayscale(true).opacity(0.58)),
        )
}

impl Render for TrialDialogs {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(dialog) = self.open.clone() else {
            return div().into_any_element();
        };
        let colors = theme(window);
        let (started, title, description) = match &dialog {
            Dialog::Started {
                days,
                has_payment_method,
            } => (
                true,
                "Your Pro trial just started".to_owned(),
                if *has_payment_method {
                    format!("Your {days}-day Pro trial starts now. Pro will continue automatically when it ends.")
                } else {
                    format!("Your {days}-day Pro trial starts now. Add a payment method before it ends to keep Pro.")
                },
            ),
            Dialog::Reminder { days } => (
                true,
                if *days == 1 {
                    "Your Pro trial ends in 1 day".to_owned()
                } else {
                    format!("Your Pro trial ends in {days} days")
                },
                "Add a payment method before it ends to keep using Pro without an interruption."
                    .to_owned(),
            ),
            Dialog::Ended => (
                false,
                "Your Pro trial has ended".to_owned(),
                if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
                    "Your notes and recordings are safe. Free local transcription still works. Upgrade anytime to keep Pro features."
                } else {
                    "Your notes and recordings are safe. Upgrade anytime to keep cloud transcription and Pro features, or configure your own transcription provider."
                }
                .to_owned(),
            ),
        };
        let nudge = (matches!(dialog, Dialog::Reminder { .. }) && self.can_refer).then(|| {
            div()
                .pt_4()
                .border_t_1()
                .border_color(colors.border.opacity(0.6))
                .flex()
                .flex_col()
                .items_center()
                .gap_2()
                .text_center()
                .child(
                    div()
                        .text_size(px(13.))
                        .line_height(px(20.))
                        .font_weight(crate::ui::theme::MEDIUM)
                        .child("Need more time? Refer a friend."),
                )
                .child(
                    div()
                        .text_xs()
                        .line_height(px(19.5))
                        .text_color(colors.muted_foreground)
                        .child("When they start their free trial, yours gets one more month. Up to 3 friends."),
                )
                .child(
                    div()
                        .id("trial-refer-friend")
                        .a11y_button("Invite friends")
                        .text_xs()
                        .line_height(px(16.))
                        .font_weight(crate::ui::theme::MEDIUM)
                        .text_color(colors.primary)
                        .cursor_pointer()
                        .hover(|style| style.underline())
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.emit(ReferFriend);
                            this.close(cx);
                        }))
                        .child("Invite friends"),
                )
        });
        let footer = match dialog {
            Dialog::Started { .. } => div().child(
                button("trial-got-it", "Got it", true, &colors)
                    .w_full()
                    .on_click(cx.listener(|this, _, _, cx| this.close(cx))),
            ),
            Dialog::Reminder { .. } => div()
                .grid()
                .grid_cols(2)
                .gap_2()
                .child(
                    button("trial-not-now", "Not now", false, &colors)
                        .on_click(cx.listener(|this, _, _, cx| this.close(cx))),
                )
                .child(
                    button("trial-add-payment", "Add payment method", true, &colors).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.open_web(
                                "/app/portal",
                                vec![("intent", "payment_method_update".into())],
                                cx,
                            )
                        }),
                    ),
                ),
            Dialog::Ended => div()
                .grid()
                .grid_cols(2)
                .gap_2()
                .child(
                    button("trial-later", "Maybe later", false, &colors)
                        .on_click(cx.listener(|this, _, _, cx| this.close(cx))),
                )
                .child(
                    button("trial-upgrade", "Upgrade to Pro", true, &colors).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.open_web(
                                "/app/checkout",
                                vec![("source", "trial_ended".into())],
                                cx,
                            )
                        },
                    )),
                ),
        };
        let size = window.viewport_size();
        gpui::deferred(
            gpui::anchored()
                .position(gpui::point(px(0.), px(0.)))
                .child(
                    div()
                        .id("trial-dialog-backdrop")
                        .w(size.width)
                        .h(size.height)
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(gpui::black().opacity(0.4))
                        .occlude()
                        .on_click(cx.listener(|this, _, _, cx| this.close(cx)))
                        .child(
                            div()
                                .id("trial-dialog")
                                .a11y_dialog(title.clone())
                                .aria_description(description.clone())
                                .w(px(320.))
                                .p_5()
                                .flex()
                                .flex_col()
                                .gap_4()
                                .border_1()
                                .border_color(colors.border.opacity(0.45))
                                .bg(colors.card)
                                .text_color(colors.foreground)
                                .shadow_xl()
                                .on_click(|_, _, cx| cx.stop_propagation())
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .gap_2()
                                        .text_center()
                                        .text_size(px(13.))
                                        .child(icon(started))
                                        .child(
                                            div()
                                                .line_height(px(20.))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(SharedString::from(title)),
                                        )
                                        .child(
                                            div()
                                                .w_full()
                                                .line_height(px(17.7))
                                                .child(SharedString::from(description)),
                                        ),
                                )
                                .child(footer)
                                .children(nudge),
                        ),
                ),
        )
        .with_priority(2)
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::product::account::{Claims, User, Workspace, WorkspaceTier};

    fn account(claims: Claims, tier: Option<WorkspaceTier>) -> Account {
        Account {
            user: Some(User {
                id: "user".into(),
                email: None,
            }),
            claims: Some(claims),
            workspaces: Some(
                tier.map(|tier| Workspace {
                    id: "ws".into(),
                    name: "Acme".into(),
                    tier,
                    seat_limit: None,
                    used_seats: 1,
                })
                .into_iter()
                .collect::<Vec<_>>()
                .into(),
            ),
            trial_eligible: false,
            devices: None,
        }
    }

    #[test]
    fn referral_nudge_needs_an_eligible_account_with_friends_left() {
        let summary = |enabled, eligible, remaining| Referrals {
            enabled,
            eligible,
            code: None,
            accepted: 3 - remaining,
            remaining,
            months_earned: 0,
            invites: Vec::new(),
            link: None,
        };
        assert!(can_refer(&summary(true, true, 1)));
        assert!(!can_refer(&summary(true, true, 0)));
        assert!(!can_refer(&summary(true, false, 3)));
        assert!(!can_refer(&summary(false, true, 3)));
    }

    #[test]
    fn trial_dialogs_show_once_in_shipping_order() {
        let now = 1_000_000;
        let trialing = |days: i64| Claims {
            subscription_status: Some("trialing".into()),
            trial_end: Some(now + days * 86_400 - 60),
            has_payment_method: Some(false),
            ..Claims::default()
        };
        let mut seen = BTreeSet::new();
        let (dialog, key) = next(&account(trialing(14), None), now, &seen).unwrap();
        assert_eq!(
            dialog,
            Dialog::Started {
                days: 14,
                has_payment_method: false
            }
        );
        seen.insert(key);
        assert_eq!(next(&account(trialing(14), None), now, &seen), None);
        let (dialog, key) = next(&account(trialing(5), None), now, &seen).unwrap();
        assert_eq!(dialog, Dialog::Reminder { days: 5 });
        seen.insert(key);
        assert_eq!(next(&account(trialing(5), None), now, &seen), None);
        assert!(matches!(
            next(&account(trialing(2), None), now, &seen),
            Some((Dialog::Reminder { days: 2 }, _))
        ));
        let ended = Claims {
            trial_end: Some(now - 60),
            ..Claims::default()
        };
        assert!(matches!(
            next(&account(ended.clone(), None), now, &seen),
            Some((Dialog::Ended, _))
        ));
        // Team seats are billed by the workspace, so no personal trial dialogs.
        assert_eq!(
            next(&account(ended, Some(WorkspaceTier::Team)), now, &seen),
            None
        );
    }

    #[test]
    fn trial_dialogs_shown_by_the_shipping_app_are_not_shown_again() {
        let now = 1_000_000;
        let trial_end = now + 5 * 86_400;
        let trialing = Claims {
            subscription_status: Some("trialing".into()),
            trial_end: Some(trial_end),
            has_payment_method: Some(false),
            ..Claims::default()
        };
        let mirror = Snapshot {
            id: "mirror".into(),
            entries: [
                ("anarlog:trial_started_seen:user".into(), "1".into()),
                (
                    format!(
                        "anarlog:trial_payment_reminder_seen:user:{}:7",
                        trial_end * 1000
                    ),
                    "1".into(),
                ),
                ("anarlog:trial_ended_seen:other".into(), "1".into()),
            ]
            .into(),
        };
        let seen = shipping_seen(&mirror).collect::<BTreeSet<_>>();

        assert_eq!(next(&account(trialing.clone(), None), now, &seen), None);
        let three_days_left = now + 2 * 86_400;
        assert!(matches!(
            next(&account(trialing, None), three_days_left, &seen),
            Some((Dialog::Reminder { days: 3 }, _))
        ));
    }
}
