//! The personal Pro trial dialogs from `auth/billing.tsx`: trial started, payment reminder
//! (7 and 3 days before the end, without a payment method) and trial ended. Each shows once
//! per account; shipping keeps that in localStorage, the native app in `app_settings`.

use std::collections::BTreeSet;

use desktop_runtime::{RuntimeHandle, ServiceError};
use gpui::{Context, FontWeight, Hsla, Render, SharedString, Window, div, prelude::*, px};
use serde_json::json;

use crate::ui::theme::{Theme, theme};

use super::{
    account::{self, Account},
    cloud::CloudServices,
};

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

pub struct TrialDialogs {
    runtime: RuntimeHandle,
    cloud: Option<CloudServices>,
    seen: Option<BTreeSet<String>>,
    open: Option<Dialog>,
    _account: gpui::Subscription,
}

impl TrialDialogs {
    pub fn new(runtime: RuntimeHandle, cx: &mut Context<Self>) -> Self {
        let account = cx.observe_global::<Account>(|this, cx| this.evaluate(cx));
        let this = Self {
            runtime: runtime.clone(),
            cloud: None,
            seen: None,
            open: None,
            _account: account,
        };
        cx.spawn(async move |this, cx| {
            let stored = super::local::data::setting_value(&runtime, SEEN_KEY.into()).await;
            let seen = stored
                .ok()
                .flatten()
                .and_then(|value| serde_json::from_value::<BTreeSet<String>>(value).ok())
                .unwrap_or_default();
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
        .h(px(32.))
        .px_4()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
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
        .rounded(px(12.))
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
                                .w(px(320.))
                                .p_5()
                                .flex()
                                .flex_col()
                                .gap_4()
                                .rounded(px(26.))
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
                                .child(footer),
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
}
