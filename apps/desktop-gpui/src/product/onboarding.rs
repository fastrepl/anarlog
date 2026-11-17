//! First-run onboarding, ported from `apps/desktop/src/onboarding`: the step order, which
//! sections show, and the decisions each step makes. `view` renders it and `bgm` plays its
//! background music.

mod bgm;
mod view;

pub use view::{OnboardingPreview, OnboardingView, State};

/// `packages/pricing/src/trial-policy.json`.
pub const WORK_EMAIL_TRIAL_DAYS: u32 = 30;

/// `app_settings` key; a fresh profile has no row and needs onboarding (shipping's
/// `OnboardingNeeded2` store key defaults to true).
pub const NEEDED_KEY: &str = "onboarding_needed";

pub fn needed(stored: Option<&serde_json::Value>) -> bool {
    stored.and_then(serde_json::Value::as_bool).unwrap_or(true)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Permissions,
    Login,
    Calendar,
    Imports,
    Final,
}

/// `config.tsx`: only macOS asks for permissions up front.
pub fn steps(macos: bool) -> &'static [Step] {
    const MACOS: &[Step] = &[
        Step::Permissions,
        Step::Login,
        Step::Calendar,
        Step::Imports,
        Step::Final,
    ];
    if macos { MACOS } else { &MACOS[1..] }
}

pub fn next(steps: &[Step], step: Step) -> Option<Step> {
    let index = steps.iter().position(|candidate| *candidate == step)?;
    steps.get(index + 1).copied()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Completed,
    Active,
    Upcoming,
}

/// `getStepStatus`; `None` for a step this platform does not have.
pub fn section(steps: &[Step], step: Step, current: Step) -> Option<Section> {
    let index = steps.iter().position(|candidate| *candidate == step)?;
    let current = steps.iter().position(|candidate| *candidate == current)?;
    Some(match index.cmp(&current) {
        std::cmp::Ordering::Less => Section::Completed,
        std::cmp::Ordering::Equal => Section::Active,
        std::cmp::Ordering::Greater => Section::Upcoming,
    })
}

/// `PermissionStatus` from the permissions plugin.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Access {
    #[default]
    NeverRequested,
    Denied,
    Authorized,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Permission {
    Microphone,
    SystemAudio,
    Accessibility,
    Calendar,
}

impl Permission {
    /// Row and target ids of the product permission service.
    pub fn id(self) -> &'static str {
        match self {
            Self::Microphone => "microphone",
            Self::SystemAudio => "system_audio",
            Self::Accessibility => "accessibility",
            Self::Calendar => "calendar",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionAction {
    Request,
    OpenSettings,
}

/// `handleAction` in `permissions.tsx`. On macOS a denied microphone or system-audio grant can
/// only be changed in System Settings; elsewhere the runtime retries the request.
pub fn permission_action(permission: Permission, access: Access, macos: bool) -> PermissionAction {
    let opens_settings_when_denied = macos && permission != Permission::Accessibility;
    if opens_settings_when_denied && access == Access::Denied {
        PermissionAction::OpenSettings
    } else {
        PermissionAction::Request
    }
}

/// The permissions step continues by itself once every listed grant is authorized.
pub fn permissions_complete(
    microphone: Access,
    system_audio: Access,
    accessibility: Access,
) -> bool {
    [microphone, system_audio, accessibility]
        .iter()
        .all(|access| *access == Access::Authorized)
}

/// The completed account section's title.
pub fn login_completed_title(signed_in: bool, skipped: bool) -> &'static str {
    if signed_in {
        "Signed in"
    } else if skipped {
        "Skipped"
    } else {
        "Account"
    }
}

/// `TrialPhase` from `account/trial.tsx`, without the personal-email switch offer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrialPhase {
    Checking,
    Starting,
    AlreadyPaid,
    AlreadyTrialing,
    Started { days: Option<u32> },
    NotEligible,
    Failed,
}

impl TrialPhase {
    /// How long the result stays on screen before the step continues; `None` while working.
    pub fn continue_after_ms(self) -> Option<u64> {
        match self {
            Self::Checking | Self::Starting => None,
            Self::Started { .. } => Some(2500),
            Self::AlreadyPaid | Self::AlreadyTrialing | Self::Failed => Some(1500),
            Self::NotEligible => Some(0),
        }
    }
}

/// Entitlement claims of the signed-in account.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Billing {
    pub paid: bool,
    pub trialing: bool,
}

/// What the account step does once it knows the account's billing state.
pub fn trial_decision(billing: Billing, eligible: bool) -> TrialPhase {
    if billing.paid && !billing.trialing {
        TrialPhase::AlreadyPaid
    } else if billing.trialing {
        TrialPhase::AlreadyTrialing
    } else if eligible {
        TrialPhase::Starting
    } else {
        TrialPhase::NotEligible
    }
}

/// A calendar provider button's click (`handleConnect` in `calendar.tsx`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectAction {
    /// Signed out: go back to the account step and start sign-in.
    SignIn,
    Connect,
    /// A connect flow is already opening.
    Ignore,
}

pub fn connect_action(signed_in: bool, opening: bool) -> ConnectAction {
    if !signed_in {
        ConnectAction::SignIn
    } else if opening {
        ConnectAction::Ignore
    } else {
        ConnectAction::Connect
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_profiles_need_onboarding_until_it_is_finished() {
        assert!(needed(None));
        assert!(needed(Some(&serde_json::json!(true))));
        assert!(!needed(Some(&serde_json::json!(false))));
    }

    #[test]
    fn only_macos_asks_for_permissions_and_later_steps_stay_hidden() {
        assert_eq!(steps(true)[0], Step::Permissions);
        assert_eq!(steps(false)[0], Step::Login);
        assert_eq!(section(steps(false), Step::Permissions, Step::Login), None);
        assert_eq!(next(steps(true), Step::Final), None);
        let steps = steps(true);
        assert_eq!(
            section(steps, Step::Permissions, Step::Calendar),
            Some(Section::Completed)
        );
        assert_eq!(
            section(steps, Step::Final, Step::Calendar),
            Some(Section::Upcoming)
        );
    }

    #[test]
    fn denied_macos_audio_opens_settings_but_first_requests_prompt() {
        assert_eq!(
            permission_action(Permission::Microphone, Access::NeverRequested, true),
            PermissionAction::Request
        );
        assert_eq!(
            permission_action(Permission::SystemAudio, Access::Denied, true),
            PermissionAction::OpenSettings
        );
        assert_eq!(
            permission_action(Permission::Accessibility, Access::Denied, true),
            PermissionAction::Request
        );
        assert_eq!(
            permission_action(Permission::Microphone, Access::Denied, false),
            PermissionAction::Request
        );
    }

    #[test]
    fn permissions_continue_only_when_every_grant_is_authorized() {
        use Access::*;
        assert!(permissions_complete(Authorized, Authorized, Authorized));
        assert!(!permissions_complete(Authorized, Denied, Authorized));
        assert!(!permissions_complete(
            Authorized,
            Authorized,
            NeverRequested
        ));
    }

    #[test]
    fn trial_starts_only_for_eligible_unpaid_accounts() {
        let free = Billing {
            paid: false,
            trialing: false,
        };
        assert_eq!(trial_decision(free, true), TrialPhase::Starting);
        assert_eq!(trial_decision(free, false), TrialPhase::NotEligible);
        assert_eq!(
            trial_decision(
                Billing {
                    paid: true,
                    trialing: false
                },
                true
            ),
            TrialPhase::AlreadyPaid
        );
        assert_eq!(
            trial_decision(
                Billing {
                    paid: true,
                    trialing: true
                },
                true
            ),
            TrialPhase::AlreadyTrialing
        );
        assert_eq!(TrialPhase::Starting.continue_after_ms(), None);
    }

    #[test]
    fn signed_out_calendar_connect_goes_to_sign_in_and_open_flows_are_not_repeated() {
        assert_eq!(connect_action(false, true), ConnectAction::SignIn);
        assert_eq!(connect_action(true, true), ConnectAction::Ignore);
        assert_eq!(connect_action(true, false), ConnectAction::Connect);
    }
}
