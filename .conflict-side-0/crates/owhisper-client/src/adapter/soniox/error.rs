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
    fn test_numeric_code() {
        let data = br#"{"error_code": 400, "error_message": "Invalid model specified."}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 400);
        assert_eq!(err.message, "Invalid model specified.");
        assert_eq!(err.provider_code, Some("400".to_string()));
        assert_eq!(err.to_ws_close_code(), 4400);
    }

    #[test]
    fn test_503() {
        let data = br#"{"error_code": 503, "error_message": "Cannot continue request (code 1). Please restart the request."}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 503);
        assert!(err.message.contains("Cannot continue request"));
        assert_eq!(err.to_ws_close_code(), 4500);
    }

    #[test]
    fn test_string_code() {
        let data = br#"{"error_code": "INVALID_API_KEY", "error_message": "API key is invalid"}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 500);
        assert_eq!(err.message, "API key is invalid");
        assert_eq!(err.provider_code, Some("INVALID_API_KEY".to_string()));
    }

    #[test]
    fn test_only_message() {
        let data = br#"{"error_message": "Something went wrong"}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 500);
        assert_eq!(err.message, "Something went wrong");
        assert_eq!(err.provider_code, None);
    }

    #[test]
    fn test_only_code() {
        let data = br#"{"error_code": 401}"#;
        let err = detect_error(data).unwrap();
        assert_eq!(err.http_code, 401);
        assert_eq!(err.message, "Unknown error");
        assert_eq!(err.provider_code, Some("401".to_string()));
    }

    #[test]
    fn test_non_error_message() {
        let data = br#"{"tokens": [], "finished": false}"#;
        assert!(detect_error(data).is_none());
    }

    #[test]
    fn test_empty_json() {
        let data = br#"{}"#;
        assert!(detect_error(data).is_none());
    }
}
