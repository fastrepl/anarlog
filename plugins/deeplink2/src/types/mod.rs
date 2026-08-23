mod auth_callback;
mod billing_refresh;
mod integration_callback;
mod onboarding_demo_complete;
mod share_open;

pub use auth_callback::*;
pub use billing_refresh::*;
pub use integration_callback::*;
pub use onboarding_demo_complete::*;
pub use share_open::*;

use serde::{Deserialize, Serialize};
use specta::Type;
use std::str::FromStr;

const SHARE_OPEN_PREFIXES: [&str; 6] = [
    "anarlog://share/open",
    "anarlog-staging://share/open",
    "anarlog-dev://share/open",
    "hyprnote://share/open",
    "hyprnote-staging://share/open",
    "hypr://share/open",
];
const MAX_SHARE_OPEN_URL_BYTES: usize = 512;

#[derive(Debug, Clone, serde::Serialize, specta::Type, tauri_specta::Event)]
pub struct DeepLinkEvent(pub DeepLink);

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "to", content = "search")]
pub enum DeepLink {
    #[serde(rename = "/auth/callback")]
    AuthCallback(AuthCallbackSearch),
    #[serde(rename = "/billing/refresh")]
    BillingRefresh(BillingRefreshSearch),
    #[serde(rename = "/integration/callback")]
    IntegrationCallback(IntegrationCallbackSearch),
    #[serde(rename = "/onboarding-demo/complete")]
    OnboardingDemoComplete(OnboardingDemoCompleteSearch),
}

pub(crate) enum IncomingDeepLink {
    Existing(DeepLink),
    ShareOpen(ShareOpenRequest),
}

impl FromStr for IncomingDeepLink {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let candidate = s.trim_matches(|character: char| character.is_ascii_whitespace());
        if candidate.len() > MAX_SHARE_OPEN_URL_BYTES
            && SHARE_OPEN_PREFIXES.iter().any(|expected| {
                candidate
                    .get(..expected.len())
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case(expected))
            })
        {
            return Err(crate::Error::InvalidShareOpen);
        }

        let parsed = url::Url::parse(candidate)?;
        let host = parsed.host_str().unwrap_or("");
        let path = parsed.path().trim_start_matches('/');
        let full_path = if path.is_empty() {
            host.to_string()
        } else {
            format!("{host}/{path}")
        };

        if full_path == "share/open" {
            return ShareOpenRequest::parse(&parsed).map(Self::ShareOpen);
        }

        DeepLink::from_str(candidate).map(Self::Existing)
    }
}

#[cfg(test)]
mod incoming_tests {
    use super::*;

    #[test]
    fn rejects_oversized_share_open_before_url_parsing() {
        for prefix in SHARE_OPEN_PREFIXES {
            let value = format!("{prefix}?{}", "unknown=x&".repeat(64));
            assert!(value.len() > MAX_SHARE_OPEN_URL_BYTES);
            assert!(matches!(
                IncomingDeepLink::from_str(&value),
                Err(crate::Error::InvalidShareOpen)
            ));
            assert!(matches!(
                IncomingDeepLink::from_str(&format!(" \n{value}")),
                Err(crate::Error::InvalidShareOpen)
            ));
        }
    }
}

impl DeepLink {
    pub fn path(&self) -> &'static str {
        match self {
            DeepLink::AuthCallback(_) => "/auth/callback",
            DeepLink::BillingRefresh(_) => "/billing/refresh",
            DeepLink::IntegrationCallback(_) => "/integration/callback",
            DeepLink::OnboardingDemoComplete(_) => "/onboarding-demo/complete",
        }
    }
}

impl FromStr for DeepLink {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parsed = url::Url::parse(s)?;

        let host = parsed.host_str().unwrap_or("");
        let path = parsed.path().trim_start_matches('/');
        let full_path = if path.is_empty() {
            host.to_string()
        } else {
            format!("{}/{}", host, path)
        };

        let query = parsed.query().unwrap_or("");

        match full_path.as_str() {
            "auth/callback" => Ok(DeepLink::AuthCallback(serde_qs::from_str(query)?)),
            "billing/refresh" => Ok(DeepLink::BillingRefresh(serde_qs::from_str(query)?)),
            "integration/callback" => Ok(DeepLink::IntegrationCallback(serde_qs::from_str(query)?)),
            "onboarding-demo/complete" => {
                Ok(DeepLink::OnboardingDemoComplete(serde_qs::from_str(query)?))
            }
            _ => Err(crate::Error::UnknownPath(full_path)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_onboarding_demo_completion() {
        assert!(matches!(
            DeepLink::from_str("anarlog://onboarding-demo/complete").unwrap(),
            DeepLink::OnboardingDemoComplete(_)
        ));
    }

    #[test]
    fn parses_chatgpt_loopback_authorization_code() {
        let DeepLink::AuthCallback(search) =
            DeepLink::from_str("local://auth/callback?code=codex-code&state=s1&scope=openid")
                .unwrap()
        else {
            panic!("expected auth callback");
        };

        assert!(search.access_token.is_empty());
        assert!(search.refresh_token.is_empty());
        assert_eq!(search.code.as_deref(), Some("codex-code"));
        assert_eq!(search.state.as_deref(), Some("s1"));
    }

    #[test]
    fn parses_subscription_auth_custom_scheme_deeplink() {
        let DeepLink::AuthCallback(search) = DeepLink::from_str(
            "anarlog://auth/callback?code=ac_nf5hq&state=xYc5ZmNlqtWTu3BIbfbVQg",
        )
        .unwrap() else {
            panic!("expected auth callback");
        };

        assert!(search.access_token.is_empty());
        assert!(search.refresh_token.is_empty());
        assert_eq!(search.code.as_deref(), Some("ac_nf5hq"));
        assert_eq!(search.state.as_deref(), Some("xYc5ZmNlqtWTu3BIbfbVQg"));
    }
}
