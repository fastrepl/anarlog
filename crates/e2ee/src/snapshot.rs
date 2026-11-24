use std::io::Write;

use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use flate2::{Compression, write::DeflateEncoder};
use serde::{Deserialize, Serialize};

use crate::{Error, Result, WorkspaceKey, WorkspaceKeyring, decompress_bounded};

pub const SNAPSHOT_MAGIC: &[u8; 8] = b"ANLGSNP1";
pub const SNAPSHOT_FORMAT_VERSION: u16 = 1;
const SNAPSHOT_AAD_DOMAIN: &[u8] = b"anarlog-e2ee-snapshot-v1";
const MAX_DECOMPRESSED_SNAPSHOT_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotRecord {
    pub record_id: String,
    pub revision: i64,
    pub writer_id: String,
    pub payload_hash: String,
    pub payload: String,
    pub sequence: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSnapshotPayload {
    pub workspace_id: String,
    pub snapshot_sequence: u64,
    pub cloud_authority_after: Option<u64>,
    pub records: Vec<SnapshotRecord>,
}

fn snapshot_aad(
    format_version: u16,
    workspace_id: &str,
    snapshot_sequence: u64,
    key_id: &str,
) -> Vec<u8> {
    let mut aad = Vec::new();
    let version = format_version.to_be_bytes();
    let sequence = snapshot_sequence.to_be_bytes();
    for part in [
        SNAPSHOT_AAD_DOMAIN,
        &version,
        workspace_id.as_bytes(),
        &sequence,
        key_id.as_bytes(),
    ] {
        aad.extend_from_slice(&(part.len() as u64).to_be_bytes());
        aad.extend_from_slice(part);
    }
    aad
}

pub fn seal_snapshot(key: &WorkspaceKey, payload: &WorkspaceSnapshotPayload) -> Result<Vec<u8>> {
    if payload.snapshot_sequence == 0 {
        return Err(Error::InvalidPayload);
    }
    let key_id_bytes = key.key_id().as_bytes();
    let key_id_len = u8::try_from(key_id_bytes.len()).map_err(|_| Error::InvalidPayload)?;

    let plaintext = serde_json::to_vec(payload).map_err(|_| Error::InvalidPayload)?;
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
    encoder
        .write_all(&plaintext)
        .map_err(|_| Error::InvalidPayload)?;
    let compressed = encoder.finish().map_err(|_| Error::InvalidPayload)?;

    let mut nonce_bytes = [0_u8; 24];
    getrandom::fill(&mut nonce_bytes).map_err(|_| Error::RandomnessUnavailable)?;
    let aad = snapshot_aad(
        SNAPSHOT_FORMAT_VERSION,
        &payload.workspace_id,
        payload.snapshot_sequence,
        key.key_id(),
    );
    let cipher = XChaCha20Poly1305::new_from_slice(key.expose_bytes().as_slice())
        .expect("XChaCha20Poly1305 accepts 32-byte keys");
    let ciphertext = cipher
        .encrypt(
            &XNonce::from(nonce_bytes),
            Payload {
                msg: &compressed,
                aad: &aad,
            },
        )
        .map_err(|_| Error::AuthenticationFailed)?;

    let mut framed = Vec::with_capacity(11 + key_id_bytes.len() + 24 + ciphertext.len());
    framed.extend_from_slice(SNAPSHOT_MAGIC);
    framed.extend_from_slice(&SNAPSHOT_FORMAT_VERSION.to_be_bytes());
    framed.push(key_id_len);
    framed.extend_from_slice(key_id_bytes);
    framed.extend_from_slice(&nonce_bytes);
    framed.extend_from_slice(&ciphertext);
    Ok(framed)
}

pub fn open_snapshot(
    keyring: &WorkspaceKeyring,
    workspace_id: &str,
    expected_sequence: u64,
    bytes: &[u8],
) -> Result<WorkspaceSnapshotPayload> {
    if bytes.len() < SNAPSHOT_MAGIC.len() + 3 + 24 || &bytes[..8] != SNAPSHOT_MAGIC {
        return Err(Error::InvalidPayload);
    }
    let format_version = u16::from_be_bytes([bytes[8], bytes[9]]);
    if format_version != SNAPSHOT_FORMAT_VERSION {
        return Err(Error::InvalidPayload);
    }
    let key_id_len = bytes[10] as usize;
    let key_id_end = 11 + key_id_len;
    if bytes.len() < key_id_end + 24 {
        return Err(Error::InvalidPayload);
    }
    let key_id = std::str::from_utf8(&bytes[11..key_id_end]).map_err(|_| Error::InvalidPayload)?;
    let key = keyring.get(key_id).ok_or(Error::UnknownKey)?;
    let nonce: [u8; 24] = bytes[key_id_end..key_id_end + 24]
        .try_into()
        .map_err(|_| Error::InvalidPayload)?;
    let ciphertext = &bytes[key_id_end + 24..];

    let aad = snapshot_aad(format_version, workspace_id, expected_sequence, key_id);
    let cipher = XChaCha20Poly1305::new_from_slice(key.expose_bytes().as_slice())
        .expect("XChaCha20Poly1305 accepts 32-byte keys");
    let compressed = cipher
        .decrypt(
            &XNonce::from(nonce),
            Payload {
                msg: ciphertext,
                aad: &aad,
            },
        )
        .map_err(|_| Error::AuthenticationFailed)?;
    let plaintext = decompress_bounded(&compressed, MAX_DECOMPRESSED_SNAPSHOT_BYTES)?;
    let parsed: WorkspaceSnapshotPayload =
        serde_json::from_slice(&plaintext).map_err(|_| Error::InvalidPayload)?;
    if parsed.workspace_id != workspace_id || parsed.snapshot_sequence != expected_sequence {
        return Err(Error::AuthenticationFailed);
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{RecoveryKey, payload_hash};

    #[test]
    fn snapshot_seals_opens_and_rejects_tampering() {
        let key = RecoveryKey([7; 32]).workspace_key("workspace-a").unwrap();
        let sealed_field = key
            .seal_field_v2_at(
                "workspace-a",
                "sessions",
                "s1",
                "title",
                "00000000000000000000000000000001",
                1,
                false,
                Some(10),
                json!("Snapshot Title"),
            )
            .unwrap();
        let snapshot = WorkspaceSnapshotPayload {
            workspace_id: "workspace-a".to_string(),
            snapshot_sequence: 42,
            cloud_authority_after: Some(0),
            records: vec![SnapshotRecord {
                record_id: sealed_field.record_id,
                revision: 1,
                writer_id: "00000000000000000000000000000001".to_string(),
                payload_hash: payload_hash(&sealed_field.payload),
                payload: sealed_field.payload,
                sequence: 42,
            }],
        };

        let bytes = seal_snapshot(&key, &snapshot).unwrap();
        assert_eq!(&bytes[..8], SNAPSHOT_MAGIC);
        let keyring = WorkspaceKeyring::new(key.clone());
        let opened = open_snapshot(&keyring, "workspace-a", 42, &bytes).unwrap();
        assert_eq!(opened, snapshot);

        assert!(matches!(
            open_snapshot(&keyring, "workspace-b", 42, &bytes),
            Err(Error::AuthenticationFailed)
        ));
        assert!(matches!(
            open_snapshot(&keyring, "workspace-a", 41, &bytes),
            Err(Error::AuthenticationFailed)
        ));

        let wrong_keyring =
            WorkspaceKeyring::new(RecoveryKey([8; 32]).workspace_key("workspace-a").unwrap());
        assert!(matches!(
            open_snapshot(&wrong_keyring, "workspace-a", 42, &bytes),
            Err(Error::UnknownKey)
        ));

        let mut tampered = bytes.clone();
        *tampered.last_mut().unwrap() ^= 0xff;
        assert!(matches!(
            open_snapshot(&keyring, "workspace-a", 42, &tampered),
            Err(Error::AuthenticationFailed)
        ));
    }
}
