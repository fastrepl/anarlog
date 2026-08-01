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
            .blind_attachment_backup_ref(other_object.workspace_id(), other_object.attachment_id(),)
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
            .blind_attachment_backup_ref("workspace-a", &"a".repeat(MAX_ATTACHMENT_ID_BYTES + 1),)
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
    let too_large =
        AttachmentBlobPlaintextMetadata::new(bytes.len() as u64 + 1, Sha256::digest(bytes).into());
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
        key(7).seal_attachment_blob(&context(), &mut longer_source, &mut destination, &too_small,),
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
