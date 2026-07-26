// https://www.assemblyai.com/docs/universal-streaming/common-session-errors-and-closures.md
// https://www.assemblyai.com/docs/api-reference/overview.md

use serde::Deserialize;

use crate::error_detection::ProviderError;

#[derive(Deserialize)]
struct AssemblyAIError<'a> {
    #[serde(borrow)]
    error: Option<&'a str>,
    #[serde(borrow)]
    status: Option<&'a str>,
}

pub fn detect_error(data: &[u8]) -> Option<ProviderError> {
    let text = std::str::from_utf8(data).ok()?;
    let parsed: AssemblyAIError = serde_json::from_str(text).ok()?;

    if !is_error_message(&parsed) {
        return None;
    }

    let code = determine_error_code(&parsed);
    let message = parsed.error.unwrap_or("Unknown error").to_string();
    let provider_code = extract_provider_code(&parsed);

    let mut error = ProviderError::new(code, message);
    if let Some(pc) = provider_code {
        error = error.with_provider_code(pc);
    }
    Some(error)
}

fn is_error_message(parsed: &AssemblyAIError) -> bool {
    if parsed.error.is_some() {
        return true;
    }
    if parsed.status == Some("error") {
        return true;
    }
    false
}

fn determine_error_code(parsed: &AssemblyAIError) -> u16 {
    let error_msg = parsed.error.unwrap_or("");
    let lower = error_msg.to_lowercase();

    if lower.contains("too many concurrent") {
        return 429;
    }
    if lower.contains("audio transmission rate exceeded") {
        return 429;
    }
    if lower.contains("missing authorization") {
        return 401;
    }
    if lower.contains("unauthorized") && !lower.contains("too many") {
        return 401;
    }
    if lower.contains("session expired") || lower.contains("maximum session duration") {
        return 408;
    }
    if lower.contains("input duration violation") {
        return 400;
    }
    if lower.contains("invalid message") || lower.contains("invalid json") {
        return 400;
    }
    if lower.contains("download error") {
        return 400;
    }
    if lower.contains("insufficient") && lower.contains("balance") {
        return 402;
    }
    if lower.contains("account") && lower.contains("disabled") {
        return 403;
    }

    500
}

fn extract_provider_code(parsed: &AssemblyAIError) -> Option<String> {
    let error_msg = parsed.error?;
    let lower = error_msg.to_lowercase();

    if lower.contains("too many concurrent") {
        Some("TOO_MANY_CONCURRENT".to_string())
    } else if lower.contains("audio transmission rate exceeded") {
        Some("AUDIO_RATE_EXCEEDED".to_string())
    } else if lower.contains("missing authorization") || lower.contains("unauthorized") {
        Some("UNAUTHORIZED".to_string())
    } else if lower.contains("session expired") {
        Some("SESSION_EXPIRED".to_string())
    } else if lower.contains("input duration violation") {
        Some("INPUT_DURATION_VIOLATION".to_string())
    } else if lower.contains("invalid message") {
        Some("INVALID_MESSAGE".to_string())
    } else if lower.contains("invalid json") {
        Some("INVALID_JSON".to_string())
    } else if lower.contains("download error") {
        Some("DOWNLOAD_ERROR".to_string())
    } else if lower.contains("session cancelled") {
        Some("SESSION_CANCELLED".to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unauthorized_missing_header() {
        let data = br#"{"error": "Unauthorized Connection: Missing Authorization header"}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 401);
        assert!(err.message.contains("Missing Authorization"));
        assert_eq!(err.provider_code, Some("UNAUTHORIZED".to_string()));
        assert_eq!(err.to_ws_close_code(), 4401);
    }

    #[test]
    fn test_too_many_concurrent() {
        let data = br#"{"error": "Unauthorized Connection: Too many concurrent sessions"}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 429);
        assert!(err.message.contains("Too many concurrent"));
        assert_eq!(err.provider_code, Some("TOO_MANY_CONCURRENT".to_string()));
        assert_eq!(err.to_ws_close_code(), 4429);
    }

    #[test]
    fn test_session_expired() {
        let data = br#"{"error": "Session Expired: Maximum session duration exceeded"}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 408);
        assert!(err.message.contains("session duration"));
        assert_eq!(err.provider_code, Some("SESSION_EXPIRED".to_string()));
        assert_eq!(err.to_ws_close_code(), 4000);
    }

    #[test]
    fn test_input_duration_violation() {
        let data =
            br#"{"error": "Input duration violation: 25 ms. Expected between 50 and 1000 ms"}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 400);
        assert!(err.message.contains("duration violation"));
        assert_eq!(
            err.provider_code,
            Some("INPUT_DURATION_VIOLATION".to_string())
        );
        assert_eq!(err.to_ws_close_code(), 4400);
    }

    #[test]
    fn test_invalid_json() {
        let data = br#"{"error": "Invalid JSON: unexpected token"}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 400);
        assert!(err.message.contains("Invalid JSON"));
        assert_eq!(err.provider_code, Some("INVALID_JSON".to_string()));
        assert_eq!(err.to_ws_close_code(), 4400);
    }

    #[test]
    fn test_invalid_message_type() {
        let data = br#"{"error": "Invalid Message Type: unknown_type"}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 400);
        assert!(err.message.contains("Invalid Message"));
        assert_eq!(err.provider_code, Some("INVALID_MESSAGE".to_string()));
        assert_eq!(err.to_ws_close_code(), 4400);
    }

    #[test]
    fn test_audio_rate_exceeded() {
        let data =
            br#"{"error": "Audio Transmission Rate Exceeded: Received 10 sec. audio in 5 sec"}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 429);
        assert!(err.message.contains("Audio Transmission Rate"));
        assert_eq!(err.provider_code, Some("AUDIO_RATE_EXCEEDED".to_string()));
        assert_eq!(err.to_ws_close_code(), 4429);
    }

    #[test]
    fn test_session_cancelled() {
        let data = br#"{"error": "Session Cancelled: An error occurred"}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 500);
        assert!(err.message.contains("Session Cancelled"));
        assert_eq!(err.provider_code, Some("SESSION_CANCELLED".to_string()));
        assert_eq!(err.to_ws_close_code(), 4500);
    }

    #[test]
    fn test_status_error() {
        let data = br#"{"status": "error", "error": "Download error, unable to access file at https://example.com"}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 400);
        assert!(err.message.contains("Download error"));
        assert_eq!(err.provider_code, Some("DOWNLOAD_ERROR".to_string()));
    }

    #[test]
    fn test_unknown_error() {
        let data = br#"{"error": "Something unexpected happened"}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 500);
        assert_eq!(err.message, "Something unexpected happened");
        assert_eq!(err.provider_code, None);
        assert_eq!(err.to_ws_close_code(), 4500);
    }

    #[test]
    fn test_non_error_begin_message() {
        let data = br#"{"type": "Begin", "id": "abc123", "expires_at": "2024-01-01T00:00:00Z"}"#;
        assert!(detect_error(data).is_none());
    }

    #[test]
    fn test_non_error_turn_message() {
        let data = br#"{"type": "Turn", "turn_order": 1, "transcript": "hello"}"#;
        assert!(detect_error(data).is_none());
    }

    #[test]
    fn test_non_error_termination_message() {
        let data =
            br#"{"type": "Termination", "audio_duration_seconds": 60, "session_duration_seconds": 65}"#;
        assert!(detect_error(data).is_none());
    }

    #[test]
    fn test_empty_json() {
        let data = br#"{}"#;
        assert!(detect_error(data).is_none());
    }
}
