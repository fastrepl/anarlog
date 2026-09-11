//! The loopback callback page: the browser lands here after auth, billing,
//! integration, and onboarding-demo flows, and bounces back into the app.

use std::str::FromStr;

use askama::Template;

use crate::types::{AuthCallbackSearch, DeepLink};

#[derive(Template)]
#[template(path = "callback.html")]
struct CallbackTemplate {
    deeplink_url: String,
    is_success: bool,
    title: String,
    description: String,
}

pub fn render_html(deep_link: &DeepLink, scheme: &str) -> String {
    let (is_success, title, description) = ui_content(deep_link);
    render_template_html(scheme, is_success, title, description, Some(deep_link))
}

pub fn render_html_from_callback(path: &str, query: &str, scheme: &str) -> String {
    let parse_result = parse_callback(path, query);
    render_html_from_parse_result(parse_result.as_ref(), scheme)
}

pub fn parse_callback(path: &str, query: &str) -> Result<DeepLink, crate::Error> {
    let path = path.trim_start_matches('/');
    let pseudo_url = if query.is_empty() {
        format!("local://{path}")
    } else {
        format!("local://{path}?{query}")
    };

    DeepLink::from_str(&pseudo_url)
}

/// Scheme, host, and path only: deep-link queries carry tokens.
pub fn redact_url(url_str: &str) -> String {
    match url::Url::parse(url_str) {
        Ok(parsed) => {
            let scheme = parsed.scheme();
            let host = parsed.host_str().unwrap_or("");
            let path = parsed.path();
            format!("{scheme}://{host}{path}")
        }
        Err(_) => "[invalid_url]".to_string(),
    }
}

fn render_html_from_parse_result<E>(parse_result: Result<&DeepLink, &E>, scheme: &str) -> String {
    let deep_link = parse_result.ok();
    let (is_success, title, description) =
        deep_link.map(ui_content).unwrap_or_else(default_ui_content);
    render_template_html(scheme, is_success, title, description, deep_link)
}

fn render_template_html(
    scheme: &str,
    is_success: bool,
    title: &str,
    description: &str,
    deep_link: Option<&DeepLink>,
) -> String {
    CallbackTemplate {
        deeplink_url: return_to_app_url(scheme, deep_link),
        is_success,
        title: title.to_string(),
        description: description.to_string(),
    }
    .render()
    .unwrap_or_default()
}

// Subscription codes bounce through `{scheme}://auth/callback?code=…` so the
// OS opens the app. Token logins stay focus-only to avoid a second auth
// callback with the same secrets.
fn return_to_app_url(scheme: &str, deep_link: Option<&DeepLink>) -> String {
    if let Some(DeepLink::AuthCallback(search)) = deep_link
        && let Some(url) = subscription_auth_deeplink(scheme, search)
    {
        return url;
    }

    format!("{scheme}://focus")
}

pub fn subscription_auth_deeplink(scheme: &str, search: &AuthCallbackSearch) -> Option<String> {
    let code = search.code.as_deref()?.trim();
    if code.is_empty() || !search.access_token.is_empty() || !search.refresh_token.is_empty() {
        return None;
    }

    let mut url = url::Url::parse(&format!("{scheme}://auth/callback")).ok()?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("code", code);
        if let Some(state) = search
            .state
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            pairs.append_pair("state", state);
        }
    }
    Some(url.into())
}

fn default_ui_content() -> (bool, &'static str, &'static str) {
    (
        false,
        "Something went wrong",
        "Please close this window and try again.",
    )
}

fn ui_content(deep_link: &DeepLink) -> (bool, &'static str, &'static str) {
    match deep_link {
        DeepLink::AuthCallback(search)
            if search
                .code
                .as_deref()
                .is_some_and(|code| !code.trim().is_empty())
                && search.access_token.is_empty()
                && search.refresh_token.is_empty() =>
        {
            (
                true,
                "Connected successfully",
                "Returning to Anarlog to finish connecting.",
            )
        }
        DeepLink::AuthCallback(_) => (
            true,
            "Signed in successfully",
            "Click the button below to return to the app.",
        ),
        DeepLink::BillingRefresh(_) => (
            true,
            "Subscription updated",
            "Click the button below to return to the app.",
        ),
        DeepLink::IntegrationCallback(s) if s.status == "success" => (
            true,
            "Connected successfully",
            "Click the button below to return to the app.",
        ),
        DeepLink::IntegrationCallback(s) if s.status == "upgrade_required" => (
            false,
            "Upgrade required",
            "You can close this window and upgrade your plan to connect this integration.",
        ),
        DeepLink::IntegrationCallback(_) => (
            false,
            "Connection failed",
            "Something went wrong. Please close this window and try again.",
        ),
        DeepLink::OnboardingDemoComplete(_) => (
            true,
            "Demo complete",
            "Anarlog is finishing your transcript and creating your summary.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subscription_search() -> AuthCallbackSearch {
        AuthCallbackSearch {
            code: Some("ac_nf5hq".to_string()),
            state: Some("state-1".to_string()),
            ..AuthCallbackSearch::default()
        }
    }

    #[test]
    fn subscription_code_bounces_through_custom_scheme_deeplink() {
        assert_eq!(
            subscription_auth_deeplink("anarlog", &subscription_search()).as_deref(),
            Some("anarlog://auth/callback?code=ac_nf5hq&state=state-1")
        );
        let html = render_html(&DeepLink::AuthCallback(subscription_search()), "anarlog");
        assert!(html.contains("anarlog://auth/callback?code=ac_nf5hq"));
        assert!(html.contains("state=state-1"));
        assert!(html.contains(r#"id="open-app""#));
        assert!(html.contains(r#"document.getElementById("open-app")?.click()"#));
        assert!(html.contains("Connected successfully"));
        assert!(!html.contains("anarlog://focus"));
    }

    #[test]
    fn token_login_stays_focus_only() {
        let html = render_html(
            &DeepLink::AuthCallback(AuthCallbackSearch {
                access_token: "access".to_string(),
                refresh_token: "refresh".to_string(),
                code: Some("should-ignore".to_string()),
                ..AuthCallbackSearch::default()
            }),
            "anarlog-dev",
        );
        assert!(html.contains("anarlog-dev://focus"));
        assert!(!html.contains("code=should-ignore"));
        assert!(html.contains("Signed in successfully"));
    }

    #[test]
    fn loopback_query_renders_subscription_deeplink() {
        let html = render_html_from_callback(
            "/auth/callback",
            "code=codex-code&state=s1&scope=openid",
            "anarlog",
        );
        assert!(html.contains("anarlog://auth/callback?code=codex-code"));
        assert!(html.contains("state=s1"));
    }

    #[test]
    fn demo_completion_renders_the_focus_bounce() {
        let html = render_html_from_callback("onboarding-demo/complete", "", "anarlog-dev");
        assert!(html.contains("Demo complete"));
        assert!(html.contains("anarlog-dev://focus"));
    }

    #[test]
    fn redacts_query_and_fragment_from_logged_urls() {
        let value = redact_url(
            "anarlog://share/open?mode=handoff&request_id=ba5ca57a-8f88-44e8-ab92-f9e10c89425c#secret",
        );
        assert_eq!(value, "anarlog://share/open");
        assert_eq!(redact_url("not a url"), "[invalid_url]");
    }
}
