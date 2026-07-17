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
    fn test_invalid_auth() {
        let data = br#"{"err_code": "INVALID_AUTH", "err_msg": "Invalid credentials.", "request_id": "uuid"}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 401);
        assert_eq!(err.message, "Invalid credentials.");
        assert_eq!(err.provider_code, Some("INVALID_AUTH".to_string()));
        assert_eq!(err.to_ws_close_code(), 4401);
    }

    #[test]
    fn test_bad_request() {
        let data = br#"{"err_code": "Bad Request", "err_msg": "Bad Request: failed to process audio: corrupt or unsupported data", "request_id": "uuid"}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 400);
        assert!(err.message.contains("failed to process audio"));
        assert_eq!(err.to_ws_close_code(), 4400);
    }

    #[test]
    fn test_invalid_json() {
        let data = br#"{"category": "INVALID_JSON", "message": "Invalid JSON submitted.", "details": "Json deserialize error"}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 400);
        assert_eq!(err.message, "Invalid JSON submitted.");
        assert_eq!(err.provider_code, Some("INVALID_JSON".to_string()));
    }

    #[test]
    fn test_rate_limit() {
        let data = br#"{"err_code": "TOO_MANY_REQUESTS", "err_msg": "Too many requests. Please try again later", "request_id": "uuid"}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 429);
        assert_eq!(err.to_ws_close_code(), 4429);
    }

    #[test]
    fn test_payment_required() {
        let data = br#"{"err_code": "ASR_PAYMENT_REQUIRED", "err_msg": "Project does not have enough credits", "request_id": "uuid"}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 402);
        assert_eq!(err.to_ws_close_code(), 4402);
    }

    #[test]
    fn test_insufficient_permissions() {
        let data = br#"{"err_code": "INSUFFICIENT_PERMISSIONS", "err_msg": "Access denied"}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 401);
        assert_eq!(err.message, "Access denied");
        assert_eq!(err.to_ws_close_code(), 4401);
    }

    #[test]
    fn test_project_not_found() {
        let data = br#"{"err_code": "PROJECT_NOT_FOUND", "err_msg": "Project not found"}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 404);
        assert_eq!(err.to_ws_close_code(), 4404);
    }

    #[test]
    fn test_unknown_err_code() {
        let data = br#"{"err_code": "UNKNOWN_ERROR", "err_msg": "Something happened"}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 500);
        assert_eq!(err.provider_code, Some("UNKNOWN_ERROR".to_string()));
    }

    #[test]
    fn test_message_with_error_keyword() {
        let data = br#"{"err_msg": "An error occurred during processing"}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 500);
        assert_eq!(err.message, "An error occurred during processing");
    }

    #[test]
    fn test_message_without_error_keyword() {
        let data = br#"{"message": "Processing complete"}"#;
        assert!(detect_error(data).is_none());
    }

    #[test]
    fn test_non_error_message() {
        let data = br#"{"type": "Results", "channel_index": [0, 1], "duration": 1.0}"#;
        assert!(detect_error(data).is_none());
    }

    #[test]
    fn test_empty_json() {
        let data = br#"{}"#;
        assert!(detect_error(data).is_none());
    }
}
