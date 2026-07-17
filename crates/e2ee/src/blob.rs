use std::io::{self, Read, Write};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chacha20poly1305::aead::AeadInPlace;
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use super::WorkspaceKey;

pub const ATTACHMENT_BLOB_CHUNK_SIZE: usize = 4 * 1024 * 1024;
pub const ATTACHMENT_BLOB_MAX_PLAINTEXT_BYTES: u64 = 512 * 1024 * 1024;

const MAGIC: &[u8; 8] = b"ANABLB01";
const VERSION: u8 = 1;
const HEADER_SCHEMA_VERSION: u8 = 1;
const AEAD_TAG_BYTES: u64 = 16;
const HEADER_NONCE_BYTES: usize = 24;
const CHUNK_NONCE_PREFIX_BYTES: usize = 16;
const FIXED_PREFIX_BYTES: usize = MAGIC.len() + 1 + HEADER_NONCE_BYTES + 4;
const MAX_WORKSPACE_ID_BYTES: usize = 256;
const MAX_ATTACHMENT_ID_BYTES: usize = 1024;
const CANONICAL_UUID_BYTES: usize = 36;
const BLOB_KEY_SALT: &[u8] = b"anarlog-e2ee-attachment-blob-key-v1";
const BLOB_KEY_INFO_DOMAIN: &[u8] = b"anarlog-e2ee-attachment-blob-object-v1";
const HEADER_KEY_INFO: &[u8] = b"anarlog-e2ee-attachment-blob-header-key-v1";
const CHUNK_KEY_INFO: &[u8] = b"anarlog-e2ee-attachment-blob-chunk-key-v1";
const HEADER_AAD_DOMAIN: &[u8] = b"anarlog-e2ee-attachment-blob-header-aad-v1";
const CHUNK_AAD_DOMAIN: &[u8] = b"anarlog-e2ee-attachment-blob-chunk-aad-v1";
const ATTACHMENT_BACKUP_REF_DOMAIN: &[u8] = b"anarlog-e2ee-attachment-backup-ref-v1";
const ATTACHMENT_BACKUP_VERSION_REF_DOMAIN: &[u8] =
    b"anarlog-e2ee-attachment-backup-version-ref-v1";

#[derive(Debug, thiserror::Error)]
pub enum AttachmentBlobError {
    #[error("the attachment blob context is invalid")]
    InvalidContext,
    #[error("the attachment blob metadata is invalid")]
    InvalidMetadata,
    #[error("the attachment blob version is unsupported")]
    UnsupportedVersion,
    #[error("the attachment blob exceeds the plaintext size limit")]
    PlaintextTooLarge,
    #[error("the attachment blob header is invalid")]
    InvalidHeader,
    #[error("the attachment blob is truncated")]
    Truncated,
    #[error("the attachment blob contains trailing data")]
    TrailingData,
    #[error("the attachment source does not match its metadata")]
    SourceMismatch,
    #[error("the attachment ciphertext does not match its metadata")]
    CiphertextMismatch,
    #[error("the attachment blob failed authentication")]
    AuthenticationFailed,
    #[error("cryptographic randomness is unavailable")]
    RandomnessUnavailable,
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub type AttachmentBlobResult<T> = std::result::Result<T, AttachmentBlobError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentBlobContext {
    workspace_id: String,
    attachment_id: String,
    object_id: String,
}

impl AttachmentBlobContext {
    pub fn new(
        workspace_id: impl Into<String>,
        attachment_id: impl Into<String>,
        object_id: impl Into<String>,
    ) -> AttachmentBlobResult<Self> {
        let workspace_id = workspace_id.into();
        let attachment_id = attachment_id.into();
        let object_id = object_id.into();
        validate_identifier(&workspace_id, MAX_WORKSPACE_ID_BYTES)?;
        validate_identifier(&attachment_id, MAX_ATTACHMENT_ID_BYTES)?;
        if !is_canonical_uuid(&object_id) {
            return Err(AttachmentBlobError::InvalidContext);
        }

        Ok(Self {
            workspace_id,
            attachment_id,
            object_id,
        })
    }

    pub fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    pub fn attachment_id(&self) -> &str {
        &self.attachment_id
    }

    pub fn object_id(&self) -> &str {
        &self.object_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentBlobPlaintextMetadata {
    pub size_bytes: u64,
    pub sha256: [u8; 32],
}

impl AttachmentBlobPlaintextMetadata {
    pub fn new(size_bytes: u64, sha256: [u8; 32]) -> Self {
        Self { size_bytes, sha256 }
    }

    pub fn from_hex(size_bytes: u64, sha256: &str) -> AttachmentBlobResult<Self> {
        Ok(Self::new(size_bytes, decode_sha256_hex(sha256)?))
    }

    pub fn sha256_hex(&self) -> String {
        encode_sha256_hex(&self.sha256)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentBlobCiphertextMetadata {
    pub size_bytes: u64,
    pub sha256: [u8; 32],
}

impl AttachmentBlobCiphertextMetadata {
    pub fn new(size_bytes: u64, sha256: [u8; 32]) -> Self {
        Self { size_bytes, sha256 }
    }

    pub fn from_hex(size_bytes: u64, sha256: &str) -> AttachmentBlobResult<Self> {
        Ok(Self::new(size_bytes, decode_sha256_hex(sha256)?))
    }

    pub fn sha256_hex(&self) -> String {
        encode_sha256_hex(&self.sha256)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentBlobMetadata {
    pub version: u8,
    pub plaintext: AttachmentBlobPlaintextMetadata,
    pub ciphertext: AttachmentBlobCiphertextMetadata,
}

impl WorkspaceKey {
    pub fn attachment_blob_ciphertext_size(
        &self,
        workspace_id: &str,
        attachment_id: &str,
        plaintext_size_bytes: u64,
    ) -> AttachmentBlobResult<u64> {
        let sizing_context = AttachmentBlobContext::new(
            workspace_id,
            attachment_id,
            "00000000-0000-4000-8000-000000000000",
        )?;
        encoded_size(&sizing_context, self.key_id(), plaintext_size_bytes)
    }

    pub fn blind_attachment_backup_ref(
        &self,
        workspace_id: &str,
        attachment_id: &str,
    ) -> AttachmentBlobResult<String> {
        validate_identifier(workspace_id, MAX_WORKSPACE_ID_BYTES)?;
        validate_identifier(attachment_id, MAX_ATTACHMENT_ID_BYTES)?;
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(self.bytes.as_slice())
            .expect("HMAC accepts 32-byte keys");
        update_mac_components(
            &mut mac,
            [
                ATTACHMENT_BACKUP_REF_DOMAIN,
                workspace_id.as_bytes(),
                attachment_id.as_bytes(),
            ],
        );
        Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
    }

    pub fn blind_attachment_backup_version_ref(
        &self,
        workspace_id: &str,
        attachment_id: &str,
        plaintext: &AttachmentBlobPlaintextMetadata,
    ) -> AttachmentBlobResult<String> {
        validate_plaintext_metadata(plaintext)?;
        let attachment_ref = self.blind_attachment_backup_ref(workspace_id, attachment_id)?;
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(self.bytes.as_slice())
            .expect("HMAC accepts 32-byte keys");
        let size = plaintext.size_bytes.to_be_bytes();
        update_mac_components(
            &mut mac,
            [
                ATTACHMENT_BACKUP_VERSION_REF_DOMAIN,
                attachment_ref.as_bytes(),
                plaintext.sha256.as_slice(),
                size.as_slice(),
            ],
        );
        Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
    }

    /// Streams a blob into `destination`. The destination must be discarded if this returns an
    /// error because source validation completes after the encrypted chunks are written.
    pub fn seal_attachment_blob<R: Read, W: Write>(
        &self,
        context: &AttachmentBlobContext,
        source: &mut R,
        destination: &mut W,
        expected_plaintext: &AttachmentBlobPlaintextMetadata,
    ) -> AttachmentBlobResult<AttachmentBlobMetadata> {
        validate_plaintext_metadata(expected_plaintext)?;
        let chunk_count = chunk_count(expected_plaintext.size_bytes);
        let keys = derive_blob_keys(self, context)?;
        let mut chunk_nonce_prefix = [0_u8; CHUNK_NONCE_PREFIX_BYTES];
        getrandom::fill(&mut chunk_nonce_prefix)
            .map_err(|_| AttachmentBlobError::RandomnessUnavailable)?;
        let header = Header {
            workspace_id: context.workspace_id.clone(),
            attachment_id: context.attachment_id.clone(),
            object_id: context.object_id.clone(),
            key_id: self.key_id().to_string(),
            plaintext_size: expected_plaintext.size_bytes,
            plaintext_sha256: expected_plaintext.sha256,
            chunk_size: ATTACHMENT_BLOB_CHUNK_SIZE as u32,
            chunk_count,
            chunk_nonce_prefix,
        };
        let mut header_plaintext = Zeroizing::new(encode_header(&header)?);
        let mut header_nonce = [0_u8; HEADER_NONCE_BYTES];
        getrandom::fill(&mut header_nonce)
            .map_err(|_| AttachmentBlobError::RandomnessUnavailable)?;
        let header_aad = header_aad(context, self.key_id());
        let header_cipher = XChaCha20Poly1305::new_from_slice(keys.header.as_slice())
            .expect("XChaCha20Poly1305 accepts 32-byte keys");
        let header_nonce = XNonce::from(header_nonce);
        header_cipher
            .encrypt_in_place(&header_nonce, &header_aad, &mut *header_plaintext)
            .map_err(|_| AttachmentBlobError::AuthenticationFailed)?;
        let header_ciphertext_len = u32::try_from(header_plaintext.len())
            .map_err(|_| AttachmentBlobError::InvalidHeader)?;

        let mut fixed_prefix = [0_u8; FIXED_PREFIX_BYTES];
        fixed_prefix[..MAGIC.len()].copy_from_slice(MAGIC);
        fixed_prefix[MAGIC.len()] = VERSION;
        fixed_prefix[MAGIC.len() + 1..MAGIC.len() + 1 + HEADER_NONCE_BYTES]
            .copy_from_slice(&header_nonce);
        fixed_prefix[FIXED_PREFIX_BYTES - 4..]
            .copy_from_slice(&header_ciphertext_len.to_be_bytes());
        let authenticated_header_hash = Sha256::new()
            .chain_update(fixed_prefix)
            .chain_update(header_plaintext.as_slice())
            .finalize();

        let mut ciphertext_hasher = Sha256::new();
        let mut ciphertext_size = 0_u64;
        write_ciphertext(
            destination,
            &mut ciphertext_hasher,
            &mut ciphertext_size,
            &fixed_prefix,
        )?;
        write_ciphertext(
            destination,
            &mut ciphertext_hasher,
            &mut ciphertext_size,
            header_plaintext.as_slice(),
        )?;

        let chunk_cipher = XChaCha20Poly1305::new_from_slice(keys.chunk.as_slice())
            .expect("XChaCha20Poly1305 accepts 32-byte keys");
        let mut plaintext_hasher = Sha256::new();
        let mut remaining = expected_plaintext.size_bytes;
        for index in 0..chunk_count {
            let plaintext_len = remaining.min(ATTACHMENT_BLOB_CHUNK_SIZE as u64) as usize;
            let mut chunk =
                Zeroizing::new(Vec::with_capacity(plaintext_len + AEAD_TAG_BYTES as usize));
            chunk.resize(plaintext_len, 0);
            read_plaintext_exact(source, chunk.as_mut_slice())?;
            plaintext_hasher.update(chunk.as_slice());
            let nonce = chunk_nonce(&chunk_nonce_prefix, u64::from(index));
            let nonce = XNonce::from(nonce);
            let aad = chunk_aad(&authenticated_header_hash, index, chunk_count);
            chunk_cipher
                .encrypt_in_place(&nonce, &aad, &mut *chunk)
                .map_err(|_| AttachmentBlobError::AuthenticationFailed)?;
            write_ciphertext(
                destination,
                &mut ciphertext_hasher,
                &mut ciphertext_size,
                chunk.as_slice(),
            )?;
            remaining -= plaintext_len as u64;
        }
        if read_one(source)?.is_some() {
            return Err(AttachmentBlobError::SourceMismatch);
        }
        let plaintext_sha256: [u8; 32] = plaintext_hasher.finalize().into();
        if plaintext_sha256 != expected_plaintext.sha256 {
            return Err(AttachmentBlobError::SourceMismatch);
        }
        let expected_ciphertext_size =
            encoded_size(context, self.key_id(), expected_plaintext.size_bytes)?;
        if ciphertext_size != expected_ciphertext_size {
            return Err(AttachmentBlobError::InvalidMetadata);
        }

        Ok(AttachmentBlobMetadata {
            version: VERSION,
            plaintext: expected_plaintext.clone(),
            ciphertext: AttachmentBlobCiphertextMetadata {
                size_bytes: ciphertext_size,
                sha256: ciphertext_hasher.finalize().into(),
            },
        })
    }

    /// Streams authenticated plaintext into `destination`. Callers must write to a temporary
    /// destination and discard it on error because the whole-ciphertext digest is verified last.
    pub fn open_attachment_blob<R: Read, W: Write>(
        &self,
        context: &AttachmentBlobContext,
        source: &mut R,
        destination: &mut W,
        expected: &AttachmentBlobMetadata,
    ) -> AttachmentBlobResult<AttachmentBlobMetadata> {
        validate_blob_metadata(context, self.key_id(), expected)?;
        let keys = derive_blob_keys(self, context)?;
        let mut ciphertext_hasher = Sha256::new();
        let mut ciphertext_size = 0_u64;
        let mut fixed_prefix = [0_u8; FIXED_PREFIX_BYTES];
        read_ciphertext_exact(
            source,
            &mut ciphertext_hasher,
            &mut ciphertext_size,
            &mut fixed_prefix,
        )?;
        if &fixed_prefix[..MAGIC.len()] != MAGIC {
            return Err(AttachmentBlobError::InvalidHeader);
        }
        if fixed_prefix[MAGIC.len()] != VERSION {
            return Err(AttachmentBlobError::UnsupportedVersion);
        }
        let header_nonce_start = MAGIC.len() + 1;
        let header_nonce: [u8; HEADER_NONCE_BYTES] = fixed_prefix
            [header_nonce_start..header_nonce_start + HEADER_NONCE_BYTES]
            .try_into()
            .expect("header nonce length is fixed");
        let header_ciphertext_len = u32::from_be_bytes(
            fixed_prefix[FIXED_PREFIX_BYTES - 4..]
                .try_into()
                .expect("header length field is fixed"),
        ) as usize;
        let expected_header_ciphertext_len = encoded_header_len(context, self.key_id())?
            .checked_add(AEAD_TAG_BYTES as usize)
            .ok_or(AttachmentBlobError::InvalidHeader)?;
        if header_ciphertext_len != expected_header_ciphertext_len {
            return Err(AttachmentBlobError::InvalidHeader);
        }

        let mut header_ciphertext = Zeroizing::new(vec![0_u8; header_ciphertext_len]);
        read_ciphertext_exact(
            source,
            &mut ciphertext_hasher,
            &mut ciphertext_size,
            header_ciphertext.as_mut_slice(),
        )?;
        let authenticated_header_hash = Sha256::new()
            .chain_update(fixed_prefix)
            .chain_update(header_ciphertext.as_slice())
            .finalize();
        let header_cipher = XChaCha20Poly1305::new_from_slice(keys.header.as_slice())
            .expect("XChaCha20Poly1305 accepts 32-byte keys");
        let header_aad = header_aad(context, self.key_id());
        let header_nonce = XNonce::from(header_nonce);
        header_cipher
            .decrypt_in_place(&header_nonce, &header_aad, &mut *header_ciphertext)
            .map_err(|_| AttachmentBlobError::AuthenticationFailed)?;
        let header = decode_header(header_ciphertext.as_slice())?;
        validate_header(&header, context, self.key_id(), &expected.plaintext)?;

        let chunk_cipher = XChaCha20Poly1305::new_from_slice(keys.chunk.as_slice())
            .expect("XChaCha20Poly1305 accepts 32-byte keys");
        let mut plaintext_hasher = Sha256::new();
        let mut plaintext_size = 0_u64;
        let mut remaining = header.plaintext_size;
        for index in 0..header.chunk_count {
            let plaintext_len = remaining.min(ATTACHMENT_BLOB_CHUNK_SIZE as u64) as usize;
            let ciphertext_len = plaintext_len + AEAD_TAG_BYTES as usize;
            let mut chunk = Zeroizing::new(vec![0_u8; ciphertext_len]);
            read_ciphertext_exact(
                source,
                &mut ciphertext_hasher,
                &mut ciphertext_size,
                chunk.as_mut_slice(),
            )?;
            let nonce = chunk_nonce(&header.chunk_nonce_prefix, u64::from(index));
            let nonce = XNonce::from(nonce);
            let aad = chunk_aad(&authenticated_header_hash, index, header.chunk_count);
            chunk_cipher
                .decrypt_in_place(&nonce, &aad, &mut *chunk)
                .map_err(|_| AttachmentBlobError::AuthenticationFailed)?;
            if chunk.len() != plaintext_len {
                return Err(AttachmentBlobError::InvalidHeader);
            }
            destination.write_all(chunk.as_slice())?;
            plaintext_hasher.update(chunk.as_slice());
            plaintext_size += chunk.len() as u64;
            remaining -= chunk.len() as u64;
        }
        if read_one(source)?.is_some() {
            return Err(AttachmentBlobError::TrailingData);
        }
        if remaining != 0 || plaintext_size != header.plaintext_size {
            return Err(AttachmentBlobError::InvalidHeader);
        }
        let plaintext_sha256: [u8; 32] = plaintext_hasher.finalize().into();
        if plaintext_sha256 != header.plaintext_sha256 {
            return Err(AttachmentBlobError::AuthenticationFailed);
        }
        let ciphertext_sha256: [u8; 32] = ciphertext_hasher.finalize().into();
        if ciphertext_size != expected.ciphertext.size_bytes
            || ciphertext_sha256 != expected.ciphertext.sha256
        {
            return Err(AttachmentBlobError::CiphertextMismatch);
        }

        Ok(AttachmentBlobMetadata {
            version: VERSION,
            plaintext: AttachmentBlobPlaintextMetadata {
                size_bytes: plaintext_size,
                sha256: plaintext_sha256,
            },
            ciphertext: AttachmentBlobCiphertextMetadata {
                size_bytes: ciphertext_size,
                sha256: ciphertext_sha256,
            },
        })
    }
}

struct BlobKeys {
    header: Zeroizing<[u8; 32]>,
    chunk: Zeroizing<[u8; 32]>,
}

struct Header {
    workspace_id: String,
    attachment_id: String,
    object_id: String,
    key_id: String,
    plaintext_size: u64,
    plaintext_sha256: [u8; 32],
    chunk_size: u32,
    chunk_count: u32,
    chunk_nonce_prefix: [u8; CHUNK_NONCE_PREFIX_BYTES],
}

fn validate_identifier(value: &str, max_bytes: usize) -> AttachmentBlobResult<()> {
    if value.is_empty()
        || value.len() > max_bytes
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(AttachmentBlobError::InvalidContext);
    }
    Ok(())
}

fn update_mac_components<const N: usize>(mac: &mut Hmac<Sha256>, components: [&[u8]; N]) {
    for component in components {
        mac.update(&(component.len() as u64).to_be_bytes());
        mac.update(component);
    }
}

fn is_canonical_uuid(value: &str) -> bool {
    value.len() == CANONICAL_UUID_BYTES
        && matches!(value.as_bytes()[14], b'4' | b'7')
        && matches!(value.as_bytes()[19], b'8' | b'9' | b'a' | b'b')
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')
            }
        })
}

fn validate_plaintext_metadata(
    metadata: &AttachmentBlobPlaintextMetadata,
) -> AttachmentBlobResult<()> {
    if metadata.size_bytes > ATTACHMENT_BLOB_MAX_PLAINTEXT_BYTES {
        return Err(AttachmentBlobError::PlaintextTooLarge);
    }
    Ok(())
}

fn validate_blob_metadata(
    context: &AttachmentBlobContext,
    key_id: &str,
    metadata: &AttachmentBlobMetadata,
) -> AttachmentBlobResult<()> {
    if metadata.version != VERSION {
        return Err(AttachmentBlobError::UnsupportedVersion);
    }
    validate_plaintext_metadata(&metadata.plaintext)?;
    let expected_size = encoded_size(context, key_id, metadata.plaintext.size_bytes)?;
    if metadata.ciphertext.size_bytes != expected_size {
        return Err(AttachmentBlobError::InvalidMetadata);
    }
    Ok(())
}

fn derive_blob_keys(
    workspace_key: &WorkspaceKey,
    context: &AttachmentBlobContext,
) -> AttachmentBlobResult<BlobKeys> {
    let hkdf = Hkdf::<Sha256>::new(Some(BLOB_KEY_SALT), workspace_key.bytes.as_slice());
    let mut root = Zeroizing::new([0_u8; 32]);
    hkdf.expand(
        &context_bytes(BLOB_KEY_INFO_DOMAIN, context, workspace_key.key_id()),
        &mut *root,
    )
    .map_err(|_| AttachmentBlobError::InvalidContext)?;
    let hkdf = Hkdf::<Sha256>::from_prk(root.as_slice())
        .map_err(|_| AttachmentBlobError::InvalidContext)?;
    let mut header = Zeroizing::new([0_u8; 32]);
    let mut chunk = Zeroizing::new([0_u8; 32]);
    hkdf.expand(HEADER_KEY_INFO, &mut *header)
        .map_err(|_| AttachmentBlobError::InvalidContext)?;
    hkdf.expand(CHUNK_KEY_INFO, &mut *chunk)
        .map_err(|_| AttachmentBlobError::InvalidContext)?;
    Ok(BlobKeys { header, chunk })
}

fn header_aad(context: &AttachmentBlobContext, key_id: &str) -> Vec<u8> {
    context_bytes(HEADER_AAD_DOMAIN, context, key_id)
}

fn context_bytes(domain: &[u8], context: &AttachmentBlobContext, key_id: &str) -> Vec<u8> {
    let mut output = Vec::new();
    for value in [
        domain,
        MAGIC.as_slice(),
        &[VERSION],
        context.workspace_id.as_bytes(),
        context.attachment_id.as_bytes(),
        context.object_id.as_bytes(),
        key_id.as_bytes(),
    ] {
        output.extend_from_slice(&(value.len() as u64).to_be_bytes());
        output.extend_from_slice(value);
    }
    output
}

fn chunk_aad(header_hash: &[u8], index: u32, chunk_count: u32) -> Vec<u8> {
    let mut output = Vec::with_capacity(CHUNK_AAD_DOMAIN.len() + header_hash.len() + 24);
    for value in [CHUNK_AAD_DOMAIN, header_hash] {
        output.extend_from_slice(&(value.len() as u64).to_be_bytes());
        output.extend_from_slice(value);
    }
    output.extend_from_slice(&index.to_be_bytes());
    output.extend_from_slice(&chunk_count.to_be_bytes());
    output
}

fn chunk_nonce(prefix: &[u8; CHUNK_NONCE_PREFIX_BYTES], index: u64) -> [u8; 24] {
    let mut nonce = [0_u8; 24];
    nonce[..CHUNK_NONCE_PREFIX_BYTES].copy_from_slice(prefix);
    nonce[CHUNK_NONCE_PREFIX_BYTES..].copy_from_slice(&index.to_be_bytes());
    nonce
}

fn encode_header(header: &Header) -> AttachmentBlobResult<Vec<u8>> {
    let mut output = Vec::with_capacity(encoded_header_len_values(
        &header.workspace_id,
        &header.attachment_id,
        &header.object_id,
        &header.key_id,
    )?);
    output.push(HEADER_SCHEMA_VERSION);
    push_string(&mut output, &header.workspace_id)?;
    push_string(&mut output, &header.attachment_id)?;
    push_string(&mut output, &header.object_id)?;
    push_string(&mut output, &header.key_id)?;
    output.extend_from_slice(&header.plaintext_size.to_be_bytes());
    output.extend_from_slice(&header.plaintext_sha256);
    output.extend_from_slice(&header.chunk_size.to_be_bytes());
    output.extend_from_slice(&header.chunk_count.to_be_bytes());
    output.extend_from_slice(&header.chunk_nonce_prefix);
    Ok(output)
}

fn decode_header(bytes: &[u8]) -> AttachmentBlobResult<Header> {
    let mut decoder = Decoder::new(bytes);
    if decoder.take_u8()? != HEADER_SCHEMA_VERSION {
        return Err(AttachmentBlobError::InvalidHeader);
    }
    let workspace_id = decoder.take_string(MAX_WORKSPACE_ID_BYTES)?;
    let attachment_id = decoder.take_string(MAX_ATTACHMENT_ID_BYTES)?;
    let object_id = decoder.take_string(CANONICAL_UUID_BYTES)?;
    let key_id = decoder.take_string(64)?;
    let plaintext_size = decoder.take_u64()?;
    let plaintext_sha256 = decoder.take_array::<32>()?;
    let chunk_size = decoder.take_u32()?;
    let chunk_count = decoder.take_u32()?;
    let chunk_nonce_prefix = decoder.take_array::<CHUNK_NONCE_PREFIX_BYTES>()?;
    if !decoder.is_empty() {
        return Err(AttachmentBlobError::InvalidHeader);
    }
    Ok(Header {
        workspace_id,
        attachment_id,
        object_id,
        key_id,
        plaintext_size,
        plaintext_sha256,
        chunk_size,
        chunk_count,
        chunk_nonce_prefix,
    })
}

fn validate_header(
    header: &Header,
    context: &AttachmentBlobContext,
    key_id: &str,
    expected_plaintext: &AttachmentBlobPlaintextMetadata,
) -> AttachmentBlobResult<()> {
    if header.workspace_id != context.workspace_id
        || header.attachment_id != context.attachment_id
        || header.object_id != context.object_id
        || header.key_id != key_id
        || header.plaintext_size != expected_plaintext.size_bytes
        || header.plaintext_sha256 != expected_plaintext.sha256
        || header.chunk_size != ATTACHMENT_BLOB_CHUNK_SIZE as u32
        || header.chunk_count != chunk_count(header.plaintext_size)
    {
        return Err(AttachmentBlobError::InvalidHeader);
    }
    validate_plaintext_metadata(expected_plaintext)
}

fn encoded_size(
    context: &AttachmentBlobContext,
    key_id: &str,
    plaintext_size: u64,
) -> AttachmentBlobResult<u64> {
    if plaintext_size > ATTACHMENT_BLOB_MAX_PLAINTEXT_BYTES {
        return Err(AttachmentBlobError::PlaintextTooLarge);
    }
    let header_size = encoded_header_len(context, key_id)? as u64 + AEAD_TAG_BYTES;
    let chunk_overhead = u64::from(chunk_count(plaintext_size))
        .checked_mul(AEAD_TAG_BYTES)
        .ok_or(AttachmentBlobError::InvalidMetadata)?;
    (FIXED_PREFIX_BYTES as u64)
        .checked_add(header_size)
        .and_then(|size| size.checked_add(plaintext_size))
        .and_then(|size| size.checked_add(chunk_overhead))
        .ok_or(AttachmentBlobError::InvalidMetadata)
}

fn encoded_header_len(
    context: &AttachmentBlobContext,
    key_id: &str,
) -> AttachmentBlobResult<usize> {
    encoded_header_len_values(
        &context.workspace_id,
        &context.attachment_id,
        &context.object_id,
        key_id,
    )
}

fn encoded_header_len_values(
    workspace_id: &str,
    attachment_id: &str,
    object_id: &str,
    key_id: &str,
) -> AttachmentBlobResult<usize> {
    let string_bytes = [workspace_id, attachment_id, object_id, key_id]
        .into_iter()
        .try_fold(0_usize, |total, value| {
            u16::try_from(value.len())
                .map_err(|_| AttachmentBlobError::InvalidHeader)
                .and_then(|_| {
                    total
                        .checked_add(2 + value.len())
                        .ok_or(AttachmentBlobError::InvalidHeader)
                })
        })?;
    1_usize
        .checked_add(string_bytes)
        .and_then(|size| size.checked_add(8 + 32 + 4 + 4 + CHUNK_NONCE_PREFIX_BYTES))
        .ok_or(AttachmentBlobError::InvalidHeader)
}

fn chunk_count(size: u64) -> u32 {
    if size == 0 {
        return 0;
    }
    size.div_ceil(ATTACHMENT_BLOB_CHUNK_SIZE as u64) as u32
}

fn push_string(output: &mut Vec<u8>, value: &str) -> AttachmentBlobResult<()> {
    let length = u16::try_from(value.len()).map_err(|_| AttachmentBlobError::InvalidHeader)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn write_ciphertext<W: Write>(
    destination: &mut W,
    hasher: &mut Sha256,
    size: &mut u64,
    bytes: &[u8],
) -> AttachmentBlobResult<()> {
    destination.write_all(bytes)?;
    hasher.update(bytes);
    *size = size
        .checked_add(bytes.len() as u64)
        .ok_or(AttachmentBlobError::InvalidMetadata)?;
    Ok(())
}

fn read_plaintext_exact<R: Read>(source: &mut R, buffer: &mut [u8]) -> AttachmentBlobResult<()> {
    read_exact(source, buffer).map_err(|error| match error.kind() {
        io::ErrorKind::UnexpectedEof => AttachmentBlobError::SourceMismatch,
        _ => AttachmentBlobError::Io(error),
    })
}

fn read_ciphertext_exact<R: Read>(
    source: &mut R,
    hasher: &mut Sha256,
    size: &mut u64,
    buffer: &mut [u8],
) -> AttachmentBlobResult<()> {
    read_exact(source, buffer).map_err(|error| match error.kind() {
        io::ErrorKind::UnexpectedEof => AttachmentBlobError::Truncated,
        _ => AttachmentBlobError::Io(error),
    })?;
    hasher.update(&*buffer);
    *size = size
        .checked_add(buffer.len() as u64)
        .ok_or(AttachmentBlobError::InvalidMetadata)?;
    Ok(())
}

fn read_exact<R: Read>(source: &mut R, mut buffer: &mut [u8]) -> io::Result<()> {
    while !buffer.is_empty() {
        match source.read(buffer) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(read) => buffer = &mut buffer[read..],
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn read_one<R: Read>(source: &mut R) -> AttachmentBlobResult<Option<u8>> {
    let mut byte = [0_u8; 1];
    loop {
        match source.read(&mut byte) {
            Ok(0) => return Ok(None),
            Ok(_) => return Ok(Some(byte[0])),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(AttachmentBlobError::Io(error)),
        }
    }
}

fn decode_sha256_hex(value: &str) -> AttachmentBlobResult<[u8; 32]> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(AttachmentBlobError::InvalidMetadata);
    }
    let mut output = [0_u8; 32];
    for (index, byte) in output.iter_mut().enumerate() {
        let offset = index * 2;
        *byte = u8::from_str_radix(&value[offset..offset + 2], 16)
            .map_err(|_| AttachmentBlobError::InvalidMetadata)?;
    }
    Ok(output)
}

fn encode_sha256_hex(value: &[u8; 32]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(64);
    for byte in value {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

struct Decoder<'a> {
    remaining: &'a [u8],
}

impl<'a> Decoder<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    fn take_u8(&mut self) -> AttachmentBlobResult<u8> {
        Ok(self.take_array::<1>()?[0])
    }

    fn take_u32(&mut self) -> AttachmentBlobResult<u32> {
        Ok(u32::from_be_bytes(self.take_array()?))
    }

    fn take_u64(&mut self) -> AttachmentBlobResult<u64> {
        Ok(u64::from_be_bytes(self.take_array()?))
    }

    fn take_string(&mut self, max_bytes: usize) -> AttachmentBlobResult<String> {
        let length = u16::from_be_bytes(self.take_array()?) as usize;
        if length == 0 || length > max_bytes {
            return Err(AttachmentBlobError::InvalidHeader);
        }
        let bytes = self.take(length)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| AttachmentBlobError::InvalidHeader)
    }

    fn take_array<const N: usize>(&mut self) -> AttachmentBlobResult<[u8; N]> {
        self.take(N)?
            .try_into()
            .map_err(|_| AttachmentBlobError::InvalidHeader)
    }

    fn take(&mut self, length: usize) -> AttachmentBlobResult<&'a [u8]> {
        if self.remaining.len() < length {
            return Err(AttachmentBlobError::InvalidHeader);
        }
        let (value, remaining) = self.remaining.split_at(length);
        self.remaining = remaining;
        Ok(value)
    }

    fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::RecoveryKey;

    const OBJECT_ID: &str = "019f6b9d-5ca3-7e61-8414-2be0ad5d9712";

    fn key(seed: u8) -> WorkspaceKey {
        RecoveryKey::parse(&format!(
            "anarlog-e2ee-v1:{}",
            base64_url_no_pad(&[seed; 32])
        ))
        .unwrap()
        .workspace_key("workspace-a")
        .unwrap()
    }

    fn context() -> AttachmentBlobContext {
        AttachmentBlobContext::new("workspace-a", "attachment-1", OBJECT_ID).unwrap()
    }

    fn plaintext_metadata(bytes: &[u8]) -> AttachmentBlobPlaintextMetadata {
        AttachmentBlobPlaintextMetadata::new(bytes.len() as u64, Sha256::digest(bytes).into())
    }

    fn seal(bytes: &[u8]) -> (Vec<u8>, AttachmentBlobMetadata) {
        let mut source = Cursor::new(bytes);
        let mut ciphertext = Vec::new();
        let metadata = key(7)
            .seal_attachment_blob(
                &context(),
                &mut source,
                &mut ciphertext,
                &plaintext_metadata(bytes),
            )
            .unwrap();
        (ciphertext, metadata)
    }

    fn open(ciphertext: &[u8], metadata: &AttachmentBlobMetadata) -> AttachmentBlobResult<Vec<u8>> {
        let mut source = Cursor::new(ciphertext);
        let mut plaintext = Vec::new();
        key(7).open_attachment_blob(&context(), &mut source, &mut plaintext, metadata)?;
        Ok(plaintext)
    }

    #[test]
    fn multi_chunk_blob_round_trips_with_both_hashes() {
        let plaintext = patterned_bytes(ATTACHMENT_BLOB_CHUNK_SIZE + 731);
        let (ciphertext, metadata) = seal(&plaintext);

        let opened = open(&ciphertext, &metadata).unwrap();
        let expected_ciphertext_sha256: [u8; 32] = Sha256::digest(&ciphertext).into();

        assert_eq!(opened, plaintext);
        assert_eq!(metadata.version, VERSION);
        assert_eq!(metadata.plaintext, plaintext_metadata(&plaintext));
        assert_eq!(metadata.ciphertext.size_bytes, ciphertext.len() as u64);
        assert_eq!(metadata.ciphertext.sha256, expected_ciphertext_sha256);
        assert!(!contains_bytes(
            &ciphertext,
            context().workspace_id().as_bytes()
        ));
        assert!(!contains_bytes(
            &ciphertext,
            context().attachment_id().as_bytes()
        ));
        assert!(!contains_bytes(&ciphertext, &plaintext[..64]));
        assert_eq!(metadata.plaintext.sha256_hex().len(), 64);
        assert_eq!(
            AttachmentBlobCiphertextMetadata::from_hex(
                metadata.ciphertext.size_bytes,
                &metadata.ciphertext.sha256_hex(),
            )
            .unwrap(),
            metadata.ciphertext,
        );
    }

    #[test]
    fn empty_blob_round_trips_without_chunks() {
        let (ciphertext, metadata) = seal(&[]);

        assert_eq!(open(&ciphertext, &metadata).unwrap(), Vec::<u8>::new());
        assert_eq!(metadata.plaintext.size_bytes, 0);
        assert_eq!(
            ciphertext.len() as u64,
            encoded_size(&context(), key(7).key_id(), 0).unwrap()
        );
    }

    #[test]
    fn ciphertext_size_is_available_before_object_reservation() {
        for plaintext_size in [
            0,
            1,
            ATTACHMENT_BLOB_CHUNK_SIZE as u64,
            ATTACHMENT_BLOB_CHUNK_SIZE as u64 + 1,
            ATTACHMENT_BLOB_MAX_PLAINTEXT_BYTES,
        ] {
            assert_eq!(
                key(7)
                    .attachment_blob_ciphertext_size(
                        context().workspace_id(),
                        context().attachment_id(),
                        plaintext_size,
                    )
                    .unwrap(),
                encoded_size(&context(), key(7).key_id(), plaintext_size).unwrap(),
            );
        }
    }

    #[test]
    fn repeated_seals_use_fresh_header_and_chunk_nonces() {
        let plaintext = patterned_bytes(ATTACHMENT_BLOB_CHUNK_SIZE + 1);
        let (first, first_metadata) = seal(&plaintext);
        let (second, second_metadata) = seal(&plaintext);

        assert_ne!(first, second);
        assert_ne!(
            first_metadata.ciphertext.sha256,
            second_metadata.ciphertext.sha256
        );
        assert_eq!(open(&first, &first_metadata).unwrap(), plaintext);
        assert_eq!(open(&second, &second_metadata).unwrap(), plaintext);

        let keys = derive_blob_keys(&key(7), &context()).unwrap();
        assert_ne!(keys.header.as_slice(), keys.chunk.as_slice());
    }

    #[test]
    fn blind_backup_refs_are_stable_opaque_and_domain_separated() {
        let workspace_key = key(7);
        let plaintext = plaintext_metadata(b"payload");
        let attachment_ref = workspace_key
            .blind_attachment_backup_ref("workspace-a", "attachment-1")
            .unwrap();
        let version_ref = workspace_key
            .blind_attachment_backup_version_ref("workspace-a", "attachment-1", &plaintext)
            .unwrap();

        assert_eq!(attachment_ref.len(), 43);
        assert_eq!(version_ref.len(), 43);
        assert_eq!(URL_SAFE_NO_PAD.decode(&attachment_ref).unwrap().len(), 32);
        assert_eq!(URL_SAFE_NO_PAD.decode(&version_ref).unwrap().len(), 32);
        assert_ne!(attachment_ref, version_ref);
        assert_eq!(
            workspace_key
                .blind_attachment_backup_ref("workspace-a", "attachment-1")
                .unwrap(),
            attachment_ref
        );
        assert_eq!(
            workspace_key
                .blind_attachment_backup_version_ref("workspace-a", "attachment-1", &plaintext,)
                .unwrap(),
            version_ref
        );
    }

    #[test]
    fn blind_backup_refs_change_with_identity_key_content_and_size() {
        let workspace_key = key(7);
        let attachment_ref = workspace_key
            .blind_attachment_backup_ref("workspace-a", "attachment-1")
            .unwrap();
        assert_ne!(
            workspace_key
                .blind_attachment_backup_ref("workspace-b", "attachment-1")
                .unwrap(),
            attachment_ref
        );
        assert_ne!(
            workspace_key
                .blind_attachment_backup_ref("workspace-a", "attachment-2")
                .unwrap(),
            attachment_ref
        );
        assert_ne!(
            key(8)
                .blind_attachment_backup_ref("workspace-a", "attachment-1")
                .unwrap(),
            attachment_ref
        );

        let original = plaintext_metadata(b"payload");
        let original_ref = workspace_key
            .blind_attachment_backup_version_ref("workspace-a", "attachment-1", &original)
            .unwrap();
        let changed_hash = AttachmentBlobPlaintextMetadata::new(original.size_bytes, [9; 32]);
        let changed_size =
            AttachmentBlobPlaintextMetadata::new(original.size_bytes + 1, original.sha256);
        assert_ne!(
            workspace_key
                .blind_attachment_backup_version_ref("workspace-a", "attachment-1", &changed_hash,)
                .unwrap(),
            original_ref
        );
        assert_ne!(
            workspace_key
                .blind_attachment_backup_version_ref("workspace-a", "attachment-1", &changed_size,)
                .unwrap(),
            original_ref
        );

        let other_object = AttachmentBlobContext::new(
            "workspace-a",
            "attachment-1",
            "019f6b9d-5ca3-7e61-8414-2be0ad5d9713",
        )
        .unwrap();
        assert_eq!(
            workspace_key
                .blind_attachment_backup_ref(
                    other_object.workspace_id(),
                    other_object.attachment_id(),
                )
                .unwrap(),
            attachment_ref
        );
    }

    #[test]
    fn blind_backup_refs_validate_identity_and_plaintext_size() {
        let workspace_key = key(7);
        let plaintext = plaintext_metadata(b"payload");
        assert!(
            workspace_key
                .blind_attachment_backup_ref("", "attachment-1")
                .is_err()
        );
        assert!(
            workspace_key
                .blind_attachment_backup_ref("workspace-a", " attachment-1")
                .is_err()
        );
        assert!(
            workspace_key
                .blind_attachment_backup_ref(
                    "workspace-a",
                    &"a".repeat(MAX_ATTACHMENT_ID_BYTES + 1),
                )
                .is_err()
        );
        assert!(
            workspace_key
                .blind_attachment_backup_version_ref(
                    "workspace-a",
                    "attachment-1",
                    &AttachmentBlobPlaintextMetadata::new(
                        ATTACHMENT_BLOB_MAX_PLAINTEXT_BYTES + 1,
                        plaintext.sha256,
                    ),
                )
                .is_err()
        );
    }

    #[test]
    fn header_and_chunk_tampering_fail_authentication() {
        let plaintext = patterned_bytes(ATTACHMENT_BLOB_CHUNK_SIZE + 1);
        let (ciphertext, metadata) = seal(&plaintext);
        let chunk_start = chunk_start(&ciphertext);

        let mut header_tampered = ciphertext.clone();
        header_tampered[FIXED_PREFIX_BYTES] ^= 0x80;
        assert!(matches!(
            open(&header_tampered, &metadata),
            Err(AttachmentBlobError::AuthenticationFailed)
        ));

        let mut chunk_tampered = ciphertext;
        chunk_tampered[chunk_start] ^= 0x80;
        assert!(matches!(
            open(&chunk_tampered, &metadata),
            Err(AttachmentBlobError::AuthenticationFailed)
        ));
    }

    #[test]
    fn truncation_and_trailing_data_are_rejected() {
        let (mut ciphertext, metadata) = seal(b"payload");
        let last = ciphertext.pop().unwrap();
        assert!(matches!(
            open(&ciphertext, &metadata),
            Err(AttachmentBlobError::Truncated)
        ));

        ciphertext.push(last);
        ciphertext.push(0);
        assert!(matches!(
            open(&ciphertext, &metadata),
            Err(AttachmentBlobError::TrailingData)
        ));
    }

    #[test]
    fn reordered_chunks_are_rejected() {
        let plaintext = patterned_bytes(ATTACHMENT_BLOB_CHUNK_SIZE * 2);
        let (mut ciphertext, metadata) = seal(&plaintext);
        let first_start = chunk_start(&ciphertext);
        let chunk_ciphertext_size = ATTACHMENT_BLOB_CHUNK_SIZE + AEAD_TAG_BYTES as usize;
        let second_start = first_start + chunk_ciphertext_size;
        for offset in 0..chunk_ciphertext_size {
            ciphertext.swap(first_start + offset, second_start + offset);
        }

        assert!(matches!(
            open(&ciphertext, &metadata),
            Err(AttachmentBlobError::AuthenticationFailed)
        ));
    }

    #[test]
    fn wrong_context_key_and_version_are_rejected() {
        let (mut ciphertext, metadata) = seal(b"secret");

        for wrong_context in [
            AttachmentBlobContext::new("workspace-b", "attachment-1", OBJECT_ID).unwrap(),
            AttachmentBlobContext::new("workspace-a", "attachment-2", OBJECT_ID).unwrap(),
            AttachmentBlobContext::new(
                "workspace-a",
                "attachment-1",
                "019f6b9d-5ca3-7e61-8414-2be0ad5d9713",
            )
            .unwrap(),
        ] {
            let mut source = Cursor::new(&ciphertext);
            let mut plaintext = Vec::new();
            assert!(
                key(7)
                    .open_attachment_blob(&wrong_context, &mut source, &mut plaintext, &metadata,)
                    .is_err()
            );
        }

        let mut source = Cursor::new(&ciphertext);
        let mut plaintext = Vec::new();
        assert!(
            key(8)
                .open_attachment_blob(&context(), &mut source, &mut plaintext, &metadata,)
                .is_err()
        );

        ciphertext[MAGIC.len()] = 2;
        assert!(matches!(
            open(&ciphertext, &metadata),
            Err(AttachmentBlobError::UnsupportedVersion)
        ));
    }

    #[test]
    fn source_size_and_hash_mismatches_are_rejected() {
        let bytes = b"payload";
        let mut destination = Vec::new();
        let mut shorter_source = Cursor::new(bytes);
        let too_large = AttachmentBlobPlaintextMetadata::new(
            bytes.len() as u64 + 1,
            Sha256::digest(bytes).into(),
        );
        assert!(matches!(
            key(7).seal_attachment_blob(
                &context(),
                &mut shorter_source,
                &mut destination,
                &too_large,
            ),
            Err(AttachmentBlobError::SourceMismatch)
        ));

        destination.clear();
        let mut longer_source = Cursor::new(bytes);
        let too_small = AttachmentBlobPlaintextMetadata::new(
            bytes.len() as u64 - 1,
            Sha256::digest(&bytes[..bytes.len() - 1]).into(),
        );
        assert!(matches!(
            key(7).seal_attachment_blob(
                &context(),
                &mut longer_source,
                &mut destination,
                &too_small,
            ),
            Err(AttachmentBlobError::SourceMismatch)
        ));

        destination.clear();
        let mut source = Cursor::new(bytes);
        let wrong_hash = AttachmentBlobPlaintextMetadata::new(bytes.len() as u64, [0; 32]);
        assert!(matches!(
            key(7).seal_attachment_blob(&context(), &mut source, &mut destination, &wrong_hash,),
            Err(AttachmentBlobError::SourceMismatch)
        ));
    }

    #[test]
    fn ciphertext_digest_mismatch_is_rejected() {
        let (ciphertext, mut metadata) = seal(b"payload");
        metadata.ciphertext.sha256 = [0; 32];

        assert!(matches!(
            open(&ciphertext, &metadata),
            Err(AttachmentBlobError::CiphertextMismatch)
        ));
    }

    #[test]
    fn plaintext_limit_is_enforced_before_reading() {
        let metadata =
            AttachmentBlobPlaintextMetadata::new(ATTACHMENT_BLOB_MAX_PLAINTEXT_BYTES + 1, [0; 32]);
        let mut source = Cursor::new(Vec::<u8>::new());
        let mut destination = Vec::new();

        assert!(matches!(
            key(7).seal_attachment_blob(&context(), &mut source, &mut destination, &metadata,),
            Err(AttachmentBlobError::PlaintextTooLarge)
        ));
        assert!(destination.is_empty());

        let (ciphertext, mut blob_metadata) = seal(&[]);
        blob_metadata.plaintext.size_bytes = ATTACHMENT_BLOB_MAX_PLAINTEXT_BYTES + 1;
        let mut source = Cursor::new(ciphertext);
        let mut destination = Vec::new();
        assert!(matches!(
            key(7).open_attachment_blob(&context(), &mut source, &mut destination, &blob_metadata,),
            Err(AttachmentBlobError::PlaintextTooLarge)
        ));
        assert_eq!(source.position(), 0);
        assert!(destination.is_empty());
        assert_eq!(
            chunk_count(ATTACHMENT_BLOB_MAX_PLAINTEXT_BYTES),
            (ATTACHMENT_BLOB_MAX_PLAINTEXT_BYTES / ATTACHMENT_BLOB_CHUNK_SIZE as u64) as u32
        );
        assert!(
            encoded_size(
                &context(),
                key(7).key_id(),
                ATTACHMENT_BLOB_MAX_PLAINTEXT_BYTES,
            )
            .is_ok()
        );
    }

    #[test]
    fn context_and_hex_metadata_are_strict() {
        assert!(AttachmentBlobContext::new("", "attachment", OBJECT_ID).is_err());
        assert!(AttachmentBlobContext::new("workspace", " attachment", OBJECT_ID).is_err());
        assert!(
            AttachmentBlobContext::new(
                "workspace",
                "attachment",
                "019F6B9D-5CA3-7E61-8414-2BE0AD5D9712",
            )
            .is_err()
        );
        assert!(
            AttachmentBlobContext::new(
                "workspace",
                "attachment",
                "00000000-0000-0000-0000-000000000000",
            )
            .is_err()
        );
        assert!(AttachmentBlobPlaintextMetadata::from_hex(1, "not-a-hash").is_err());
        assert!(
            AttachmentBlobPlaintextMetadata::from_hex(
                1,
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            )
            .is_err()
        );
    }

    fn patterned_bytes(length: usize) -> Vec<u8> {
        (0..length).map(|index| (index % 251) as u8).collect()
    }

    fn chunk_start(ciphertext: &[u8]) -> usize {
        let header_length = u32::from_be_bytes(
            ciphertext[FIXED_PREFIX_BYTES - 4..FIXED_PREFIX_BYTES]
                .try_into()
                .unwrap(),
        ) as usize;
        FIXED_PREFIX_BYTES + header_length
    }

    fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    fn base64_url_no_pad(bytes: &[u8]) -> String {
        use base64::Engine as _;

        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    }
}
