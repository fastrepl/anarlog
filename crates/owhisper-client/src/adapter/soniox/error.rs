// https://soniox.com/docs/stt/rt/error-handling

use serde::Deserialize;

use crate::error_detection::ProviderError;

#[derive(Deserialize)]
struct SonioxError<'a> {
    #[serde(borrow)]
    error_code: Option<ErrorCode<'a>>,
    #[serde(borrow)]
    error_message: Option<&'a str>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ErrorCode<'a> {
    Number(u16),
    #[serde(borrow)]
    String(&'a str),
}

impl ErrorCode<'_> {
    fn as_u16(&self) -> u16 {
        match self {
            ErrorCode::Number(n) => *n,
            ErrorCode::String(s) => s.parse().unwrap_or(500),
        }
    }

    fn as_string(&self) -> String {
        match self {
            ErrorCode::Number(n) => n.to_string(),
            ErrorCode::String(s) => s.to_string(),
        }
    }
}

pub fn detect_error(data: &[u8]) -> Option<ProviderError> {
    let text = std::str::from_utf8(data).ok()?;
    let parsed: SonioxError = serde_json::from_str(text).ok()?;

    if parsed.error_code.is_none() && parsed.error_message.is_none() {
        return None;
    }

    let code = parsed
        .error_code
        .as_ref()
        .map(|c| c.as_u16())
        .unwrap_or(500);
    let provider_code = parsed.error_code.as_ref().map(|c| c.as_string());
    let message = parsed.error_message.unwrap_or("Unknown error").to_string();

    let mut error = ProviderError::new(code, message);
    if let Some(pc) = provider_code {
        error = error.with_provider_code(pc);
    }
    Some(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_error_messages() {
        for (data, http_code, provider_code, ws_close_code, message_check) in [
            (
                br#"{"error_code": 400, "error_message": "Invalid model specified."}"#.as_slice(),
                400,
                Some("400"),
                4400,
                "Invalid model specified.",
            ),
            (
                br#"{"error_code": 503, "error_message": "Cannot continue request (code 1). Please restart the request."}"#
                    .as_slice(),
                503,
                Some("503"),
                4500,
                "Cannot continue request",
            ),
            (
                br#"{"error_code": "INVALID_API_KEY", "error_message": "API key is invalid"}"#
                    .as_slice(),
                500,
                Some("INVALID_API_KEY"),
                4500,
                "API key is invalid",
            ),
            (
                br#"{"error_message": "Something went wrong"}"#.as_slice(),
                500,
                None,
                4500,
                "Something went wrong",
            ),
            (
                br#"{"error_code": 401}"#.as_slice(),
                401,
                Some("401"),
                4401,
                "Unknown error",
            ),
        ] {
            let err = detect_error(data).unwrap();
            assert_eq!(err.http_code, http_code, "{data:?}");
            assert!(
                err.message == message_check || err.message.contains(message_check),
                "{data:?}"
            );
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
            br#"{"tokens": [], "finished": false}"#.as_slice(),
            br#"{}"#.as_slice(),
        ] {
            assert!(detect_error(data).is_none(), "{data:?}");
        }
    }
}
