use std::{collections::VecDeque, fmt, sync::Arc};

use serde::Deserialize;
use url::Url;
use uuid::Uuid;

use crate::{Result, ServiceError};

const SCHEMES: &[&str] = &[
    "anarlog",
    "anarlog-staging",
    "anarlog-nightly",
    "anarlog-dev",
    "hyprnote",
    "hyprnote-staging",
    "hyprnote-nightly",
    "hypr",
];

#[derive(Clone, Default, Deserialize)]
pub struct AuthCallback {
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    pub code: Option<String>,
    pub state: Option<String>,
}

#[derive(Clone, Deserialize)]
pub struct IntegrationCallback {
    pub integration_id: String,
    pub status: String,
    pub disconnected_connection_id: Option<String>,
    pub return_to: Option<String>,
}

#[derive(Clone)]
pub enum DeepLink {
    Auth(AuthCallback),
    BillingRefresh,
    Integration(IntegrationCallback),
    OnboardingComplete,
    ShareAccount { share_id: String },
    ShareHandoff { request_id: String },
}

impl fmt::Debug for DeepLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Auth(_) => "Auth([redacted])",
            Self::BillingRefresh => "BillingRefresh",
            Self::Integration(_) => "Integration([redacted])",
            Self::OnboardingComplete => "OnboardingComplete",
            Self::ShareAccount { .. } => "ShareAccount([redacted])",
            Self::ShareHandoff { .. } => "ShareHandoff([redacted])",
        })
    }
}

fn invalid() -> ServiceError {
    ServiceError::Failed("Invalid or unsupported deep link".into())
}

impl DeepLink {
    pub fn parse(raw: &str) -> Result<Self> {
        let raw = raw.trim_matches(|c: char| c.is_ascii_whitespace());
        if raw.len() > 512
            && SCHEMES.iter().any(|scheme| {
                raw.get(..scheme.len() + 13).is_some_and(|prefix| {
                    prefix.eq_ignore_ascii_case(&format!("{scheme}://share/open"))
                })
            })
        {
            return Err(invalid());
        }
        if raw.len() > 64 * 1024 {
            return Err(invalid());
        }
        let url = Url::parse(raw).map_err(|_| invalid())?;
        let path = format!(
            "{}/{}",
            url.host_str().unwrap_or(""),
            url.path().trim_start_matches('/')
        );
        let query = url.query().unwrap_or("");
        match path.as_str() {
            "auth/callback" => serde_qs::from_str(query)
                .map(Self::Auth)
                .map_err(|_| invalid()),
            "billing/refresh" => Ok(Self::BillingRefresh),
            "integration/callback" => serde_qs::from_str(query)
                .map(Self::Integration)
                .map_err(|_| invalid()),
            "onboarding-demo/complete" => Ok(Self::OnboardingComplete),
            "share/open" => Self::share(&url, raw.len()),
            _ => Err(invalid()),
        }
    }

    fn share(url: &Url, bytes: usize) -> Result<Self> {
        if bytes > 512
            || !SCHEMES.contains(&url.scheme())
            || url.host_str() != Some("share")
            || url.path() != "/open"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
            || url.fragment().is_some()
        {
            return Err(invalid());
        }
        let query = url.query().ok_or_else(invalid)?;
        if query
            .split('&')
            .any(|pair| pair.is_empty() || !pair.contains('='))
        {
            return Err(invalid());
        }
        let (mut mode, mut share, mut request) = (None, None, None);
        for (key, value) in url.query_pairs() {
            let slot = match key.as_ref() {
                "mode" => &mut mode,
                "share_id" => &mut share,
                "request_id" => &mut request,
                _ => return Err(invalid()),
            };
            if slot.replace(value.into_owned()).is_some() {
                return Err(invalid());
            }
        }
        let (link, id) = match (mode.as_deref(), share, request) {
            (Some("account"), Some(id), None) => (
                Self::ShareAccount {
                    share_id: id.clone(),
                },
                id,
            ),
            (Some("handoff"), None, Some(id)) => (
                Self::ShareHandoff {
                    request_id: id.clone(),
                },
                id,
            ),
            _ => return Err(invalid()),
        };
        let uuid = Uuid::parse_str(&id).map_err(|_| invalid())?;
        if uuid.is_nil() || uuid.get_version_num() != 4 || uuid.hyphenated().to_string() != id {
            return Err(invalid());
        }
        Ok(link)
    }
}

/// Callbacks stay queued until the domain handler acknowledges them.
#[derive(Default)]
pub struct DeepLinkInbox {
    pending: VecDeque<(u64, Arc<DeepLink>)>,
    next_id: u64,
}

impl DeepLinkInbox {
    pub fn push(&mut self, raw: &str) -> Result<u64> {
        if self.pending.len() == 64 {
            return Err(ServiceError::Busy);
        }
        let link = DeepLink::parse(raw)?;
        self.next_id = self.next_id.checked_add(1).ok_or(ServiceError::Busy)?;
        self.pending.push_back((self.next_id, Arc::new(link)));
        Ok(self.next_id)
    }

    pub fn pending(&self, shares: bool) -> impl Iterator<Item = &(u64, Arc<DeepLink>)> {
        self.pending.iter().filter(move |(_, link)| {
            matches!(
                link.as_ref(),
                DeepLink::ShareAccount { .. } | DeepLink::ShareHandoff { .. }
            ) == shares
        })
    }

    pub fn acknowledge(&mut self, id: u64) -> bool {
        let Some(index) = self.pending.iter().position(|(pending, _)| *pending == id) else {
            return false;
        };
        self.pending.remove(index);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "40bc9d36-7634-4c48-988f-6a3e301467e7";

    #[test]
    fn shipping_callbacks_and_share_validation() {
        let auth = DeepLink::parse("local://auth/callback?code=private&state=s1").unwrap();
        assert!(!format!("{auth:?}").contains("private"));
        for scheme in SCHEMES {
            assert!(
                DeepLink::parse(&format!("{scheme}://share/open?mode=account&share_id={ID}"))
                    .is_ok()
            );
        }
        for suffix in [
            "&mode=account",
            "&unknown=1",
            "#fragment",
            "&",
            "&request_id=x",
        ] {
            assert!(
                DeepLink::parse(&format!(
                    "anarlog://share/open?mode=account&share_id={ID}{suffix}"
                ))
                .is_err()
            );
        }
        assert!(DeepLink::parse(&format!("anarlog://share/open?{}", "x=1&".repeat(200))).is_err());
        assert!(DeepLink::parse("anarlog://integration/callback?status=success").is_err());
    }

    #[test]
    fn startup_and_live_links_require_ack_and_share_waits_for_signin() {
        let mut inbox = DeepLinkInbox::default();
        let first = inbox.push("anarlog://billing/refresh").unwrap();
        inbox
            .push(&format!("anarlog://share/open?mode=account&share_id={ID}"))
            .unwrap();
        assert_eq!(inbox.pending(false).count(), 1);
        assert_eq!(inbox.pending(true).count(), 1);
        assert!(inbox.acknowledge(first));
        assert!(!inbox.acknowledge(first));
        for _ in 0..63 {
            inbox.push("anarlog://billing/refresh").unwrap();
        }
        assert!(matches!(
            inbox.push("anarlog://billing/refresh"),
            Err(ServiceError::Busy)
        ));
    }
}
