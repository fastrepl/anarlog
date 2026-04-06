#![cfg(target_os = "macos")]

use hypr_activity_capture_interface::{
    ActivityKind, AppIdentity, BrowserContext, CaptureCandidate, CaptureError, SnapshotSource,
};
use objc2_application_services::AXUIElement;

use crate::{app_profile::AppProfile, ax, browser_url::BrowserUrlResolver};

pub(crate) struct CaptureContext<'a> {
    pub app: &'a AppIdentity,
    pub app_profile: AppProfile,
    pub focused_window: &'a AXUIElement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaptureTextMode {
    Generic,
    Slack,
    Spotify,
}

#[derive(Debug, Clone)]
pub(crate) struct CapturePlan {
    pub candidate: CaptureCandidate,
    pub text_mode: CaptureTextMode,
}

pub(crate) fn resolve_capture_plan(
    context: &CaptureContext<'_>,
) -> Result<CapturePlan, CaptureError> {
    Ok(match context.app_profile {
        AppProfile::Slack | AppProfile::Spotify => CapturePlan {
            candidate: foreground_candidate(context),
            text_mode: capture_text_mode_for_profile(context.app_profile),
        },
        profile if BrowserUrlResolver.supports_profile(profile) => browser_capture_plan(context)?,
        _ => CapturePlan {
            candidate: foreground_candidate(context),
            text_mode: CaptureTextMode::Generic,
        },
    })
}

fn browser_capture_plan(context: &CaptureContext<'_>) -> Result<CapturePlan, CaptureError> {
    let raw_url = BrowserUrlResolver
        .current_url(context.app_profile)
        .or(ax::string_attribute(context.focused_window, "AXURL")?);

    Ok(CapturePlan {
        candidate: CaptureCandidate {
            app: context.app.clone(),
            activity_kind: ActivityKind::Browser,
            source: SnapshotSource::Accessibility,
            browser: Some(BrowserContext {
                raw_url,
                is_private: BrowserUrlResolver.front_window_is_private(context.app_profile)
                    == Some(true),
            }),
        },
        text_mode: CaptureTextMode::Generic,
    })
}

fn foreground_candidate(context: &CaptureContext<'_>) -> CaptureCandidate {
    CaptureCandidate {
        app: context.app.clone(),
        activity_kind: ActivityKind::ForegroundWindow,
        source: SnapshotSource::Accessibility,
        browser: None,
    }
}

fn capture_text_mode_for_profile(profile: AppProfile) -> CaptureTextMode {
    match profile {
        AppProfile::Slack => CaptureTextMode::Slack,
        AppProfile::Spotify => CaptureTextMode::Spotify,
        _ => CaptureTextMode::Generic,
    }
}

#[cfg(test)]
mod tests {
    use super::{CaptureTextMode, capture_text_mode_for_profile};
    use crate::app_profile::AppProfile;

    #[test]
    fn special_profiles_map_to_expected_text_modes() {
        assert_eq!(
            capture_text_mode_for_profile(AppProfile::Slack),
            CaptureTextMode::Slack
        );
        assert_eq!(
            capture_text_mode_for_profile(AppProfile::Spotify),
            CaptureTextMode::Spotify
        );
        assert_eq!(
            capture_text_mode_for_profile(AppProfile::Chrome),
            CaptureTextMode::Generic
        );
    }
}
