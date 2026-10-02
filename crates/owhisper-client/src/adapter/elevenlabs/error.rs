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
    fn classifies_error_messages() {
        for (data, http_code, ws_close_code) in [
            (
                br#"{"message_type": "auth_error", "error": "Invalid API key."}"#.as_slice(),
                401,
                4401,
            ),
            (
                br#"{"message_type": "quota_exceeded", "error": "Your usage quota has been reached."}"#
                    .as_slice(),
                402,
                4402,
            ),
            (
                br#"{"message_type": "unaccepted_terms", "error": "Terms of service not accepted."}"#
                    .as_slice(),
                403,
                4403,
            ),
            (
                br#"{"message_type": "session_time_limit_exceeded", "error": "Session exceeded max duration."}"#
                    .as_slice(),
                408,
                4000,
            ),
            (
                br#"{"message_type": "chunk_size_exceeded", "error": "Audio chunks too large."}"#
                    .as_slice(),
                413,
                4000,
            ),
            (
                br#"{"message_type": "rate_limited", "error": "Too many requests."}"#.as_slice(),
                429,
                4429,
            ),
            (
                br#"{"message_type": "commit_throttled", "error": "Too many commit calls."}"#
                    .as_slice(),
                429,
                4429,
            ),
            (
                br#"{"message_type": "input_error", "error": "Audio format invalid or not supported."}"#
                    .as_slice(),
                400,
                4400,
            ),
            (
                br#"{"message_type": "insufficient_audio_activity", "error": "No speech detected."}"#
                    .as_slice(),
                400,
                4400,
            ),
            (
                br#"{"message_type": "queue_overflow", "error": "Internal queue overloaded."}"#
                    .as_slice(),
                503,
                4500,
            ),
            (
                br#"{"message_type": "resource_exhausted", "error": "Resource exhausted."}"#
                    .as_slice(),
                503,
                4500,
            ),
            (
                br#"{"message_type": "transcriber_error", "error": "Internal transcription failure."}"#
                    .as_slice(),
                500,
                4500,
            ),
            (
                br#"{"message_type": "error", "error": "Server error."}"#.as_slice(),
                500,
                4500,
            ),
        ] {
            let err = detect_error(data).unwrap();
            assert_eq!(err.http_code, http_code, "{data:?}");
            assert_eq!(err.to_ws_close_code(), ws_close_code, "{data:?}");
            let parsed: serde_json::Value = serde_json::from_slice(data).unwrap();
            assert_eq!(
                err.provider_code,
                Some(parsed["message_type"].as_str().unwrap().to_string()),
                "{data:?}"
            );
        }
    }

    #[test]
    fn error_payload_carries_message_and_provider_code() {
        let err = detect_error(br#"{"message_type": "auth_error", "error": "Invalid API key."}"#)
            .unwrap();
        assert_eq!(err.message, "Invalid API key.");

        let err = detect_error(br#"{"message_type": "auth_error"}"#).unwrap();
        assert_eq!(err.http_code, 401);
        assert_eq!(err.message, "Unknown error");
    }

    #[test]
    fn ignores_non_error_and_empty_messages() {
        for data in [
            br#"{"message_type": "partial_transcript", "text": "hello"}"#.as_slice(),
            br#"{"message_type": "session_started", "session_id": "abc123"}"#.as_slice(),
            br#"{}"#.as_slice(),
        ] {
            assert!(detect_error(data).is_none(), "{data:?}");
        }
    }
}
