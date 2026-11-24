#![forbid(unsafe_code)]

mod blob;
mod member;
mod snapshot;

pub use blob::{
    ATTACHMENT_BLOB_CHUNK_SIZE, ATTACHMENT_BLOB_MAX_PLAINTEXT_BYTES,
    AttachmentBlobCiphertextMetadata, AttachmentBlobContext, AttachmentBlobError,
    AttachmentBlobMetadata, AttachmentBlobPlaintextMetadata, AttachmentBlobResult,
};
pub use member::{
    MemberIdentityKey, WorkspaceKeyGrant, WorkspaceKeyring, seal_workspace_key_for_member,
};
pub use snapshot::{
    SNAPSHOT_FORMAT_VERSION, SNAPSHOT_MAGIC, SnapshotRecord, WorkspaceSnapshotPayload,
    open_snapshot, seal_snapshot,
};

use std::io::{Read, Write};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use flate2::{Compression, read::DeflateDecoder, write::DeflateEncoder};
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
const PAYLOAD_AAD_V2_DOMAIN: &[u8] = b"anarlog-e2ee-payload-v2";
const COMPRESS_FIELD_MIN_BYTES: usize = 256;
const MAX_DECOMPRESSED_FIELD_BYTES: usize = 16 * 1024 * 1024;
const DEVICE_ENROLLMENT_KEY_PREFIX: &str = "anarlog-device-key-v1:";
const DEVICE_ENROLLMENT_KDF_SALT: &[u8] = b"anarlog-device-enrollment-kdf-v1";
const DEVICE_ENROLLMENT_AAD_DOMAIN: &[u8] = b"anarlog-device-enrollment-v1";

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
    #[error("the E2EE writer ID is invalid")]
    InvalidWriterId,
    #[error("the encrypted payload failed authentication")]
    AuthenticationFailed,
    #[error("cryptographic randomness is unavailable")]
    RandomnessUnavailable,
    #[error("the device enrollment key is invalid")]
    InvalidDeviceEnrollmentKey,
    #[error("the device enrollment package is invalid")]
    InvalidDeviceEnrollmentPackage,
    #[error("the member identity key is invalid")]
    InvalidMemberIdentityKey,
    #[error("the workspace key grant is invalid")]
    InvalidWorkspaceKeyGrant,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct DeviceEnrollmentKey([u8; 32]);

impl DeviceEnrollmentKey {
    pub fn generate() -> Result<Self> {
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| Error::RandomnessUnavailable)?;
        Ok(Self(bytes))
    }

    pub fn parse(value: &str) -> Result<Self> {
        let encoded = value
            .trim()
            .strip_prefix(DEVICE_ENROLLMENT_KEY_PREFIX)
            .ok_or(Error::InvalidDeviceEnrollmentKey)?;
        let decoded = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| Error::InvalidDeviceEnrollmentKey)?;
        Ok(Self(
            decoded
                .try_into()
                .map_err(|_| Error::InvalidDeviceEnrollmentKey)?,
        ))
    }

    pub fn expose_code(&self) -> Zeroizing<String> {
        Zeroizing::new(format!(
            "{DEVICE_ENROLLMENT_KEY_PREFIX}{}",
            URL_SAFE_NO_PAD.encode(self.0)
        ))
    }

    pub fn public_key(&self) -> String {
        let secret = x25519_dalek::StaticSecret::from(self.0);
        URL_SAFE_NO_PAD.encode(x25519_dalek::PublicKey::from(&secret).as_bytes())
    }

    pub fn open_recovery_key(
        &self,
        account_user_id: &str,
        request_id: &str,
        package: &DeviceEnrollmentPackage,
    ) -> Result<RecoveryKey> {
        let ephemeral_public = decode_enrollment_bytes(&package.ephemeral_public_key)?;
        let nonce: [u8; 24] = URL_SAFE_NO_PAD
            .decode(&package.nonce)
            .map_err(|_| Error::InvalidDeviceEnrollmentPackage)?
            .try_into()
            .map_err(|_| Error::InvalidDeviceEnrollmentPackage)?;
        let ciphertext = URL_SAFE_NO_PAD
            .decode(&package.ciphertext)
            .map_err(|_| Error::InvalidDeviceEnrollmentPackage)?;
        let shared = x25519_dalek::StaticSecret::from(self.0)
            .diffie_hellman(&x25519_dalek::PublicKey::from(ephemeral_public));
        if !shared.was_contributory() {
            return Err(Error::InvalidDeviceEnrollmentPackage);
        }
        let cipher = enrollment_cipher(shared.as_bytes(), account_user_id, request_id)?;
        let nonce = XNonce::from(nonce);
        let plaintext = cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: &ciphertext,
                    aad: &enrollment_aad(account_user_id, request_id),
                },
            )
            .map_err(|_| Error::AuthenticationFailed)?;
        let code =
            String::from_utf8(plaintext).map_err(|_| Error::InvalidDeviceEnrollmentPackage)?;
        RecoveryKey::parse(&code)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeviceEnrollmentPackage {
    pub ephemeral_public_key: String,
    pub nonce: String,
    pub ciphertext: String,
}

pub fn seal_recovery_key_for_device(
    recovery_key: &RecoveryKey,
    recipient_public_key: &str,
    account_user_id: &str,
    request_id: &str,
) -> Result<DeviceEnrollmentPackage> {
    let recipient_public = decode_enrollment_bytes(recipient_public_key)?;
    let ephemeral_secret = DeviceEnrollmentKey::generate()?;
    let ephemeral_static = x25519_dalek::StaticSecret::from(ephemeral_secret.0);
    let ephemeral_public = x25519_dalek::PublicKey::from(&ephemeral_static);
    let shared = ephemeral_static.diffie_hellman(&x25519_dalek::PublicKey::from(recipient_public));
    if !shared.was_contributory() {
        return Err(Error::InvalidDeviceEnrollmentKey);
    }
    let cipher = enrollment_cipher(shared.as_bytes(), account_user_id, request_id)?;
    let mut nonce = [0_u8; 24];
    getrandom::fill(&mut nonce).map_err(|_| Error::RandomnessUnavailable)?;
    let recovery_code = recovery_key.expose_code();
    let nonce_value = XNonce::from(nonce);
    let ciphertext = cipher
        .encrypt(
            &nonce_value,
            Payload {
                msg: recovery_code.as_bytes(),
                aad: &enrollment_aad(account_user_id, request_id),
            },
        )
        .map_err(|_| Error::InvalidDeviceEnrollmentPackage)?;
    Ok(DeviceEnrollmentPackage {
        ephemeral_public_key: URL_SAFE_NO_PAD.encode(ephemeral_public.as_bytes()),
        nonce: URL_SAFE_NO_PAD.encode(nonce),
        ciphertext: URL_SAFE_NO_PAD.encode(ciphertext),
    })
}

fn decode_enrollment_bytes(value: &str) -> Result<[u8; 32]> {
    URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| Error::InvalidDeviceEnrollmentKey)?
        .try_into()
        .map_err(|_| Error::InvalidDeviceEnrollmentKey)
}

fn enrollment_cipher(
    shared_secret: &[u8; 32],
    account_user_id: &str,
    request_id: &str,
) -> Result<XChaCha20Poly1305> {
    if account_user_id.trim().is_empty() || request_id.trim().is_empty() {
        return Err(Error::InvalidDeviceEnrollmentPackage);
    }
    let hkdf = Hkdf::<Sha256>::new(Some(DEVICE_ENROLLMENT_KDF_SALT), shared_secret);
    let mut key = Zeroizing::new([0_u8; 32]);
    hkdf.expand(&enrollment_aad(account_user_id, request_id), key.as_mut())
        .map_err(|_| Error::InvalidDeviceEnrollmentPackage)?;
    Ok(XChaCha20Poly1305::new_from_slice(key.as_ref())
        .expect("XChaCha20Poly1305 accepts 32-byte keys"))
}

fn enrollment_aad(account_user_id: &str, request_id: &str) -> Vec<u8> {
    [
        DEVICE_ENROLLMENT_AAD_DOMAIN,
        account_user_id.as_bytes(),
        request_id.as_bytes(),
    ]
    .join(&0)
}

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

    pub(crate) fn expose_bytes(&self) -> &[u8; 32] {
        &self.0
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
    /// Mints a random shared-workspace key.
    ///
    /// Unlike [`RecoveryKey::workspace_key`], this is tied to no member, so it
    /// can be handed to several of them and rotated without anyone's recovery
    /// key changing.
    pub fn generate() -> Result<Self> {
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| Error::RandomnessUnavailable)?;
        let key = Self::new(bytes);
        bytes.zeroize();
        Ok(key)
    }

    pub(crate) fn from_bytes(bytes: [u8; 32]) -> Self {
        Self::new(bytes)
    }

    pub(crate) fn expose_bytes(&self) -> &Zeroizing<[u8; 32]> {
        &self.bytes
    }

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

    #[allow(clippy::too_many_arguments)]
    pub fn seal_field(
        &self,
        workspace_id: &str,
        table: &str,
        row_id: &str,
        field: &str,
        writer_id: &str,
        revision: u64,
        deleted: bool,
        value: Value,
    ) -> Result<SealedField> {
        self.seal_field_at(
            workspace_id,
            table,
            row_id,
            field,
            writer_id,
            revision,
            deleted,
            None,
            value,
        )
    }

    /// Seals a field that also records when the local write behind it happened,
    /// so replicas can order concurrent edits by edit time instead of by which
    /// device published first. Older readers ignore the extra field.
    #[allow(clippy::too_many_arguments)]
    pub fn seal_field_at(
        &self,
        workspace_id: &str,
        table: &str,
        row_id: &str,
        field: &str,
        writer_id: &str,
        revision: u64,
        deleted: bool,
        edited_at_ms: Option<u64>,
        value: Value,
    ) -> Result<SealedField> {
        self.seal_field_internal(
            workspace_id,
            table,
            row_id,
            field,
            writer_id,
            revision,
            deleted,
            edited_at_ms,
            value,
            false,
        )
    }

    /// Seals a field using deflate compression + v2 AEAD framing when the encoded
    /// plaintext benefits from compression; small payloads remain v1 so older readers
    /// can continue reading unbloated fields.
    #[allow(clippy::too_many_arguments)]
    pub fn seal_field_v2_at(
        &self,
        workspace_id: &str,
        table: &str,
        row_id: &str,
        field: &str,
        writer_id: &str,
        revision: u64,
        deleted: bool,
        edited_at_ms: Option<u64>,
        value: Value,
    ) -> Result<SealedField> {
        self.seal_field_internal(
            workspace_id,
            table,
            row_id,
            field,
            writer_id,
            revision,
            deleted,
            edited_at_ms,
            value,
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn seal_field_internal(
        &self,
        workspace_id: &str,
        table: &str,
        row_id: &str,
        field: &str,
        writer_id: &str,
        revision: u64,
        deleted: bool,
        edited_at_ms: Option<u64>,
        value: Value,
        allow_compression: bool,
    ) -> Result<SealedField> {
        if !is_valid_writer_id(writer_id) {
            return Err(Error::InvalidWriterId);
        }
        let record_id = self.blind_field_id(table, row_id, field);
        let plaintext = serde_json::to_vec(&ProtectedField {
            table,
            row_id,
            field,
            writer_id,
            revision,
            deleted,
            edited_at_ms,
            value,
        })
        .map_err(|_| Error::InvalidPayload)?;
        let (version, msg): (u8, Vec<u8>) =
            if allow_compression && plaintext.len() >= COMPRESS_FIELD_MIN_BYTES {
                let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
                encoder
                    .write_all(&plaintext)
                    .map_err(|_| Error::InvalidPayload)?;
                let compressed = encoder.finish().map_err(|_| Error::InvalidPayload)?;
                if compressed.len() < plaintext.len() {
                    (2, compressed)
                } else {
                    (1, plaintext)
                }
            } else {
                (1, plaintext)
            };
        let mut nonce = [0_u8; 24];
        getrandom::fill(&mut nonce).map_err(|_| Error::RandomnessUnavailable)?;
        let aad = payload_aad_for_version(workspace_id, &record_id, &self.key_id, version);
        let cipher = XChaCha20Poly1305::new_from_slice(self.bytes.as_slice())
            .expect("XChaCha20Poly1305 accepts 32-byte keys");
        let nonce = XNonce::from(nonce);
        let ciphertext = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: &msg,
                    aad: &aad,
                },
            )
            .map_err(|_| Error::AuthenticationFailed)?;
        let envelope = Envelope {
            version,
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
        if envelope.version != 1 && envelope.version != 2 {
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
        let aad = payload_aad_for_version(workspace_id, record_id, &self.key_id, envelope.version);
        let cipher = XChaCha20Poly1305::new_from_slice(self.bytes.as_slice())
            .expect("XChaCha20Poly1305 accepts 32-byte keys");
        let nonce = XNonce::from(nonce);
        let decrypted = cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: &ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|_| Error::AuthenticationFailed)?;
        let plaintext = if envelope.version == 2 {
            decompress_bounded(&decrypted, MAX_DECOMPRESSED_FIELD_BYTES)?
        } else {
            decrypted
        };
        let field: OwnedProtectedField =
            serde_json::from_slice(&plaintext).map_err(|_| Error::InvalidPayload)?;
        if !field.writer_id.is_empty() && !is_valid_writer_id(&field.writer_id) {
            return Err(Error::InvalidPayload);
        }
        if self.blind_field_id(&field.table, &field.row_id, &field.field) != record_id {
            return Err(Error::AuthenticationFailed);
        }

        Ok(OpenedField {
            key_id: self.key_id.clone(),
            table: field.table,
            row_id: field.row_id,
            field: field.field,
            writer_id: field.writer_id,
            revision: field.revision,
            deleted: field.deleted,
            edited_at_ms: field.edited_at_ms,
            value: field.value,
        })
    }
}

pub(crate) fn decompress_bounded(compressed: &[u8], max_bytes: usize) -> Result<Vec<u8>> {
    let decoder = DeflateDecoder::new(compressed);
    let mut out = Vec::new();
    let limit = u64::try_from(max_bytes)
        .unwrap_or(u64::MAX - 1)
        .saturating_add(1);
    decoder
        .take(limit)
        .read_to_end(&mut out)
        .map_err(|_| Error::InvalidPayload)?;
    if out.len() > max_bytes {
        return Err(Error::InvalidPayload);
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq)]
pub struct SealedField {
    pub record_id: String,
    pub payload: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenedField {
    pub key_id: String,
    pub table: String,
    pub row_id: String,
    pub field: String,
    pub writer_id: String,
    pub revision: u64,
    pub deleted: bool,
    pub edited_at_ms: Option<u64>,
    pub value: Value,
}

pub fn payload_hash(payload: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(payload.as_bytes()))
}

pub fn bytes_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(char::from(b"0123456789abcdef"[(byte >> 4) as usize]));
        encoded.push(char::from(b"0123456789abcdef"[(byte & 0x0f) as usize]));
    }
    encoded
}

#[derive(Serialize)]
struct ProtectedField<'a> {
    table: &'a str,
    row_id: &'a str,
    field: &'a str,
    writer_id: &'a str,
    revision: u64,
    deleted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    edited_at_ms: Option<u64>,
    value: Value,
}

#[derive(Deserialize)]
struct OwnedProtectedField {
    table: String,
    row_id: String,
    field: String,
    #[serde(default)]
    writer_id: String,
    revision: u64,
    deleted: bool,
    #[serde(default)]
    edited_at_ms: Option<u64>,
    value: Value,
}

#[derive(Deserialize, Serialize)]
struct Envelope {
    version: u8,
    key_id: String,
    nonce: String,
    ciphertext: String,
}

fn is_valid_writer_id(writer_id: &str) -> bool {
    writer_id.len() == 32
        && writer_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
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

fn payload_aad_for_version(
    workspace_id: &str,
    record_id: &str,
    key_id: &str,
    version: u8,
) -> Vec<u8> {
    let domain = if version == 2 {
        PAYLOAD_AAD_V2_DOMAIN
    } else {
        PAYLOAD_AAD_DOMAIN
    };
    let mut aad = Vec::new();
    for value in [
        domain,
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
    fn personal_payload_key_id_is_not_the_recovery_identity() {
        let recovery = recovery_key();
        let workspace = recovery.workspace_key("workspace-a").unwrap();
        let sealed = workspace
            .seal_field(
                "workspace-a",
                "sessions",
                "session-1",
                "title",
                "00000000000000000000000000000001",
                1,
                false,
                json!("Planning"),
            )
            .unwrap();
        let envelope: Value = serde_json::from_str(&sealed.payload).unwrap();

        assert_ne!(workspace.key_id(), recovery.key_id());
        assert_eq!(envelope["key_id"], workspace.key_id());
        assert_ne!(envelope["key_id"], recovery.key_id());
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
                "00000000000000000000000000000001",
                1,
                false,
                json!({ "type": "doc", "content": [] }),
            )
            .unwrap();

        assert!(!sealed.payload.contains("session_documents"));
        assert!(!sealed.payload.contains("document-1"));
        assert!(!sealed.payload.contains("content"));
        let envelope: Value = serde_json::from_str(&sealed.payload).unwrap();
        assert_eq!(envelope["version"], 1);
        let opened = key
            .open_field("workspace-a", &sealed.record_id, &sealed.payload)
            .unwrap();

        assert_eq!(opened.table, "session_documents");
        assert_eq!(opened.row_id, "document-1");
        assert_eq!(opened.field, "body");
        assert_eq!(opened.writer_id, "00000000000000000000000000000001");
        assert_eq!(opened.revision, 1);
        assert_eq!(opened.value, json!({ "type": "doc", "content": [] }));
    }

    #[test]
    fn rejects_invalid_writer_ids_and_decodes_legacy_fields() {
        let key = recovery_key().workspace_key("workspace-a").unwrap();
        let error = key
            .seal_field(
                "workspace-a",
                "sessions",
                "session-1",
                "title",
                "short",
                1,
                false,
                json!("Planning"),
            )
            .unwrap_err();
        assert!(matches!(error, Error::InvalidWriterId));

        let legacy: OwnedProtectedField = serde_json::from_value(json!({
            "table": "sessions",
            "row_id": "session-1",
            "field": "title",
            "revision": 1,
            "deleted": false,
            "value": "Planning"
        }))
        .unwrap();
        assert!(legacy.writer_id.is_empty());
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
                "00000000000000000000000000000001",
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

    #[test]
    fn transfers_recovery_keys_between_devices() {
        let recipient = DeviceEnrollmentKey::generate().unwrap();
        let package = seal_recovery_key_for_device(
            &recovery_key(),
            &recipient.public_key(),
            "account-a",
            "request-a",
        )
        .unwrap();

        let opened = recipient
            .open_recovery_key("account-a", "request-a", &package)
            .unwrap();
        assert_eq!(opened.key_id(), recovery_key().key_id());
    }

    #[test]
    fn device_enrollment_packages_are_bound_to_account_and_request() {
        let recipient = DeviceEnrollmentKey::generate().unwrap();
        let package = seal_recovery_key_for_device(
            &recovery_key(),
            &recipient.public_key(),
            "account-a",
            "request-a",
        )
        .unwrap();

        assert!(
            recipient
                .open_recovery_key("account-b", "request-a", &package)
                .is_err()
        );
        assert!(
            recipient
                .open_recovery_key("account-a", "request-b", &package)
                .is_err()
        );
    }

    #[test]
    fn rejects_noncontributory_recipient_public_key() {
        let noncontributory_public_key = URL_SAFE_NO_PAD.encode([0_u8; 32]);

        assert!(matches!(
            seal_recovery_key_for_device(
                &recovery_key(),
                &noncontributory_public_key,
                "account-a",
                "request-a",
            ),
            Err(Error::InvalidDeviceEnrollmentKey)
        ));
    }

    #[test]
    fn rejects_noncontributory_ephemeral_public_key() {
        let account_user_id = "account-a";
        let request_id = "request-a";
        let nonce = [0_u8; 24];
        let cipher = enrollment_cipher(&[0_u8; 32], account_user_id, request_id).unwrap();
        let recovery_code = recovery_key().expose_code();
        let ciphertext = cipher
            .encrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: recovery_code.as_bytes(),
                    aad: &enrollment_aad(account_user_id, request_id),
                },
            )
            .unwrap();
        let package = DeviceEnrollmentPackage {
            ephemeral_public_key: URL_SAFE_NO_PAD.encode([0_u8; 32]),
            nonce: URL_SAFE_NO_PAD.encode(nonce),
            ciphertext: URL_SAFE_NO_PAD.encode(ciphertext),
        };

        assert!(matches!(
            DeviceEnrollmentKey::generate().unwrap().open_recovery_key(
                account_user_id,
                request_id,
                &package,
            ),
            Err(Error::InvalidDeviceEnrollmentPackage)
        ));
    }

    #[test]
    fn device_enrollment_keys_round_trip() {
        let key = DeviceEnrollmentKey::generate().unwrap();
        let encoded = key.expose_code();
        let parsed = DeviceEnrollmentKey::parse(&encoded).unwrap();
        assert_eq!(parsed.public_key(), key.public_key());
    }

    #[test]
    fn v2_compressed_fields_round_trip_and_shrink_repetitive_docs() {
        let key = recovery_key().workspace_key("workspace-a").unwrap();
        let doc = json!({
            "type": "doc",
            "content": vec![json!({
                "type": "paragraph",
                "content": [{ "type": "text", "text": "Meeting notes repeated line for deflate ratio check." }]
            }); 32]
        });
        let writer = "00000000000000000000000000000001";
        let v1 = key
            .seal_field_at(
                "workspace-a",
                "session_documents",
                "doc-1",
                "body",
                writer,
                3,
                false,
                Some(42),
                doc.clone(),
            )
            .unwrap();
        let v2 = key
            .seal_field_v2_at(
                "workspace-a",
                "session_documents",
                "doc-1",
                "body",
                writer,
                3,
                false,
                Some(42),
                doc.clone(),
            )
            .unwrap();
        let env: Value = serde_json::from_str(&v2.payload).unwrap();
        assert_eq!(env["version"], 2);
        assert!(v2.payload.len() * 3 < v1.payload.len());

        let opened = key
            .open_field("workspace-a", &v2.record_id, &v2.payload)
            .unwrap();
        assert_eq!(opened.value, doc);
        assert_eq!(opened.revision, 3);
        assert_eq!(opened.edited_at_ms, Some(42));

        // Version downgrade/substitution in envelope header must fail AEAD authentication.
        let mut downgraded = env;
        downgraded["version"] = json!(1);
        assert!(matches!(
            key.open_field("workspace-a", &v2.record_id, &downgraded.to_string()),
            Err(Error::AuthenticationFailed)
        ));

        // Short strings below compression threshold stay on v1 automatically.
        let small = key
            .seal_field_v2_at(
                "workspace-a",
                "sessions",
                "s-1",
                "title",
                writer,
                1,
                false,
                None,
                json!("Hi"),
            )
            .unwrap();
        let small_env: Value = serde_json::from_str(&small.payload).unwrap();
        assert_eq!(small_env["version"], 1);
        assert_eq!(
            key.open_field("workspace-a", &small.record_id, &small.payload)
                .unwrap()
                .value,
            json!("Hi")
        );
    }

    #[test]
    fn rejects_v2_zip_bombs_past_limit() {
        let key = recovery_key().workspace_key("workspace-a").unwrap();
        let record_id = key.blind_field_id("sessions", "s-bomb", "title");
        let giant = vec![b'A'; 64 * 1024];
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(&giant).unwrap();
        let compressed = encoder.finish().unwrap();
        let err = decompress_bounded(&compressed, 1024).unwrap_err();
        assert!(matches!(err, Error::InvalidPayload));

        let nonce = [1_u8; 24];
        let aad = payload_aad_for_version("workspace-a", &record_id, key.key_id(), 2);
        let cipher = XChaCha20Poly1305::new_from_slice(key.expose_bytes().as_slice()).unwrap();
        let ciphertext = cipher
            .encrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: &compressed,
                    aad: &aad,
                },
            )
            .unwrap();
        let payload = serde_json::to_string(&Envelope {
            version: 2,
            key_id: key.key_id().to_string(),
            nonce: URL_SAFE_NO_PAD.encode(nonce),
            ciphertext: URL_SAFE_NO_PAD.encode(ciphertext),
        })
        .unwrap();
        assert!(key.open_field("workspace-a", &record_id, &payload).is_err());
    }
}
