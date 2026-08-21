use std::collections::BTreeSet;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const LICENSE_ENV: &str = "ANARLOG_ENTERPRISE_LICENSE";
pub const LICENSE_KEY_ENV: &str = "ANARLOG_ENTERPRISE_LICENSE_KEY";

const TOKEN_VERSION: &str = "v1";
const HMAC_BLOCK_BYTES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LicenseClaims {
    pub customer_id: String,
    #[serde(default)]
    pub workspace_ids: Vec<String>,
    pub not_before: DateTime<Utc>,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub features: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct License {
    pub claims: LicenseClaims,
}

impl License {
    pub fn parse(token: &str, key: &str, now: DateTime<Utc>) -> Result<Self, LicenseError> {
        if key.len() < 32 || key.len() > 512 || key.chars().any(char::is_control) {
            return Err(LicenseError::InvalidKey);
        }
        let (payload, mac) = split_token(token)?;
        let expected = hmac_sha256(key.as_bytes(), payload.as_bytes());
        if !constant_time_eq(&expected, &mac) {
            return Err(LicenseError::InvalidSignature);
        }
        let claims: LicenseClaims = serde_json::from_slice(
            &URL_SAFE_NO_PAD
                .decode(payload)
                .map_err(|_| LicenseError::InvalidToken)?,
        )
        .map_err(|_| LicenseError::InvalidToken)?;
        validate_claims(&claims, now)?;
        Ok(Self { claims })
    }

    pub fn issue(claims: &LicenseClaims, key: &str) -> Result<String, LicenseError> {
        if key.len() < 32 || key.len() > 512 || key.chars().any(char::is_control) {
            return Err(LicenseError::InvalidKey);
        }
        validate_claims(claims, claims.not_before)?;
        let payload = URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(claims).map_err(|_| LicenseError::InvalidToken)?);
        let mac = hmac_sha256(key.as_bytes(), payload.as_bytes());
        Ok(format!(
            "{TOKEN_VERSION}.{payload}.{}",
            URL_SAFE_NO_PAD.encode(mac)
        ))
    }

    pub fn authorizes_workspace(&self, workspace_id: &str) -> bool {
        self.claims.workspace_ids.is_empty()
            || self
                .claims
                .workspace_ids
                .iter()
                .any(|allowed| allowed == workspace_id)
    }

    pub fn has_feature(&self, feature: &str) -> bool {
        self.claims.features.is_empty() || self.claims.features.contains(feature)
    }
}

fn split_token(token: &str) -> Result<(&str, Vec<u8>), LicenseError> {
    let mut parts = token.split('.');
    let version = parts.next().ok_or(LicenseError::InvalidToken)?;
    let payload = parts.next().ok_or(LicenseError::InvalidToken)?;
    let mac = parts.next().ok_or(LicenseError::InvalidToken)?;
    if version != TOKEN_VERSION || parts.next().is_some() || payload.is_empty() || mac.is_empty() {
        return Err(LicenseError::InvalidToken);
    }
    Ok((
        payload,
        URL_SAFE_NO_PAD
            .decode(mac)
            .map_err(|_| LicenseError::InvalidToken)?,
    ))
}

fn validate_claims(claims: &LicenseClaims, now: DateTime<Utc>) -> Result<(), LicenseError> {
    if claims.customer_id.is_empty()
        || claims.customer_id.len() > 128
        || claims.customer_id.chars().any(char::is_control)
    {
        return Err(LicenseError::InvalidToken);
    }
    for workspace_id in &claims.workspace_ids {
        if workspace_id.is_empty()
            || workspace_id.len() > 128
            || !workspace_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
        {
            return Err(LicenseError::InvalidToken);
        }
    }
    if now < claims.not_before {
        return Err(LicenseError::NotYetValid);
    }
    if claims
        .expires_at
        .is_some_and(|expires_at| now >= expires_at)
    {
        return Err(LicenseError::Expired);
    }
    Ok(())
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut key_block = [0u8; HMAC_BLOCK_BYTES];
    if key.len() > HMAC_BLOCK_BYTES {
        let hashed = Sha256::digest(key);
        key_block[..hashed.len()].copy_from_slice(&hashed);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; HMAC_BLOCK_BYTES];
    let mut opad = [0x5cu8; HMAC_BLOCK_BYTES];
    for index in 0..HMAC_BLOCK_BYTES {
        ipad[index] ^= key_block[index];
        opad[index] ^= key_block[index];
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(message);
    let inner_hash = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_hash);
    outer.finalize().into()
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LicenseError {
    #[error("{LICENSE_KEY_ENV} must contain between 32 and 512 bytes")]
    InvalidKey,
    #[error("{LICENSE_ENV} is not a valid Anarlog enterprise license")]
    InvalidToken,
    #[error("{LICENSE_ENV} signature is invalid")]
    InvalidSignature,
    #[error("{LICENSE_ENV} is not yet valid")]
    NotYetValid,
    #[error("{LICENSE_ENV} has expired")]
    Expired,
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "0123456789abcdef0123456789abcdef";

    fn claims() -> LicenseClaims {
        LicenseClaims {
            customer_id: "acme".into(),
            workspace_ids: vec!["workspace-a".into()],
            not_before: DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            expires_at: None,
            features: BTreeSet::from(["capture".into()]),
        }
    }

    #[test]
    fn round_trips_a_perpetual_offline_license() {
        let token = License::issue(&claims(), KEY).unwrap();
        let license = License::parse(
            &token,
            KEY,
            DateTime::parse_from_rfc3339("2026-08-21T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        )
        .unwrap();

        assert!(license.authorizes_workspace("workspace-a"));
        assert!(!license.authorizes_workspace("workspace-b"));
        assert!(license.has_feature("capture"));
        assert!(!license.has_feature("telemetry"));
        assert!(!token.contains(KEY));
    }

    #[test]
    fn rejects_tampered_payloads_without_echoing_the_key() {
        let token = License::issue(&claims(), KEY).unwrap();
        let mut parts = token.split('.');
        let version = parts.next().unwrap();
        let payload = parts.next().unwrap();
        let mac = parts.next().unwrap();
        let mut tampered_payload = payload.as_bytes().to_vec();
        tampered_payload[0] ^= 0x01;
        let tampered = format!(
            "{version}.{}.{mac}",
            URL_SAFE_NO_PAD.encode(tampered_payload)
        );
        let error = License::parse(
            &tampered,
            KEY,
            DateTime::parse_from_rfc3339("2026-08-21T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        )
        .unwrap_err();
        assert_eq!(error, LicenseError::InvalidSignature);
        assert!(!error.to_string().contains(KEY));
    }

    #[test]
    fn empty_workspace_and_feature_sets_authorize_configured_tenants() {
        let mut claims = claims();
        claims.workspace_ids.clear();
        claims.features.clear();
        let token = License::issue(&claims, KEY).unwrap();
        let license = License::parse(
            &token,
            KEY,
            DateTime::parse_from_rfc3339("2026-08-21T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        )
        .unwrap();
        assert!(license.authorizes_workspace("any-workspace"));
        assert!(license.has_feature("zoom"));
    }
}
