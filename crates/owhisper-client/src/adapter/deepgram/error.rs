use serde::Deserialize;

use crate::error_detection::ProviderError;

#[derive(Deserialize)]
struct DeepgramError<'a> {
    #[serde(borrow)]
    err_code: Option<&'a str>,
    #[serde(borrow)]
    err_msg: Option<&'a str>,
    #[serde(borrow)]
    category: Option<&'a str>,
    #[serde(borrow)]
    message: Option<&'a str>,
}

pub fn detect_error(data: &[u8]) -> Option<ProviderError> {
    let text = std::str::from_utf8(data).ok()?;
    let parsed: DeepgramError = serde_json::from_str(text).ok()?;

    if !is_error_message(&parsed) {
        return None;
    }

    let code = determine_error_code(&parsed);
    let provider_code = parsed.err_code.or(parsed.category).map(|s| s.to_string());
    let message = parsed
        .err_msg
        .or(parsed.message)
        .unwrap_or("Unknown error")
        .to_string();

    let mut error = ProviderError::new(code, message);
    if let Some(pc) = provider_code {
        error = error.with_provider_code(pc);
    }
    Some(error)
}

fn is_error_message(parsed: &DeepgramError) -> bool {
    let has_fields = parsed.err_code.is_some()
        || parsed.err_msg.is_some()
        || parsed.category.is_some()
        || parsed.message.is_some();

    if !has_fields {
        return false;
    }

    parsed.err_code.is_some()
        || parsed.category.is_some()
        || parsed
            .err_msg
            .map(|m| m.to_lowercase().contains("error"))
            .unwrap_or(false)
}

fn determine_error_code(parsed: &DeepgramError) -> u16 {
    parsed
        .err_code
        .and_then(map_err_code)
        .or_else(|| parsed.category.and_then(map_category))
        .unwrap_or(500)
}

fn map_err_code(code: &str) -> Option<u16> {
    match code {
        "Bad Request" => Some(400),
        "INVALID_AUTH" | "INSUFFICIENT_PERMISSIONS" => Some(401),
        "ASR_PAYMENT_REQUIRED" => Some(402),
        "TOO_MANY_REQUESTS" => Some(429),
        "PROJECT_NOT_FOUND" => Some(404),
        _ => None,
    }
}

fn map_category(category: &str) -> Option<u16> {
    match category {
        "INVALID_JSON" => Some(400),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_error_messages() {
        for (data, http_code, provider_code, ws_close_code, message_check) in [
            (
                br#"{"err_code": "INVALID_AUTH", "err_msg": "Invalid credentials.", "request_id": "uuid"}"#
                    .as_slice(),
                401,
                Some("INVALID_AUTH"),
                4401,
                Some("Invalid credentials."),
            ),
            (
                br#"{"err_code": "Bad Request", "err_msg": "Bad Request: failed to process audio: corrupt or unsupported data", "request_id": "uuid"}"#
                    .as_slice(),
                400,
                Some("Bad Request"),
                4400,
                Some("failed to process audio"),
            ),
            (
                br#"{"category": "INVALID_JSON", "message": "Invalid JSON submitted.", "details": "Json deserialize error"}"#
                    .as_slice(),
                400,
                Some("INVALID_JSON"),
                4400,
                Some("Invalid JSON submitted."),
            ),
            (
                br#"{"err_code": "TOO_MANY_REQUESTS", "err_msg": "Too many requests. Please try again later", "request_id": "uuid"}"#
                    .as_slice(),
                429,
                Some("TOO_MANY_REQUESTS"),
                4429,
                None,
            ),
            (
                br#"{"err_code": "ASR_PAYMENT_REQUIRED", "err_msg": "Project does not have enough credits", "request_id": "uuid"}"#
                    .as_slice(),
                402,
                Some("ASR_PAYMENT_REQUIRED"),
                4402,
                None,
            ),
            (
                br#"{"err_code": "INSUFFICIENT_PERMISSIONS", "err_msg": "Access denied"}"#
                    .as_slice(),
                401,
                Some("INSUFFICIENT_PERMISSIONS"),
                4401,
                Some("Access denied"),
            ),
            (
                br#"{"err_code": "PROJECT_NOT_FOUND", "err_msg": "Project not found"}"#.as_slice(),
                404,
                Some("PROJECT_NOT_FOUND"),
                4404,
                None,
            ),
            (
                br#"{"err_code": "UNKNOWN_ERROR", "err_msg": "Something happened"}"#.as_slice(),
                500,
                Some("UNKNOWN_ERROR"),
                4500,
                None,
            ),
            (
                br#"{"err_msg": "An error occurred during processing"}"#.as_slice(),
                500,
                None,
                4500,
                Some("An error occurred during processing"),
            ),
        ] {
            let err = detect_error(data).unwrap();
            assert_eq!(err.http_code, http_code, "{data:?}");
            if let Some(expected) = message_check {
                assert!(
                    err.message == expected || err.message.contains(expected),
                    "{data:?}"
                );
            }
            assert_eq!(
                err.provider_code,
                provider_code.map(str::to_string),
                "{data:?}"
            );
            assert_eq!(err.to_ws_close_code(), ws_close_code, "{data:?}");
        }
    }

    #[test]
    fn ignores_non_error_and_empty_messages() {
        for data in [
            br#"{"message": "Processing complete"}"#.as_slice(),
            br#"{"type": "Results", "channel_index": [0, 1], "duration": 1.0}"#.as_slice(),
            br#"{}"#.as_slice(),
        ] {
            assert!(detect_error(data).is_none(), "{data:?}");
        }
    }
}
