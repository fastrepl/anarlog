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
    fn classifies_error_messages() {
        for (data, http_code, provider_code, ws_close_code, message_part) in [
            (
                br#"{"error": "Unauthorized Connection: Missing Authorization header"}"#.as_slice(),
                401,
                Some("UNAUTHORIZED"),
                4401,
                "Missing Authorization",
            ),
            (
                br#"{"error": "Unauthorized Connection: Too many concurrent sessions"}"#.as_slice(),
                429,
                Some("TOO_MANY_CONCURRENT"),
                4429,
                "Too many concurrent",
            ),
            (
                br#"{"error": "Session Expired: Maximum session duration exceeded"}"#.as_slice(),
                408,
                Some("SESSION_EXPIRED"),
                4000,
                "session duration",
            ),
            (
                br#"{"error": "Input duration violation: 25 ms. Expected between 50 and 1000 ms"}"#
                    .as_slice(),
                400,
                Some("INPUT_DURATION_VIOLATION"),
                4400,
                "duration violation",
            ),
            (
                br#"{"error": "Invalid JSON: unexpected token"}"#.as_slice(),
                400,
                Some("INVALID_JSON"),
                4400,
                "Invalid JSON",
            ),
            (
                br#"{"error": "Invalid Message Type: unknown_type"}"#.as_slice(),
                400,
                Some("INVALID_MESSAGE"),
                4400,
                "Invalid Message",
            ),
            (
                br#"{"error": "Audio Transmission Rate Exceeded: Received 10 sec. audio in 5 sec"}"#
                    .as_slice(),
                429,
                Some("AUDIO_RATE_EXCEEDED"),
                4429,
                "Audio Transmission Rate",
            ),
            (
                br#"{"error": "Session Cancelled: An error occurred"}"#.as_slice(),
                500,
                Some("SESSION_CANCELLED"),
                4500,
                "Session Cancelled",
            ),
            (
                br#"{"status": "error", "error": "Download error, unable to access file at https://example.com"}"#
                    .as_slice(),
                400,
                Some("DOWNLOAD_ERROR"),
                4400,
                "Download error",
            ),
            (
                br#"{"error": "Something unexpected happened"}"#.as_slice(),
                500,
                None,
                4500,
                "Something unexpected happened",
            ),
        ] {
            let err = detect_error(data).unwrap();
            assert_eq!(err.http_code, http_code, "{data:?}");
            assert!(err.message.contains(message_part), "{data:?}");
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
            br#"{"type": "Begin", "id": "abc123", "expires_at": "2024-01-01T00:00:00Z"}"#.as_slice(),
            br#"{"type": "Turn", "turn_order": 1, "transcript": "hello"}"#.as_slice(),
            br#"{"type": "Termination", "audio_duration_seconds": 60, "session_duration_seconds": 65}"#
                .as_slice(),
            br#"{}"#.as_slice(),
        ] {
            assert!(detect_error(data).is_none(), "{data:?}");
        }
    }
}
