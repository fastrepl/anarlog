#![cfg(target_os = "macos")]

use hypr_activity_capture_interface::{CaptureAccess, CaptureError, CapturePolicy};
use objc2_application_services::AXUIElement;
use url::Host;
use url::Url;

use crate::{app_profile::AppProfile, ax, browser_url::BrowserUrlResolver};

pub(crate) struct CaptureContext<'a> {
    pub policy: &'a CapturePolicy,
    pub bundle_id: Option<&'a str>,
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
    pub access: CaptureAccess,
    pub url: Option<String>,
    pub skip: bool,
    pub text_mode: CaptureTextMode,
}

pub(crate) fn resolve_capture_plan(
    context: &CaptureContext<'_>,
) -> Result<CapturePlan, CaptureError> {
    let app_access = context.policy.access_for_bundle(context.bundle_id);
    if !app_access.allows_snapshot() {
        return Ok(CapturePlan {
            access: app_access,
            url: None,
            skip: true,
            text_mode: CaptureTextMode::Generic,
        });
    }

    if let Some(mut plan) = app_specific_capture_plan(context)? {
        plan.access = std::cmp::min(plan.access, app_access);
        plan.skip |= !plan.access.allows_snapshot();
        return Ok(plan);
    }

    Ok(CapturePlan {
        access: normalize_generic_access(app_access),
        url: None,
        skip: false,
        text_mode: CaptureTextMode::Generic,
    })
}

fn app_specific_capture_plan(
    context: &CaptureContext<'_>,
) -> Result<Option<CapturePlan>, CaptureError> {
    Ok(match context.app_profile {
        AppProfile::Slack => Some(app_text_capture_plan(context)),
        AppProfile::Spotify => Some(app_text_capture_plan(context)),
        profile if BrowserUrlResolver.supports_profile(profile) => {
            Some(browser_capture_plan(context)?)
        }
        _ => None,
    })
}

fn app_text_capture_plan(context: &CaptureContext<'_>) -> CapturePlan {
    CapturePlan {
        access: normalize_generic_access(context.policy.access_for_bundle(context.bundle_id)),
        url: None,
        skip: false,
        text_mode: capture_text_mode_for_profile(context.app_profile),
    }
}

fn browser_capture_plan(context: &CaptureContext<'_>) -> Result<CapturePlan, CaptureError> {
    if context.policy.browser.block_private_browsing
        && BrowserUrlResolver.front_window_is_private(context.app_profile) == Some(true)
    {
        return Ok(private_browser_capture_plan());
    }

    let raw_url = BrowserUrlResolver
        .current_url(context.app_profile)
        .or(ax::string_attribute(context.focused_window, "AXURL")?);
    let (sanitized_url, host) = raw_url
        .as_deref()
        .and_then(|value| sanitize_browser_url(&context.policy.browser, value))
        .map(|(url, host)| (Some(url), host))
        .unwrap_or((None, None));

    let access = resolve_browser_access(
        &context.policy.browser,
        host.as_deref(),
        sanitized_url.is_some(),
        context.policy.mode,
    );

    Ok(CapturePlan {
        access,
        url: access.allows_url().then_some(sanitized_url).flatten(),
        skip: !access.allows_snapshot(),
        text_mode: CaptureTextMode::Generic,
    })
}

fn capture_text_mode_for_profile(profile: AppProfile) -> CaptureTextMode {
    match profile {
        AppProfile::Slack => CaptureTextMode::Slack,
        AppProfile::Spotify => CaptureTextMode::Spotify,
        _ => CaptureTextMode::Generic,
    }
}

fn private_browser_capture_plan() -> CapturePlan {
    CapturePlan {
        access: CaptureAccess::None,
        url: None,
        skip: true,
        text_mode: CaptureTextMode::Generic,
    }
}

fn resolve_browser_access(
    policy: &hypr_activity_capture_interface::BrowserPolicy,
    host: Option<&str>,
    has_sanitized_url: bool,
    mode: hypr_activity_capture_interface::PolicyMode,
) -> CaptureAccess {
    let mut access = policy.access_for_host(host, mode);
    if !has_sanitized_url && access > CaptureAccess::Metadata {
        access = CaptureAccess::Metadata;
    }
    if !has_sanitized_url && access.allows_text() && policy.require_url_for_text_access {
        access = CaptureAccess::Metadata;
    }
    access
}

fn sanitize_browser_url(
    policy: &hypr_activity_capture_interface::BrowserPolicy,
    raw_url: &str,
) -> Option<(String, Option<String>)> {
    let mut parsed = Url::parse(raw_url).ok()?;
    let host = parsed
        .host()
        .and_then(normalize_host)
        .map(|value| value.trim_matches('.').to_ascii_lowercase())
        .filter(|value| !value.is_empty());

    if policy.strip_query {
        parsed.set_query(None);
    }
    if policy.strip_fragment {
        parsed.set_fragment(None);
    }

    Some((parsed.to_string(), host))
}

fn normalize_generic_access(access: CaptureAccess) -> CaptureAccess {
    match access {
        CaptureAccess::Url => CaptureAccess::Metadata,
        _ => access,
    }
}

fn normalize_host(host: Host<&str>) -> Option<&str> {
    match host {
        Host::Domain(value) => Some(value),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use hypr_activity_capture_interface::{BrowserPolicy, CaptureAccess, DomainRule, PolicyMode};

    use super::{
        CaptureTextMode, capture_text_mode_for_profile, normalize_generic_access,
        private_browser_capture_plan, resolve_browser_access, sanitize_browser_url,
    };
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

    #[test]
    fn browser_url_sanitization_strips_query_and_fragment() {
        let policy = BrowserPolicy::default();
        let (url, host) =
            sanitize_browser_url(&policy, "https://example.com/path?q=1#section").unwrap();

        assert_eq!(url, "https://example.com/path");
        assert_eq!(host.as_deref(), Some("example.com"));
    }

    #[test]
    fn browser_access_downgrades_without_url() {
        let policy = BrowserPolicy {
            rules: vec![DomainRule {
                domain: "example.com".to_string(),
                include_subdomains: true,
                access: CaptureAccess::Full,
            }],
            ..BrowserPolicy::default()
        };

        assert_eq!(
            resolve_browser_access(&policy, Some("example.com"), false, PolicyMode::OptIn),
            CaptureAccess::Metadata
        );
    }

    #[test]
    fn browser_access_allows_url_backed_full_capture() {
        let policy = BrowserPolicy {
            rules: vec![DomainRule {
                domain: "example.com".to_string(),
                include_subdomains: true,
                access: CaptureAccess::Full,
            }],
            ..BrowserPolicy::default()
        };

        assert_eq!(
            resolve_browser_access(&policy, Some("example.com"), true, PolicyMode::OptIn),
            CaptureAccess::Full
        );
    }

    #[test]
    fn private_browser_plan_skips_capture() {
        let plan = private_browser_capture_plan();

        assert_eq!(plan.access, CaptureAccess::None);
        assert!(plan.skip);
        assert_eq!(plan.text_mode, CaptureTextMode::Generic);
    }

    #[test]
    fn generic_access_normalizes_url_to_metadata() {
        assert_eq!(
            normalize_generic_access(CaptureAccess::Url),
            CaptureAccess::Metadata
        );
        assert_eq!(
            normalize_generic_access(CaptureAccess::Full),
            CaptureAccess::Full
        );
    }
}
