use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::NaiveDate;
use serde_json::Value;

use crate::session::Session;

use super::{Error, Result};

/// Earliest API version whose response bodies expose the structured `code` field.
///
/// Mirrors `API_VERSIONS['2024-01-01']` in supabase-js `lib/constants.ts`.
fn min_api_version_for_code() -> NaiveDate {
    NaiveDate::from_ymd_opt(2024, 1, 1).expect("2024-01-01 is a valid date")
}

pub(super) fn parse_session(body: &str) -> Result<Session> {
    let mut session: Session = serde_json::from_str(body)?;
    if session.access_token.is_empty()
        || session.refresh_token().is_none()
        || !session.expires_in.is_some_and(|expires_in| expires_in > 0)
    {
        return Err(invalid_session_payload("session missing required fields").into());
    }

    if session.expires_at.is_none() {
        session.expires_at = session
            .expires_in
            .map(|expires_in| now_epoch_secs().saturating_add(expires_in));
    }

    Ok(session)
}

pub(super) fn error_from_response(
    status: u16,
    body: &str,
    api_version: Option<NaiveDate>,
) -> Error {
    let (code, message) = extract_error(body, api_version);

    if code.as_deref() == Some("session_not_found") {
        return Error::SessionMissing;
    }

    Error::Api {
        status,
        code,
        message,
    }
}

/// Parses the `X-Supabase-Api-Version` response header.
///
/// Matches supabase-js' `parseResponseAPIVersion` in `lib/helpers.ts`.
pub(super) fn parse_api_version(raw: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok()
}

fn extract_error(body: &str, api_version: Option<NaiveDate>) -> (Option<String>, String) {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return (None, body.to_string());
    };

    let code = resolve_error_code(&value, api_version);
    let message = resolve_error_message(&value).unwrap_or_else(|| body.to_string());
    (code, message)
}

/// Resolves the error code using the same precedence as supabase-js' `handleError`.
///
/// `data.code` is only trusted when the response advertises api version >= 2024-01-01
/// because legacy GoTrue servers put the numeric HTTP status in that field. Otherwise
/// we fall back to `data.error_code`.
fn resolve_error_code(value: &Value, api_version: Option<NaiveDate>) -> Option<String> {
    let current_api = api_version
        .map(|version| version >= min_api_version_for_code())
        .unwrap_or(false);

    if current_api {
        if let Some(code) = value.get("code").and_then(Value::as_str) {
            return Some(code.to_string());
        }
    }

    value
        .get("error_code")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// Resolves the error message using the same precedence as supabase-js' `_getErrorMessage`:
/// `msg > message > error_description > error`.
fn resolve_error_message(value: &Value) -> Option<String> {
    for key in ["msg", "message", "error_description", "error"] {
        if let Some(message) = value.get(key).and_then(Value::as_str) {
            return Some(message.to_string());
        }
    }
    None
}

fn invalid_session_payload(message: &str) -> serde_json::Error {
    serde_json::Error::io(io::Error::new(io::ErrorKind::InvalidData, message))
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
