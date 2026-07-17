// https://elevenlabs.io/docs/api-reference/speech-to-text/v-1-speech-to-text-realtime
// https://elevenlabs.io/docs/developers/resources/error-messages

use serde::Deserialize;

use crate::error_detection::ProviderError;

const ERROR_MESSAGE_TYPES: &[&str] = &[
    "error",
    "auth_error",
    "quota_exceeded",
    "commit_throttled",
    "unaccepted_terms",
    "rate_limited",
    "queue_overflow",
    "resource_exhausted",
    "session_time_limit_exceeded",
    "input_error",
    "chunk_size_exceeded",
    "insufficient_audio_activity",
    "transcriber_error",
];

#[derive(Deserialize)]
struct ElevenLabsError<'a> {
    #[serde(borrow)]
    message_type: Option<&'a str>,
    #[serde(borrow)]
    error: Option<&'a str>,
}

pub fn detect_error(data: &[u8]) -> Option<ProviderError> {
    let text = std::str::from_utf8(data).ok()?;
    let parsed: ElevenLabsError = serde_json::from_str(text).ok()?;

    let message_type = parsed.message_type?;
    if !ERROR_MESSAGE_TYPES.contains(&message_type) {
        return None;
    }

    let code = map_error_type(message_type);
    let message = parsed.error.unwrap_or("Unknown error").to_string();

    Some(ProviderError::new(code, message).with_provider_code(message_type))
}

fn map_error_type(message_type: &str) -> u16 {
    match message_type {
        "auth_error" => 401,
        "quota_exceeded" => 402,
        "unaccepted_terms" => 403,
        "session_time_limit_exceeded" => 408,
        "chunk_size_exceeded" => 413,
        "rate_limited" | "commit_throttled" => 429,
        "input_error" | "insufficient_audio_activity" => 400,
        "queue_overflow" | "resource_exhausted" => 503,
        "transcriber_error" | "error" => 500,
        _ => 500,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_error() {
        let data = br#"{"message_type": "auth_error", "error": "Invalid API key."}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 401);
        assert_eq!(err.message, "Invalid API key.");
        assert_eq!(err.provider_code, Some("auth_error".to_string()));
        assert_eq!(err.to_ws_close_code(), 4401);
    }

    #[test]
    fn test_quota_exceeded() {
        let data =
            br#"{"message_type": "quota_exceeded", "error": "Your usage quota has been reached."}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 402);
        assert_eq!(err.message, "Your usage quota has been reached.");
        assert_eq!(err.provider_code, Some("quota_exceeded".to_string()));
        assert_eq!(err.to_ws_close_code(), 4402);
    }

    #[test]
    fn test_input_error() {
        let data = br#"{"message_type": "input_error", "error": "Audio format invalid or not supported."}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 400);
        assert_eq!(err.message, "Audio format invalid or not supported.");
        assert_eq!(err.provider_code, Some("input_error".to_string()));
        assert_eq!(err.to_ws_close_code(), 4400);
    }

    #[test]
    fn test_rate_limited() {
        let data = br#"{"message_type": "rate_limited", "error": "Too many requests."}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 429);
        assert_eq!(err.message, "Too many requests.");
        assert_eq!(err.to_ws_close_code(), 4429);
    }

    #[test]
    fn test_transcriber_error() {
        let data =
            br#"{"message_type": "transcriber_error", "error": "Internal transcription failure."}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 500);
        assert_eq!(err.message, "Internal transcription failure.");
        assert_eq!(err.to_ws_close_code(), 4500);
    }

    #[test]
    fn test_queue_overflow() {
        let data = br#"{"message_type": "queue_overflow", "error": "Internal queue overloaded."}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 503);
        assert_eq!(err.to_ws_close_code(), 4500);
    }

    #[test]
    fn test_resource_exhausted() {
        let data = br#"{"message_type": "resource_exhausted", "error": "Resource exhausted."}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 503);
        assert_eq!(err.provider_code, Some("resource_exhausted".to_string()));
        assert_eq!(err.to_ws_close_code(), 4500);
    }

    #[test]
    fn test_commit_throttled() {
        let data = br#"{"message_type": "commit_throttled", "error": "Too many commit calls."}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 429);
        assert_eq!(err.to_ws_close_code(), 4429);
    }

    #[test]
    fn test_unaccepted_terms() {
        let data =
            br#"{"message_type": "unaccepted_terms", "error": "Terms of service not accepted."}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 403);
        assert_eq!(err.to_ws_close_code(), 4403);
    }

    #[test]
    fn test_session_time_limit() {
        let data = br#"{"message_type": "session_time_limit_exceeded", "error": "Session exceeded max duration."}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 408);
        assert_eq!(err.to_ws_close_code(), 4000);
    }

    #[test]
    fn test_chunk_size_exceeded() {
        let data =
            br#"{"message_type": "chunk_size_exceeded", "error": "Audio chunks too large."}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 413);
        assert_eq!(err.to_ws_close_code(), 4000);
    }

    #[test]
    fn test_insufficient_audio() {
        let data =
            br#"{"message_type": "insufficient_audio_activity", "error": "No speech detected."}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 400);
        assert_eq!(err.to_ws_close_code(), 4400);
    }

    #[test]
    fn test_generic_error() {
        let data = br#"{"message_type": "error", "error": "Server error."}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 500);
        assert_eq!(err.to_ws_close_code(), 4500);
    }

    #[test]
    fn test_error_without_message() {
        let data = br#"{"message_type": "auth_error"}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 401);
        assert_eq!(err.message, "Unknown error");
    }

    #[test]
    fn test_non_error_message() {
        let data = br#"{"message_type": "partial_transcript", "text": "hello"}"#;
        assert!(detect_error(data).is_none());
    }

    #[test]
    fn test_session_started() {
        let data = br#"{"message_type": "session_started", "session_id": "abc123"}"#;
        assert!(detect_error(data).is_none());
    }

    #[test]
    fn test_empty_json() {
        let data = br#"{}"#;
        assert!(detect_error(data).is_none());
    }
}
