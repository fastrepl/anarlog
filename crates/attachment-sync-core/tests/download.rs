use anlg_e2ee::{AttachmentBlobContext, AttachmentBlobPlaintextMetadata};
use attachment_sync_core::{
    DownloadObject, hex_digest, persist_staged_attachment, require_configured_supabase_url,
    seal_attachment_to_cache, stage_attachment_restore, validate_signed_download_url,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[test]
fn signed_urls_are_origin_and_path_bound() {
    let object_key =
        "00000000-0000-4000-8000-000000000001/00000000-0000-4000-8000-000000000002.anb1";
    let valid = format!(
        "https://project.supabase.co/storage/v1/object/sign/attachment-backups/{object_key}?token=secret"
    );
    assert!(
        validate_signed_download_url(
            &valid,
            "https://project.supabase.co",
            DownloadObject::Private(object_key),
        )
        .is_ok()
    );
    assert!(
        validate_signed_download_url(
            &valid,
            "https://other.supabase.co",
            DownloadObject::Private(object_key),
        )
        .is_err()
    );
    assert!(
        validate_signed_download_url(
            "http://project.supabase.co/storage/v1/object/sign/bucket/item?token=secret",
            "http://project.supabase.co",
            DownloadObject::Private(object_key),
        )
        .is_err()
    );
    assert!(
        validate_signed_download_url(
            &format!(
                "http://127.0.0.1:54321/storage/v1/object/sign/attachment-backups/{object_key}?token=secret"
            ),
            "http://127.0.0.1:54321",
            DownloadObject::Private(object_key),
        )
        .is_ok()
    );

    let attachment_id = "00000000-0000-4000-8000-000000000003";
    let shared = format!(
        "https://project.supabase.co/storage/v1/object/sign/shared-note-attachments/00000000-0000-4000-8000-000000000001/00000000-0000-4000-8000-000000000002/{attachment_id}.sna1?token=secret"
    );
    assert!(
        validate_signed_download_url(
            &shared,
            "https://project.supabase.co",
            DownloadObject::Shared(attachment_id),
        )
        .is_ok()
    );
    assert!(
        validate_signed_download_url(
            &shared.replace("shared-note-attachments", "other-bucket"),
            "https://project.supabase.co",
            DownloadObject::Shared(attachment_id),
        )
        .is_err()
    );
    assert!(
        validate_signed_download_url(
            &shared.replace(attachment_id, "00000000-0000-4000-8000-000000000004"),
            "https://project.supabase.co",
            DownloadObject::Shared(attachment_id),
        )
        .is_err()
    );
}

#[test]
fn missing_or_empty_configured_origins_fail_closed() {
    assert!(require_configured_supabase_url(None).is_err());
    assert!(require_configured_supabase_url(Some("")).is_err());
    assert!(require_configured_supabase_url(Some("   ")).is_err());
    assert_eq!(
        require_configured_supabase_url(Some("https://project.supabase.co")).unwrap(),
        "https://project.supabase.co"
    );
}

#[tokio::test]
async fn seals_and_restores_encrypted_attachment_atomically() {
    let directory = tempfile::tempdir().unwrap();
    let source_path = directory.path().join("source.bin");
    let cache_path = directory.path().join("cache.anb1");
    let destination_path = directory.path().join("destination.bin");
    let plaintext = b"private attachment bytes";
    std::fs::write(&source_path, plaintext).unwrap();
    std::fs::write(&destination_path, b"old bytes").unwrap();

    let key = anlg_e2ee::RecoveryKey::generate()
        .unwrap()
        .workspace_key("workspace-a")
        .unwrap();
    let context =
        AttachmentBlobContext::new("workspace-a", "attachment-a", Uuid::new_v4().to_string())
            .unwrap();
    let expected_plaintext = AttachmentBlobPlaintextMetadata::from_hex(
        plaintext.len() as u64,
        &hex_digest(Sha256::digest(plaintext).as_slice()),
    )
    .unwrap();
    let (metadata, cache_guard) = seal_attachment_to_cache(
        key.clone(),
        context.clone(),
        source_path,
        cache_path.clone(),
        expected_plaintext,
    )
    .await
    .unwrap();
    cache_guard.disarm();

    let staged = stage_attachment_restore(
        &key,
        &context,
        &cache_path,
        destination_path.parent().unwrap(),
        &metadata,
    )
    .unwrap();
    assert_eq!(std::fs::read(&destination_path).unwrap(), b"old bytes");
    persist_staged_attachment(staged, &destination_path).unwrap();
    assert_eq!(std::fs::read(destination_path).unwrap(), plaintext);
}
