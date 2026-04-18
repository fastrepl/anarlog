use base64::{Engine, engine::general_purpose::STANDARD, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use ssh_key::{HashAlg, LineEnding, PrivateKey};

pub const NAMESPACE_API: &str = "v0@exe.dev";

pub fn namespace_vm(vm_name: &str) -> String {
    format!("v0@{vm_name}.exe.xyz")
}

const MAX_TOKEN_BYTES: usize = 8 * 1024;
const MIN_TIMESTAMP: i64 = 946_684_800;
const MAX_TIMESTAMP: i64 = 4_102_444_800;
const ALLOWED_TOP_LEVEL: &[&str] = &["exp", "nbf", "cmds", "ctx"];

#[derive(Clone)]
pub struct Exe0Token(String);

impl std::fmt::Debug for Exe0Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Exe0Token").field(&"<redacted>").finish()
    }
}

impl Exe0Token {
    pub fn mint(
        permissions: &serde_json::Value,
        signing_key_pem: &str,
        namespace: &str,
    ) -> Result<Self, crate::Error> {
        let obj = permissions
            .as_object()
            .ok_or(crate::Error::InvalidPermissions(
                "permissions must be a JSON object",
            ))?;

        for key in obj.keys() {
            if !ALLOWED_TOP_LEVEL.contains(&key.as_str()) {
                return Err(crate::Error::InvalidPermissions(
                    "unknown top-level field; allowed: exp, nbf, cmds, ctx",
                ));
            }
        }

        for field in ["exp", "nbf"] {
            if let Some(v) = obj.get(field) {
                let n = v.as_i64().ok_or(crate::Error::InvalidPermissions(
                    "exp and nbf must be integers",
                ))?;
                if !(MIN_TIMESTAMP..=MAX_TIMESTAMP).contains(&n) {
                    return Err(crate::Error::InvalidPermissions(
                        "exp/nbf must be between 2000-01-01 and 2100-01-01",
                    ));
                }
            }
        }

        let payload_bytes = serde_json::to_vec(permissions)?;

        if payload_bytes.is_empty() || payload_bytes.len() > MAX_TOKEN_BYTES {
            return Err(crate::Error::InvalidPermissions(
                "serialized permissions too large (>8KiB)",
            ));
        }
        if payload_bytes
            .iter()
            .any(|b| *b == 0 || *b == b'\n' || *b == b'\r')
        {
            return Err(crate::Error::InvalidPermissions(
                "serialized permissions may not contain null or newline bytes",
            ));
        }

        let payload_b64 = URL_SAFE_NO_PAD.encode(&payload_bytes);

        let key = PrivateKey::from_openssh(signing_key_pem)
            .map_err(|e| crate::Error::InvalidSigningKey(e.to_string()))?;

        let sig = key
            .sign(namespace, HashAlg::Sha512, &payload_bytes)
            .map_err(|e| crate::Error::Signing(e.to_string()))?;

        let pem = sig
            .to_pem(LineEnding::LF)
            .map_err(|e| crate::Error::Signing(e.to_string()))?;

        let sig_bytes = decode_pem_body(&pem)
            .ok_or(crate::Error::Signing("malformed SSH signature PEM".into()))?;
        let sig_b64 = URL_SAFE_NO_PAD.encode(&sig_bytes);

        Ok(Self(format!("exe0.{payload_b64}.{sig_b64}")))
    }

    pub fn mint_for_api(
        permissions: &serde_json::Value,
        signing_key_pem: &str,
    ) -> Result<Self, crate::Error> {
        Self::mint(permissions, signing_key_pem, NAMESPACE_API)
    }

    pub fn mint_for_vm(
        vm_name: &str,
        permissions: &serde_json::Value,
        signing_key_pem: &str,
    ) -> Result<Self, crate::Error> {
        Self::mint(permissions, signing_key_pem, &namespace_vm(vm_name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

/// Short, opaque token that acts as a handle to an `exe0` token.
///
/// Obtained via `ExedevClient::exe0_to_exe1`; the server validates the exe0 on
/// every request, so revoking the exe0 (or its signing key) revokes the exe1.
#[derive(Clone)]
pub struct Exe1Token(String);

impl std::fmt::Debug for Exe1Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Exe1Token").field(&"<redacted>").finish()
    }
}

impl Exe1Token {
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

/// Typed permissions shared across helpers. `Default` produces `{}` (server-defaults).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Permissions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nbf: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmds: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ctx: Option<serde_json::Value>,
}

impl Permissions {
    pub fn with_exp(mut self, exp: i64) -> Self {
        self.exp = Some(exp);
        self
    }

    pub fn with_nbf(mut self, nbf: i64) -> Self {
        self.nbf = Some(nbf);
        self
    }

    pub fn with_cmds<I, S>(mut self, cmds: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.cmds = Some(cmds.into_iter().map(Into::into).collect());
        self
    }

    pub fn with_ctx<C: Serialize>(mut self, ctx: &C) -> Result<Self, crate::Error> {
        self.ctx = Some(serde_json::to_value(ctx)?);
        Ok(self)
    }

    pub fn to_json(&self) -> Result<serde_json::Value, crate::Error> {
        Ok(serde_json::to_value(self)?)
    }
}

fn decode_pem_body(pem: &str) -> Option<Vec<u8>> {
    let body: String = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect();
    STANDARD.decode(body.as_bytes()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_core::OsRng;
    use ssh_key::Algorithm;

    fn gen_key() -> String {
        let key = PrivateKey::random(&mut OsRng, Algorithm::Ed25519).unwrap();
        key.to_openssh(LineEnding::LF).unwrap().to_string()
    }

    #[test]
    fn mint_assembles_three_parts() {
        let pem = gen_key();
        let perms = serde_json::json!({ "exp": 1_922_918_400i64 });
        let token = Exe0Token::mint(&perms, &pem, NAMESPACE_API).unwrap();
        let parts: Vec<&str> = token.as_str().split('.').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "exe0");
        let decoded = URL_SAFE_NO_PAD.decode(parts[1]).unwrap();
        let roundtrip: serde_json::Value = serde_json::from_slice(&decoded).unwrap();
        assert_eq!(roundtrip, perms);
    }

    #[test]
    fn empty_permissions_ok() {
        let pem = gen_key();
        let token = Exe0Token::mint(&serde_json::json!({}), &pem, NAMESPACE_API).unwrap();
        assert!(token.as_str().starts_with("exe0."));
    }

    #[test]
    fn rejects_unknown_field() {
        let pem = gen_key();
        let perms = serde_json::json!({ "evil": true });
        let err = Exe0Token::mint(&perms, &pem, NAMESPACE_API).unwrap_err();
        assert!(matches!(err, crate::Error::InvalidPermissions(_)));
    }

    #[test]
    fn rejects_non_integer_exp() {
        let pem = gen_key();
        let perms = serde_json::json!({ "exp": 1.5 });
        let err = Exe0Token::mint(&perms, &pem, NAMESPACE_API).unwrap_err();
        assert!(matches!(err, crate::Error::InvalidPermissions(_)));
    }

    #[test]
    fn rejects_out_of_range_exp() {
        let pem = gen_key();
        let perms = serde_json::json!({ "exp": 1i64 });
        let err = Exe0Token::mint(&perms, &pem, NAMESPACE_API).unwrap_err();
        assert!(matches!(err, crate::Error::InvalidPermissions(_)));
    }

    #[test]
    fn signature_verifies() {
        let pem = gen_key();
        let key = PrivateKey::from_openssh(&pem).unwrap();
        let perms = serde_json::json!({ "exp": 1_922_918_400i64 });

        let payload_bytes = serde_json::to_vec(&perms).unwrap();
        let sig = key
            .sign(NAMESPACE_API, HashAlg::Sha512, &payload_bytes)
            .unwrap();
        key.public_key()
            .verify(NAMESPACE_API, &payload_bytes, &sig)
            .unwrap();

        let token = Exe0Token::mint(&perms, &pem, NAMESPACE_API).unwrap();
        let parts: Vec<&str> = token.as_str().split('.').collect();
        let payload = URL_SAFE_NO_PAD.decode(parts[1]).unwrap();
        assert_eq!(payload, payload_bytes);
        let _sig_bytes = URL_SAFE_NO_PAD.decode(parts[2]).unwrap();
    }

    #[test]
    fn namespace_vm_format() {
        assert_eq!(namespace_vm("claw-ab12"), "v0@claw-ab12.exe.xyz");
    }

    #[test]
    fn mint_for_vm_uses_vm_namespace() {
        let pem = gen_key();
        let perms = serde_json::json!({ "exp": 1_922_918_400i64 });

        let api_token = Exe0Token::mint_for_api(&perms, &pem).unwrap();
        let vm_token = Exe0Token::mint_for_vm("claw-abc", &perms, &pem).unwrap();

        let api_parts: Vec<&str> = api_token.as_str().split('.').collect();
        let vm_parts: Vec<&str> = vm_token.as_str().split('.').collect();

        // same payload, different signature (namespace differs)
        assert_eq!(api_parts[1], vm_parts[1]);
        assert_ne!(api_parts[2], vm_parts[2]);
    }

    #[test]
    fn permissions_builder() {
        let p = Permissions::default()
            .with_exp(1_922_918_400)
            .with_cmds(["ls", "new"]);
        let v = p.to_json().unwrap();
        assert_eq!(v["exp"], 1_922_918_400);
        assert_eq!(v["cmds"], serde_json::json!(["ls", "new"]));
        assert!(v.get("nbf").is_none());
    }
}
