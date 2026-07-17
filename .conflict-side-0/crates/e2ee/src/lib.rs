#![forbid(unsafe_code)]

mod blob;

pub use blob::{
    ATTACHMENT_BLOB_CHUNK_SIZE, ATTACHMENT_BLOB_MAX_PLAINTEXT_BYTES,
    AttachmentBlobCiphertextMetadata, AttachmentBlobContext, AttachmentBlobError,
    AttachmentBlobMetadata, AttachmentBlobPlaintextMetadata, AttachmentBlobResult,
};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

const RECOVERY_KEY_PREFIX: &str = "anarlog-e2ee-v1:";
const RECOVERY_KEY_ID_DOMAIN: &[u8] = b"anarlog-e2ee-recovery-key-id-v1";
const WORKSPACE_KEY_SALT: &[u8] = b"anarlog-e2ee-workspace-key-v1";
const FIELD_ID_DOMAIN: &[u8] = b"anarlog-e2ee-field-id-v1";
const VALUE_TAG_DOMAIN: &[u8] = b"anarlog-e2ee-value-tag-v1";
const PAYLOAD_AAD_DOMAIN: &[u8] = b"anarlog-e2ee-payload-v1";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the recovery key is invalid")]
    InvalidRecoveryKey,
    #[error("the workspace ID is invalid")]
    InvalidWorkspaceId,
    #[error("the encrypted payload is invalid")]
    InvalidPayload,
    #[error("the encrypted payload was created with an unavailable key")]
    UnknownKey,
    #[error("the encrypted payload failed authentication")]
    AuthenticationFailed,
    #[error("cryptographic randomness is unavailable")]
    RandomnessUnavailable,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct RecoveryKey([u8; 32]);

impl Clone for RecoveryKey {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl RecoveryKey {
    pub fn generate() -> Result<Self> {
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| Error::RandomnessUnavailable)?;
        Ok(Self(bytes))
    }

    pub fn parse(value: &str) -> Result<Self> {
        let encoded = value
            .trim()
            .strip_prefix(RECOVERY_KEY_PREFIX)
            .ok_or(Error::InvalidRecoveryKey)?;
        let decoded = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| Error::InvalidRecoveryKey)?;
        let bytes: [u8; 32] = decoded.try_into().map_err(|_| Error::InvalidRecoveryKey)?;
        Ok(Self(bytes))
    }

    pub fn expose_code(&self) -> Zeroizing<String> {
        Zeroizing::new(format!(
            "{RECOVERY_KEY_PREFIX}{}",
            URL_SAFE_NO_PAD.encode(self.0)
        ))
    }

    pub fn key_id(&self) -> String {
        let digest = Sha256::digest([RECOVERY_KEY_ID_DOMAIN, self.0.as_slice()].concat());
        URL_SAFE_NO_PAD.encode(&digest[..16])
    }

    pub fn workspace_key(&self, workspace_id: &str) -> Result<WorkspaceKey> {
        let workspace_id = workspace_id.trim();
        if workspace_id.is_empty() || workspace_id.len() > 256 {
            return Err(Error::InvalidWorkspaceId);
        }

        let hkdf = Hkdf::<Sha256>::new(Some(WORKSPACE_KEY_SALT), &self.0);
        let mut bytes = [0_u8; 32];
        hkdf.expand(workspace_id.as_bytes(), &mut bytes)
            .map_err(|_| Error::InvalidWorkspaceId)?;
        Ok(WorkspaceKey::new(bytes))
    }
}

pub struct WorkspaceKey {
    bytes: Zeroizing<[u8; 32]>,
    key_id: String,
}

impl Clone for WorkspaceKey {
    fn clone(&self) -> Self {
        Self {
            bytes: Zeroizing::new(*self.bytes),
            key_id: self.key_id.clone(),
        }
    }
}

impl WorkspaceKey {
    fn new(bytes: [u8; 32]) -> Self {
        let digest = Sha256::digest([b"anarlog-e2ee-key-id-v1".as_slice(), &bytes].concat());
        let key_id = URL_SAFE_NO_PAD.encode(&digest[..16]);
        Self {
            bytes: Zeroizing::new(bytes),
            key_id,
        }
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn blind_field_id(&self, table: &str, row_id: &str, field: &str) -> String {
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(self.bytes.as_slice())
            .expect("HMAC accepts 32-byte keys");
        update_context(&mut mac, FIELD_ID_DOMAIN, table, row_id, field);
        URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    }

    pub fn value_tag(
        &self,
        table: &str,
        row_id: &str,
        field: &str,
        deleted: bool,
        value: &Value,
    ) -> String {
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(self.bytes.as_slice())
            .expect("HMAC accepts 32-byte keys");
        update_context(&mut mac, VALUE_TAG_DOMAIN, table, row_id, field);
        mac.update(&[u8::from(deleted)]);
        mac.update(&serde_json::to_vec(value).expect("JSON values serialize"));
        URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    }

    pub fn seal_field(
        &self,
        workspace_id: &str,
        table: &str,
        row_id: &str,
        field: &str,
        revision: u64,
        deleted: bool,
        value: Value,
    ) -> Result<SealedField> {
        let record_id = self.blind_field_id(table, row_id, field);
        let plaintext = serde_json::to_vec(&ProtectedField {
            table,
            row_id,
            field,
            revision,
            deleted,
            value,
        })
        .map_err(|_| Error::InvalidPayload)?;
        let mut nonce = [0_u8; 24];
        getrandom::fill(&mut nonce).map_err(|_| Error::RandomnessUnavailable)?;
        let aad = payload_aad(workspace_id, &record_id, &self.key_id);
        let cipher = XChaCha20Poly1305::new_from_slice(self.bytes.as_slice())
            .expect("XChaCha20Poly1305 accepts 32-byte keys");
        let nonce = XNonce::from(nonce);
        let ciphertext = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: &plaintext,
                    aad: &aad,
                },
            )
            .map_err(|_| Error::AuthenticationFailed)?;
        let envelope = Envelope {
            version: 1,
            key_id: self.key_id.clone(),
            nonce: URL_SAFE_NO_PAD.encode(nonce),
            ciphertext: URL_SAFE_NO_PAD.encode(ciphertext),
        };
        let payload = serde_json::to_string(&envelope).map_err(|_| Error::InvalidPayload)?;

        Ok(SealedField { record_id, payload })
    }

    pub fn open_field(
        &self,
        workspace_id: &str,
        record_id: &str,
        payload: &str,
    ) -> Result<OpenedField> {
        let envelope: Envelope =
            serde_json::from_str(payload).map_err(|_| Error::InvalidPayload)?;
        if envelope.version != 1 {
            return Err(Error::InvalidPayload);
        }
        if envelope.key_id != self.key_id {
            return Err(Error::UnknownKey);
        }
        let nonce = URL_SAFE_NO_PAD
            .decode(envelope.nonce)
            .map_err(|_| Error::InvalidPayload)?;
        let nonce: [u8; 24] = nonce.try_into().map_err(|_| Error::InvalidPayload)?;
        let ciphertext = URL_SAFE_NO_PAD
            .decode(envelope.ciphertext)
            .map_err(|_| Error::InvalidPayload)?;
        let aad = payload_aad(workspace_id, record_id, &self.key_id);
        let cipher = XChaCha20Poly1305::new_from_slice(self.bytes.as_slice())
            .expect("XChaCha20Poly1305 accepts 32-byte keys");
        let nonce = XNonce::from(nonce);
        let plaintext = cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: &ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|_| Error::AuthenticationFailed)?;
        let field: OwnedProtectedField =
            serde_json::from_slice(&plaintext).map_err(|_| Error::InvalidPayload)?;
        if self.blind_field_id(&field.table, &field.row_id, &field.field) != record_id {
            return Err(Error::AuthenticationFailed);
        }

        Ok(OpenedField {
            table: field.table,
            row_id: field.row_id,
            field: field.field,
            revision: field.revision,
            deleted: field.deleted,
            value: field.value,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SealedField {
    pub record_id: String,
    pub payload: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenedField {
    pub table: String,
    pub row_id: String,
    pub field: String,
    pub revision: u64,
    pub deleted: bool,
    pub value: Value,
}

pub fn payload_hash(payload: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(payload.as_bytes()))
}

#[derive(Serialize)]
struct ProtectedField<'a> {
    table: &'a str,
    row_id: &'a str,
    field: &'a str,
    revision: u64,
    deleted: bool,
    value: Value,
}

#[derive(Deserialize)]
struct OwnedProtectedField {
    table: String,
    row_id: String,
    field: String,
    revision: u64,
    deleted: bool,
    value: Value,
}

#[derive(Deserialize, Serialize)]
struct Envelope {
    version: u8,
    key_id: String,
    nonce: String,
    ciphertext: String,
}

fn update_context(mac: &mut Hmac<Sha256>, domain: &[u8], table: &str, row_id: &str, field: &str) {
    for value in [
        domain,
        table.as_bytes(),
        row_id.as_bytes(),
        field.as_bytes(),
    ] {
        mac.update(&(value.len() as u64).to_be_bytes());
        mac.update(value);
    }
}

fn payload_aad(workspace_id: &str, record_id: &str, key_id: &str) -> Vec<u8> {
    let mut aad = Vec::new();
    for value in [
        PAYLOAD_AAD_DOMAIN,
        workspace_id.as_bytes(),
        record_id.as_bytes(),
        key_id.as_bytes(),
    ] {
        aad.extend_from_slice(&(value.len() as u64).to_be_bytes());
        aad.extend_from_slice(value);
    }
    aad
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn recovery_key() -> RecoveryKey {
        RecoveryKey([7; 32])
    }

    #[test]
    fn recovery_code_round_trips() {
        let key = recovery_key();
        let code = key.expose_code();
        let parsed = RecoveryKey::parse(&code).unwrap();

        assert_eq!(parsed.expose_code().as_str(), code.as_str());
        assert_eq!(parsed.key_id(), key.key_id());
        assert_eq!(key.key_id().len(), 22);
        assert!(RecoveryKey::parse("not-a-recovery-key").is_err());
    }

    #[test]
    fn workspace_keys_are_stable_and_scoped() {
        let key = recovery_key();
        let first = key.workspace_key("workspace-a").unwrap();
        let same = key.workspace_key("workspace-a").unwrap();
        let other = key.workspace_key("workspace-b").unwrap();

        assert_eq!(first.key_id(), same.key_id());
        assert_ne!(first.key_id(), other.key_id());
    }

    #[test]
    fn fields_round_trip_with_blinded_identifiers() {
        let key = recovery_key().workspace_key("workspace-a").unwrap();
        let sealed = key
            .seal_field(
                "workspace-a",
                "session_documents",
                "document-1",
                "body",
                1,
                false,
                json!({ "type": "doc", "content": [] }),
            )
            .unwrap();

        assert!(!sealed.payload.contains("session_documents"));
        assert!(!sealed.payload.contains("document-1"));
        assert!(!sealed.payload.contains("content"));
        let opened = key
            .open_field("workspace-a", &sealed.record_id, &sealed.payload)
            .unwrap();

        assert_eq!(opened.table, "session_documents");
        assert_eq!(opened.row_id, "document-1");
        assert_eq!(opened.field, "body");
        assert_eq!(opened.revision, 1);
        assert_eq!(opened.value, json!({ "type": "doc", "content": [] }));
    }

    #[test]
    fn tampering_and_context_swaps_fail_closed() {
        let key = recovery_key().workspace_key("workspace-a").unwrap();
        let sealed = key
            .seal_field(
                "workspace-a",
                "sessions",
                "session-1",
                "title",
                1,
                false,
                json!("Planning"),
            )
            .unwrap();

        assert!(
            key.open_field("workspace-b", &sealed.record_id, &sealed.payload)
                .is_err()
        );
        assert!(
            key.open_field("workspace-a", "different-record", &sealed.payload)
                .is_err()
        );

        let mut envelope: Value = serde_json::from_str(&sealed.payload).unwrap();
        envelope["ciphertext"] = json!(URL_SAFE_NO_PAD.encode([0_u8; 32]));
        assert!(
            key.open_field("workspace-a", &sealed.record_id, &envelope.to_string(),)
                .is_err()
        );
    }

    #[test]
    fn tags_do_not_expose_plaintext_and_change_with_values() {
        let key = recovery_key().workspace_key("workspace-a").unwrap();
        let first = key.value_tag("sessions", "session-1", "title", false, &json!("A"));
        let same = key.value_tag("sessions", "session-1", "title", false, &json!("A"));
        let changed = key.value_tag("sessions", "session-1", "title", false, &json!("B"));

        assert_eq!(first, same);
        assert_ne!(first, changed);
        assert_ne!(first, "A");
    }
}
