//! The signed-in Anarlog account and its plan, as one app-wide value every gated surface
//! reads. Billing mirrors shipping's `deriveBillingInfo` (`packages/supabase/src/billing.ts`);
//! plan tiers mirror the Billing page, which counts a paid workspace seat as Pro.

use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use gpui::{App, Global};
use serde::Deserialize;

/// The billing fields of the Anarlog access token.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Claims {
    #[serde(default)]
    pub entitlements: Vec<String>,
    #[serde(default)]
    pub subscription_status: Option<String>,
    #[serde(default)]
    pub trial_end: Option<i64>,
    #[serde(default)]
    pub has_payment_method: Option<bool>,
    #[serde(default)]
    pub referral_extension: Option<bool>,
    #[serde(default)]
    pub cancel_at_period_end: Option<bool>,
    #[serde(default)]
    pub current_period_end: Option<i64>,
}

impl Claims {
    /// Reads the payload without verifying the signature; the server checks it on every call.
    pub fn decode(token: &str) -> Option<Self> {
        let mut parts = token.split('.');
        let (Some(_), Some(payload), Some(_), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return None;
        };
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).ok()?).ok()
    }

    pub fn billing(&self, now: i64) -> Billing {
        let status = self.subscription_status.as_deref();
        let trial_days_remaining = self.trial_end.map(|end| {
            let remaining = end - now;
            if remaining <= 0 {
                0
            } else {
                (remaining + 86_399) / 86_400
            }
        });
        let active_trial =
            status == Some("trialing") && trial_days_remaining.is_some_and(|d| d > 0);
        let paused = status == Some("paused");
        let entitled = |name: &str| self.entitlements.iter().any(|value| value == name);
        let is_pro = match status {
            Some("trialing") => active_trial,
            Some("paused") => false,
            _ => entitled("hyprnote_pro"),
        };
        let is_lite = entitled("hyprnote_lite");
        let is_trialing = active_trial && self.referral_extension != Some(true);
        Billing {
            is_pro,
            is_lite,
            is_paid: is_pro || is_lite,
            is_trialing,
            is_paused: paused,
            has_payment_method: self.has_payment_method == Some(true),
            trial_end: self.trial_end,
            trial_days_remaining,
            cancel_at_period_end: self.cancel_at_period_end == Some(true),
            current_period_end: self.current_period_end,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Billing {
    pub is_pro: bool,
    pub is_lite: bool,
    pub is_paid: bool,
    pub is_trialing: bool,
    pub is_paused: bool,
    pub has_payment_method: bool,
    pub trial_end: Option<i64>,
    pub trial_days_remaining: Option<i64>,
    pub cancel_at_period_end: bool,
    pub current_period_end: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WorkspaceTier {
    Free,
    Team,
    Enterprise,
}

impl WorkspaceTier {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "free" => Some(Self::Free),
            "team" => Some(Self::Team),
            "enterprise" => Some(Self::Enterprise),
            _ => None,
        }
    }
}

/// A shared workspace the account belongs to and the seats its plan uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Workspace {
    pub id: Arc<str>,
    pub name: Arc<str>,
    pub tier: WorkspaceTier,
    pub seat_limit: Option<u32>,
    pub used_seats: u32,
}

/// The plan shown on the Billing page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    Free,
    Pro,
    Enterprise,
}

/// `getActionForTier` from `@anlg/pricing`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TierAction {
    Current,
    StartTrial,
    Checkout,
}

pub fn tier_action(tier: Tier, current: Tier, can_start_trial: bool) -> Option<TierAction> {
    if tier == current {
        return Some(TierAction::Current);
    }
    match current {
        Tier::Free if tier == Tier::Pro && can_start_trial => Some(TierAction::StartTrial),
        Tier::Free => Some(TierAction::Checkout),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct User {
    pub id: Arc<str>,
    pub email: Option<Arc<str>>,
}

/// Synced devices against the plan's limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Devices {
    pub used: u32,
    pub max: u32,
}

/// The account as the app last saw it. `None` fields are still loading.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Account {
    pub user: Option<User>,
    pub claims: Option<Claims>,
    pub workspaces: Option<Arc<[Workspace]>>,
    /// The server allows a personal trial (`/subscription/can-start-trial`).
    pub trial_eligible: bool,
    pub devices: Option<Devices>,
}

impl Global for Account {}

impl Account {
    pub fn signed_in(&self) -> bool {
        self.user.is_some()
    }

    pub fn billing(&self, now: i64) -> Billing {
        self.claims
            .as_ref()
            .map(|claims| claims.billing(now))
            .unwrap_or_default()
    }

    /// The best paid plan among the account's workspaces.
    pub fn workspace_tier(&self) -> Option<WorkspaceTier> {
        self.workspaces
            .iter()
            .flat_map(|workspaces| workspaces.iter())
            .map(|workspace| workspace.tier)
            .filter(|tier| *tier != WorkspaceTier::Free)
            .max()
    }

    /// Pro features are available through the account's own plan or a paid workspace seat.
    pub fn is_pro(&self, now: i64) -> bool {
        self.signed_in() && (self.billing(now).is_pro || self.workspace_tier().is_some())
    }

    pub fn is_paid(&self, now: i64) -> bool {
        self.signed_in() && (self.billing(now).is_paid || self.workspace_tier().is_some())
    }

    /// The Billing page's current plan; `None` until the workspaces have loaded.
    pub fn tier(&self, now: i64) -> Option<Tier> {
        self.workspaces.as_ref()?;
        Some(match self.workspace_tier() {
            Some(WorkspaceTier::Enterprise) => Tier::Enterprise,
            Some(_) => Tier::Pro,
            None if self.billing(now).is_paid || self.billing(now).is_trialing => Tier::Pro,
            None => Tier::Free,
        })
    }

    /// Personal trials are offered only to unpaid accounts outside paid workspaces.
    pub fn can_start_trial(&self, now: i64) -> bool {
        self.trial_eligible
            && !self.billing(now).is_paid
            && self
                .workspaces
                .as_ref()
                .is_some_and(|workspaces| workspaces.iter().all(|w| w.tier == WorkspaceTier::Free))
    }
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_default()
}

pub fn current(cx: &App) -> Account {
    cx.try_global::<Account>().cloned().unwrap_or_default()
}

pub fn is_pro(cx: &App) -> bool {
    cx.try_global::<Account>()
        .is_some_and(|account| account.is_pro(now()))
}

pub fn is_paid(cx: &App) -> bool {
    cx.try_global::<Account>()
        .is_some_and(|account| account.is_paid(now()))
}

/// Replaces the app-wide account; views observing `Account` re-render.
pub fn set(account: Account, cx: &mut App) {
    if cx.try_global::<Account>() != Some(&account) {
        cx.set_global(account);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000;
    const DAY: i64 = 86_400;

    fn claims(json: &str) -> Claims {
        serde_json::from_str(json).unwrap()
    }

    fn signed_in(claims: Claims, workspaces: Vec<Workspace>) -> Account {
        Account {
            user: Some(User {
                id: "u".into(),
                email: None,
            }),
            claims: Some(claims),
            workspaces: Some(workspaces.into()),
            trial_eligible: true,
            devices: None,
        }
    }

    fn workspace(tier: WorkspaceTier) -> Workspace {
        Workspace {
            id: "w".into(),
            name: "Acme".into(),
            tier,
            seat_limit: Some(5),
            used_seats: 2,
        }
    }

    #[test]
    fn decodes_the_token_payload() {
        let payload = URL_SAFE_NO_PAD.encode(
            r#"{"sub":"u","entitlements":["hyprnote_pro"],"current_period_end":1800000000,"cancel_at_period_end":true}"#,
        );
        let decoded = Claims::decode(&format!("h.{payload}.s")).unwrap();
        assert_eq!(decoded.entitlements, ["hyprnote_pro"]);
        assert_eq!(decoded.current_period_end, Some(NOW));
        assert_eq!(decoded.cancel_at_period_end, Some(true));
        assert_eq!(Claims::decode("not-a-token"), None);
        assert_eq!(Claims::decode(&format!("h.{payload}.s.extra")), None);
    }

    #[test]
    fn billing_matches_shipping_for_each_subscription_state() {
        let active = claims(r#"{"entitlements":["hyprnote_pro"],"subscription_status":"active"}"#)
            .billing(NOW);
        assert!(active.is_pro && active.is_paid && !active.is_trialing);

        let trial = claims(&format!(
            r#"{{"subscription_status":"trialing","trial_end":{}}}"#,
            NOW + 5 * DAY - 60
        ))
        .billing(NOW);
        assert!(trial.is_pro && trial.is_trialing);
        assert_eq!(trial.trial_days_remaining, Some(5));

        let expired = claims(&format!(
            r#"{{"entitlements":["hyprnote_pro"],"subscription_status":"trialing","trial_end":{}}}"#,
            NOW - 1
        ))
        .billing(NOW);
        assert!(!expired.is_pro && !expired.is_trialing);
        assert_eq!(expired.trial_days_remaining, Some(0));

        let paused = claims(r#"{"entitlements":["hyprnote_pro"],"subscription_status":"paused"}"#)
            .billing(NOW);
        assert!(!paused.is_pro && paused.is_paused);

        let referral = claims(&format!(
            r#"{{"subscription_status":"trialing","trial_end":{},"referral_extension":true}}"#,
            NOW + DAY
        ))
        .billing(NOW);
        assert!(referral.is_pro && !referral.is_trialing);

        let lite = claims(r#"{"entitlements":["hyprnote_lite"]}"#).billing(NOW);
        assert!(!lite.is_pro && lite.is_paid);
    }

    #[test]
    fn a_paid_workspace_seat_counts_as_pro() {
        let free = Claims::default();
        assert!(!signed_in(free.clone(), vec![]).is_pro(NOW));
        assert!(!signed_in(free.clone(), vec![workspace(WorkspaceTier::Free)]).is_pro(NOW));

        let team = signed_in(free.clone(), vec![workspace(WorkspaceTier::Team)]);
        assert!(team.is_pro(NOW) && team.is_paid(NOW));
        assert_eq!(team.tier(NOW), Some(Tier::Pro));
        assert!(!team.can_start_trial(NOW));

        let enterprise = signed_in(
            free,
            vec![
                workspace(WorkspaceTier::Team),
                workspace(WorkspaceTier::Enterprise),
            ],
        );
        assert_eq!(enterprise.tier(NOW), Some(Tier::Enterprise));
    }

    #[test]
    fn signed_out_or_loading_accounts_are_never_pro() {
        let mut account = signed_in(claims(r#"{"entitlements":["hyprnote_pro"]}"#), vec![]);
        account.user = None;
        assert!(!account.is_pro(NOW));
        assert!(!Account::default().is_pro(NOW));

        let loading = Account {
            workspaces: None,
            ..signed_in(Claims::default(), vec![])
        };
        assert_eq!(loading.tier(NOW), None);
        assert!(!loading.can_start_trial(NOW));
    }

    #[test]
    fn trial_offer_requires_an_unpaid_personal_account() {
        assert!(signed_in(Claims::default(), vec![]).can_start_trial(NOW));
        let pro = signed_in(claims(r#"{"entitlements":["hyprnote_pro"]}"#), vec![]);
        assert!(!pro.can_start_trial(NOW));
        let ineligible = Account {
            trial_eligible: false,
            ..signed_in(Claims::default(), vec![])
        };
        assert!(!ineligible.can_start_trial(NOW));
    }

    #[test]
    fn tier_actions_follow_pricing() {
        assert_eq!(
            tier_action(Tier::Pro, Tier::Free, true),
            Some(TierAction::StartTrial)
        );
        assert_eq!(
            tier_action(Tier::Pro, Tier::Free, false),
            Some(TierAction::Checkout)
        );
        assert_eq!(tier_action(Tier::Free, Tier::Pro, true), None);
        assert_eq!(
            tier_action(Tier::Pro, Tier::Pro, false),
            Some(TierAction::Current)
        );
    }
}
