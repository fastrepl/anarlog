use super::*;

fn transfer_record(cloud_sync_enabled: i64) -> TransferAttachment {
    TransferAttachment {
        job_id: "job-1".to_string(),
        attachment_id: "attachment-1".to_string(),
        session_id: "session-1".to_string(),
        workspace_id: "workspace-1".to_string(),
        expected_sha256: "a".repeat(64),
        expected_size_bytes: 1,
        ciphertext_sha256: String::new(),
        ciphertext_size_bytes: 0,
        remote_object_id: String::new(),
        object_key: String::new(),
        cache_id: String::new(),
        phase: "preparing".to_string(),
        relative_path: "attachments/file.bin".to_string(),
        source_type: "note_upload".to_string(),
        attachment_sha256: "a".repeat(64),
        attachment_size_bytes: 1,
        attachment_cloud_object_key: String::new(),
        cloud_sync_enabled,
    }
}

#[test]
fn disabled_cloud_sync_is_restore_only() {
    let disabled = transfer_record(0);
    assert!(validate_transfer_version(&disabled).is_ok());
    assert!(validate_upload_transfer_version(&disabled).is_err());

    let enabled = transfer_record(1);
    assert!(validate_transfer_version(&enabled).is_ok());
    assert!(validate_upload_transfer_version(&enabled).is_ok());
}

#[test]
fn shared_upload_snapshot_stays_stable_when_the_source_changes() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("attachment.bin");
    let snapshot = directory.path().join("snapshot.bin");
    let bytes = b"stable shared attachment";
    let sha256 = hex_digest(Sha256::digest(bytes).as_slice());
    std::fs::write(&source, bytes).unwrap();

    snapshot_verified_file(
        &source,
        &snapshot,
        bytes.len() as u64,
        &sha256,
        &tokio_util::sync::CancellationToken::new(),
    )
    .unwrap()
    .disarm();
    std::fs::write(&source, vec![b'x'; bytes.len()]).unwrap();

    assert_eq!(std::fs::read(&snapshot).unwrap(), bytes);
    assert!(file_matches(&snapshot, bytes.len() as u64, &sha256).unwrap());
    assert!(!file_matches(&source, bytes.len() as u64, &sha256).unwrap());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&snapshot).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn shared_upload_snapshot_rejects_and_removes_unregistered_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("attachment.bin");
    let snapshot = directory.path().join("snapshot.bin");
    let expected = b"expected shared attachment";
    let sha256 = hex_digest(Sha256::digest(expected).as_slice());
    std::fs::write(&source, vec![b'x'; expected.len()]).unwrap();

    assert!(matches!(
        snapshot_verified_file(
            &source,
            &snapshot,
            expected.len() as u64,
            &sha256,
            &tokio_util::sync::CancellationToken::new(),
        ),
        Err(Error::ChecksumMismatch)
    ));
    assert!(!snapshot.exists());
}

#[test]
fn cancelled_shared_upload_snapshot_leaves_no_plaintext_cache() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("attachment.bin");
    let snapshot = directory.path().join("snapshot.bin");
    let bytes = b"cancelled shared attachment";
    let sha256 = hex_digest(Sha256::digest(bytes).as_slice());
    let cancellation = tokio_util::sync::CancellationToken::new();
    cancellation.cancel();
    std::fs::write(&source, bytes).unwrap();

    assert!(matches!(
        snapshot_verified_file(
            &source,
            &snapshot,
            bytes.len() as u64,
            &sha256,
            &cancellation,
        ),
        Err(Error::Cancelled)
    ));
    assert!(!snapshot.exists());
}

#[test]
fn shared_upload_cleanup_is_idempotent() {
    let directory = tempfile::tempdir().unwrap();
    let snapshot = directory.path().join("snapshot.bin");
    std::fs::write(&snapshot, b"shared attachment").unwrap();

    assert!(cleanup_shared_upload_path(&snapshot).unwrap());
    assert!(!cleanup_shared_upload_path(&snapshot).unwrap());
}

#[tokio::test]
async fn shared_upload_cache_directory_is_owner_only() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("shared-upload");

    create_shared_upload_cache_root(&root).await.unwrap();
    assert!(root.is_dir());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(root).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
}

#[test]
fn validates_upload_ranges() {
    assert!(validate_range(0, 1).is_ok());
    assert!(validate_range(0, MAX_RANGE_BYTES).is_ok());
    assert!(validate_range(0, 0).is_err());
    assert!(validate_range(2, 1).is_err());
    assert!(validate_range(0, MAX_RANGE_BYTES + 1).is_err());
}

#[test]
fn only_accepts_expected_attachment_paths() {
    assert!(validate_attachment_relative_path("note_upload", "attachments/image.png").is_ok());
    assert!(validate_attachment_relative_path("session_audio", "audio.wav").is_ok());
    assert!(validate_attachment_relative_path("session_audio", "attachments/audio.wav").is_err());
    assert!(validate_attachment_relative_path("note_upload", "../image.png").is_err());
    assert!(validate_attachment_relative_path("note_upload", "attachments/a/b.png").is_err());
}

#[test]
fn scoped_cache_names_do_not_expose_ids() {
    let first = hash_identifier("share/../../secret");
    let second = hash_identifier("share/../../secret");
    assert_eq!(first, second);
    assert_eq!(first.len(), 64);
    assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert!(!first.contains("secret"));
    assert_ne!(
        shared_cache_id("viewer-a", "attachment-a"),
        shared_cache_id("viewer-b", "attachment-a")
    );
}

#[test]
fn shared_cache_requires_matching_sidecar_and_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let cache_id = shared_cache_id("viewer-a", "attachment-a");
    let bytes = b"shared attachment";
    let sha256 = hex_digest(Sha256::digest(bytes).as_slice());
    std::fs::write(
        cached_shared_attachment_path(directory.path(), &cache_id),
        bytes,
    )
    .unwrap();
    write_shared_cache_metadata(directory.path(), &cache_id, bytes.len() as u64, &sha256).unwrap();
    assert!(
        shared_cache_matches(directory.path(), &cache_id, bytes.len() as u64, &sha256).unwrap()
    );

    std::fs::write(
        cached_shared_attachment_path(directory.path(), &cache_id),
        b"tampered attachment",
    )
    .unwrap();
    assert!(
        !shared_cache_matches(directory.path(), &cache_id, bytes.len() as u64, &sha256).unwrap()
    );
}

#[test]
fn shared_cache_removes_plaintext_when_sidecar_commit_fails() {
    let directory = tempfile::tempdir().unwrap();
    let cache_id = shared_cache_id("viewer-a", "attachment-a");
    let data_path = cached_shared_attachment_path(directory.path(), &cache_id);
    std::fs::write(&data_path, b"shared attachment").unwrap();
    std::fs::create_dir(shared_cache_metadata_path(directory.path(), &cache_id)).unwrap();

    assert!(
        commit_shared_cache_entry(directory.path(), &cache_id, &data_path, 17, &"a".repeat(64),)
            .is_err()
    );
    assert!(!data_path.exists());
}

#[test]
fn startup_cleanup_removes_an_entire_cache_root() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("shared");
    let orphan = root.join("orphaned-viewer");
    std::fs::create_dir_all(&orphan).unwrap();
    std::fs::write(orphan.join("attachment.bin"), b"plaintext").unwrap();

    clear_attachment_cache_directory(&root).unwrap();

    assert!(!root.exists());
    clear_attachment_cache_directory(&root).unwrap();
}

#[test]
fn cache_file_guard_removes_abandoned_files_only() {
    let directory = tempfile::tempdir().unwrap();
    let abandoned = directory.path().join("abandoned.anb1");
    std::fs::write(&abandoned, b"partial").unwrap();
    drop(CacheFileGuard::new(abandoned.clone()));
    assert!(!abandoned.exists());

    let retained = directory.path().join("retained.anb1");
    std::fs::write(&retained, b"complete").unwrap();
    CacheFileGuard::new(retained.clone()).disarm();
    assert!(retained.exists());
}

#[tokio::test]
async fn startup_cleanup_removes_only_preview_cache_scopes() {
    let directory = tempfile::tempdir().unwrap();
    let attachment_sync_root = directory.path().join("attachment-sync");
    let preview_root = attachment_sync_root.join("shared-preview");
    let durable_root = attachment_sync_root.join("shared");
    let orphan_scope = preview_root.join("orphan-scope");
    let durable_scope = durable_root.join("viewer-scope");
    std::fs::create_dir_all(&orphan_scope).unwrap();
    std::fs::create_dir_all(&durable_scope).unwrap();
    std::fs::write(orphan_scope.join("attachment.bin"), b"preview").unwrap();
    std::fs::write(durable_scope.join("attachment.bin"), b"durable").unwrap();

    assert!(
        clear_shared_attachment_preview_scopes_at(&preview_root)
            .await
            .unwrap()
    );
    assert!(!preview_root.exists());
    assert_eq!(
        std::fs::read(durable_scope.join("attachment.bin")).unwrap(),
        b"durable"
    );
    assert!(
        !clear_shared_attachment_preview_scopes_at(&preview_root)
            .await
            .unwrap()
    );
}
