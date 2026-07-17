use std::fmt;

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize, Type)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ShareOpenRequest {
    Account { share_id: String },
    Handoff { request_id: String },
}

impl ShareOpenRequest {
    pub(crate) fn parse(parsed: &url::Url) -> Result<Self, crate::Error> {
        if !matches!(parsed.scheme(), "hyprnote" | "hyprnote-staging")
            || parsed.host_str() != Some("share")
            || parsed.path() != "/open"
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.port().is_some()
            || parsed.fragment().is_some()
        {
            return Err(crate::Error::InvalidShareOpen);
        }

        let query = parsed.query().ok_or(crate::Error::InvalidShareOpen)?;
        if query
            .split('&')
            .any(|pair| pair.is_empty() || !pair.contains('='))
        {
            return Err(crate::Error::InvalidShareOpen);
        }

        let mut mode = None;
        let mut share_id = None;
        let mut request_id = None;

        for (key, value) in parsed.query_pairs() {
            let target = match key.as_ref() {
                "mode" => &mut mode,
                "share_id" => &mut share_id,
                "request_id" => &mut request_id,
                _ => return Err(crate::Error::InvalidShareOpen),
            };
            if target.replace(value.into_owned()).is_some() {
                return Err(crate::Error::InvalidShareOpen);
            }
        }

        match (mode.as_deref(), share_id, request_id) {
            (Some("account"), Some(share_id), None) => {
                validate_uuid(&share_id)?;
                Ok(Self::Account { share_id })
            }
            (Some("handoff"), None, Some(request_id)) => {
                validate_uuid(&request_id)?;
                Ok(Self::Handoff { request_id })
            }
            _ => Err(crate::Error::InvalidShareOpen),
        }
    }
}

impl fmt::Debug for ShareOpenRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Account { .. } => f
                .debug_struct("Account")
                .field("share_id", &"[REDACTED]")
                .finish(),
            Self::Handoff { .. } => f
                .debug_struct("Handoff")
                .field("request_id", &"[REDACTED]")
                .finish(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Type, tauri_specta::Event)]
pub struct ShareOpenPendingEvent {
    pub pending_id: String,
}

fn validate_uuid(value: &str) -> Result<(), crate::Error> {
    let parsed = Uuid::parse_str(value).map_err(|_| crate::Error::InvalidShareOpen)?;
    if parsed.is_nil() || parsed.get_version_num() != 4 || parsed.hyphenated().to_string() != value
    {
        return Err(crate::Error::InvalidShareOpen);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHARE_ID: &str = "40bc9d36-7634-4c48-988f-6a3e301467e7";
    const REQUEST_ID: &str = "ba5ca57a-8f88-44e8-ab92-f9e10c89425c";

    fn parse(value: &str) -> Result<ShareOpenRequest, crate::Error> {
        let parsed = url::Url::parse(value).unwrap();
        ShareOpenRequest::parse(&parsed)
    }

    #[test]
    fn parses_only_account_and_handoff_routes() {
        for scheme in ["hyprnote", "hyprnote-staging"] {
            assert!(matches!(
                parse(&format!(
                    "{scheme}://share/open?mode=account&share_id={SHARE_ID}"
                )),
                Ok(ShareOpenRequest::Account { share_id }) if share_id == SHARE_ID
            ));
            assert!(matches!(
                parse(&format!(
                    "{scheme}://share/open?request_id={REQUEST_ID}&mode=handoff"
                )),
                Ok(ShareOpenRequest::Handoff { request_id }) if request_id == REQUEST_ID
            ));
        }
    }

    #[test]
    fn rejects_noncanonical_or_ambiguous_routes() {
        let invalid = [
            format!("hypr://share/open?mode=account&share_id={SHARE_ID}"),
            format!("char://share/open?mode=account&share_id={SHARE_ID}"),
            format!("hyprnote://share/open/?mode=account&share_id={SHARE_ID}"),
            format!("hyprnote://share/open?mode=account&share_id={SHARE_ID}#fragment"),
            format!("hyprnote://share/open?mode=account&share_id={SHARE_ID}&extra=1"),
            format!("hyprnote://share/open?mode=account&share_id={SHARE_ID}&"),
            format!("hyprnote://share/open?mode=account&share_id={SHARE_ID}&extra"),
            format!("hyprnote://share/open?mode=account&mode=handoff&share_id={SHARE_ID}"),
            format!("hyprnote://share/open?mode=account&request_id={REQUEST_ID}"),
            format!("hyprnote://share/open?mode=handoff&share_id={SHARE_ID}"),
            format!("hyprnote://share/open?mode=public&public_slug=s_deadbeef"),
            format!("hyprnote://share/open?mode=link&token=secret&share_id={SHARE_ID}"),
            "hyprnote://share/open?mode=account&share_id=00000000-0000-0000-0000-000000000000"
                .to_string(),
            format!(
                "hyprnote://share/open?mode=account&share_id={}",
                SHARE_ID.to_uppercase()
            ),
        ];

        for value in invalid {
            assert!(parse(&value).is_err(), "accepted {value}");
        }
    }

    #[test]
    fn debug_output_redacts_external_identifiers() {
        let account = parse(&format!(
            "hyprnote://share/open?mode=account&share_id={SHARE_ID}"
        ))
        .unwrap();
        let handoff = parse(&format!(
            "hyprnote://share/open?mode=handoff&request_id={REQUEST_ID}"
        ))
        .unwrap();

        let output = format!("{account:?} {handoff:?}");
        assert!(!output.contains(SHARE_ID));
        assert!(!output.contains(REQUEST_ID));
        assert_eq!(output.matches("[REDACTED]").count(), 2);
    }
}
