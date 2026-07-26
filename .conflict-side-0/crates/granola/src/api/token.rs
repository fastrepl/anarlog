use crate::error::{Error, Result};
use serde::Deserialize;

#[derive(Deserialize)]
struct SupabaseWrapper {
    workos_tokens: String,
}

#[derive(Deserialize)]
struct WorkosTokens {
    access_token: String,
}

pub fn extract_access_token(content: &[u8]) -> Result<String> {
    let wrapper: SupabaseWrapper =
        serde_json::from_slice(content).map_err(Error::SupabaseJsonParse)?;

    let tokens: WorkosTokens =
        serde_json::from_str(&wrapper.workos_tokens).map_err(Error::TokenJsonParse)?;

    let access_token = tokens.access_token.trim().to_string();
    if access_token.is_empty() {
        return Err(Error::AccessTokenNotFound);
    }

    Ok(access_token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_access_token_success() {
        let json = r#"{"workos_tokens": "{\"access_token\":\"test_token_123\"}"}"#;
        let token = extract_access_token(json.as_bytes()).unwrap();
        assert_eq!(token, "test_token_123");
    }

    #[test]
    fn test_extract_access_token_empty() {
        let json = r#"{"workos_tokens": "{\"access_token\":\"   \"}"}"#;
        let result = extract_access_token(json.as_bytes());
        assert!(matches!(result, Err(Error::AccessTokenNotFound)));
    }

    #[test]
    fn test_extract_access_token_invalid_wrapper() {
        let json = r#"{"invalid": "json"}"#;
        let result = extract_access_token(json.as_bytes());
        assert!(matches!(result, Err(Error::SupabaseJsonParse(_))));
    }
}
