use super::*;

fn delete_source_record(cloud_sync_enabled: i64) -> DeleteSourcePreflight {
    DeleteSourcePreflight {
        attachment_id: "attachment-1".to_string(),
        session_id: "session-1".to_string(),
        workspace_id: "workspace-1".to_string(),
        expected_sha256: "a".repeat(64),
        expected_size_bytes: 1,
        object_key: "owner/object.anb1".to_string(),
        cache_id: String::new(),
        ciphertext_sha256: String::new(),
        ciphertext_size_bytes: 0,
        current_attachment_id: Some("attachment-1".to_string()),
        relative_path: Some("attachments/file.bin".to_string()),
        source_type: Some("note_upload".to_string()),
        attachment_sha256: Some("a".repeat(64)),
        attachment_size_bytes: Some(1),
        attachment_cloud_object_key: Some("owner/object.anb1".to_string()),
        cloud_sync_enabled: Some(cloud_sync_enabled),
        deleted_at: None,
        local_availability: "present".to_string(),
    }
}

#[test]
fn only_an_exact_current_delete_source_requires_hashing() {
    assert!(
        delete_source_attachment(&delete_source_record(0))
            .unwrap()
            .is_some()
    );
    assert!(
        delete_source_attachment(&delete_source_record(1))
            .unwrap()
            .is_some()
    );

    let mut missing = delete_source_record(0);
    missing.current_attachment_id = None;
    assert!(delete_source_attachment(&missing).unwrap().is_none());

    let mut deleted = delete_source_record(0);
    deleted.deleted_at = Some("2026-07-18T00:00:00.000Z".to_string());
    deleted.relative_path = None;
    assert!(delete_source_attachment(&deleted).unwrap().is_none());

    let mut replaced = delete_source_record(0);
    replaced.attachment_sha256 = Some("b".repeat(64));
    assert!(delete_source_attachment(&replaced).unwrap().is_none());

    let mut other_object = delete_source_record(0);
    other_object.attachment_cloud_object_key = Some("owner/other.anb1".to_string());
    assert!(delete_source_attachment(&other_object).unwrap().is_none());
}

#[test]
fn exact_remote_dependency_blocks_delete_commit() {
    let mut enabled = delete_source_record(1);
    assert!(exact_delete_dependency(&enabled).unwrap());

    enabled.cloud_sync_enabled = Some(0);
    enabled.local_availability = "absent".to_string();
    assert!(exact_delete_dependency(&enabled).unwrap());

    enabled.local_availability = "present".to_string();
    assert!(!exact_delete_dependency(&enabled).unwrap());

    enabled.attachment_cloud_object_key = Some("owner/newer.anb1".to_string());
    assert!(!exact_delete_dependency(&enabled).unwrap());
}

#[test]
fn delete_backup_refs_are_stable_for_persisted_job_metadata() {
    let key = anlg_e2ee::RecoveryKey::generate()
        .unwrap()
        .workspace_key("workspace-1")
        .unwrap();
    let expected = plaintext_metadata(&"a".repeat(64), 42).unwrap();
    let first = attachment_backup_refs(&key, "workspace-1", "attachment-1", &expected).unwrap();
    let retry = attachment_backup_refs(&key, "workspace-1", "attachment-1", &expected).unwrap();
    assert_eq!(first, retry);

    let newer = plaintext_metadata(&"b".repeat(64), 42).unwrap();
    let newer = attachment_backup_refs(&key, "workspace-1", "attachment-1", &newer).unwrap();
    assert_eq!(first.0, newer.0);
    assert_ne!(first.1, newer.1);
}

#[test]
fn delete_source_hash_detects_missing_and_changed_local_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("attachment.bin");
    let bytes = b"preserved attachment";
    let sha256 = hex_digest(Sha256::digest(bytes).as_slice());

    assert!(!file_matches(&path, bytes.len() as u64, &sha256).unwrap());
    std::fs::write(&path, bytes).unwrap();
    assert!(file_matches(&path, bytes.len() as u64, &sha256).unwrap());
    std::fs::write(&path, b"different attachment").unwrap();
    assert!(!file_matches(&path, bytes.len() as u64, &sha256).unwrap());
}

#[test]
fn delete_guard_restores_canonical_bytes_and_preserves_a_conflict() {
    let directory = tempfile::tempdir().unwrap();
    let source_path = directory.path().join("source.bin");
    let guard_path = directory.path().join(format!("{}.anb1", Uuid::new_v4()));
    let destination_path = directory.path().join("destination.bin");
    let canonical = b"canonical private attachment";
    let local_edit = b"different local attachment";
    std::fs::write(&source_path, canonical).unwrap();

    let key = anlg_e2ee::RecoveryKey::generate()
        .unwrap()
        .workspace_key("workspace-a")
        .unwrap();
    let context =
        AttachmentBlobContext::new("workspace-a", "attachment-a", Uuid::new_v4().to_string())
            .unwrap();
    let expected = AttachmentBlobPlaintextMetadata::from_hex(
        canonical.len() as u64,
        &hex_digest(Sha256::digest(canonical).as_slice()),
    )
    .unwrap();
    let (metadata, guard) = seal_delete_guard(
        &key,
        &context,
        &source_path,
        &guard_path,
        &expected,
        &tokio_util::sync::CancellationToken::new(),
    )
    .unwrap();
    guard.disarm();

    assert_ne!(std::fs::read(&guard_path).unwrap(), canonical);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&guard_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    std::fs::write(&source_path, b"source changed after delete began").unwrap();
    std::fs::write(&destination_path, local_edit).unwrap();
    let staged = stage_delete_guard_restore(
        &key,
        &context,
        &guard_path,
        directory.path(),
        &metadata,
        &tokio_util::sync::CancellationToken::new(),
    )
    .unwrap();
    let conflicts = reconcile_staged_delete_guard(
        staged,
        &destination_path,
        canonical.len() as u64,
        &expected.sha256_hex(),
    )
    .unwrap();

    assert_eq!(std::fs::read(&destination_path).unwrap(), canonical);
    assert_eq!(conflicts.len(), 1);
    assert_eq!(std::fs::read(&conflicts[0]).unwrap(), local_edit);

    let staged = stage_delete_guard_restore(
        &key,
        &context,
        &guard_path,
        directory.path(),
        &metadata,
        &tokio_util::sync::CancellationToken::new(),
    )
    .unwrap();
    assert!(
        reconcile_staged_delete_guard(
            staged,
            &destination_path,
            canonical.len() as u64,
            &expected.sha256_hex(),
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn delete_guard_restores_a_missing_destination() {
    let directory = tempfile::tempdir().unwrap();
    let source_path = directory.path().join("source.bin");
    let guard_path = directory.path().join(format!("{}.anb1", Uuid::new_v4()));
    let destination_path = directory.path().join("missing.bin");
    let canonical = b"attachment recovered after remote delete";
    std::fs::write(&source_path, canonical).unwrap();
    let key = anlg_e2ee::RecoveryKey::generate()
        .unwrap()
        .workspace_key("workspace-a")
        .unwrap();
    let context =
        AttachmentBlobContext::new("workspace-a", "attachment-a", Uuid::new_v4().to_string())
            .unwrap();
    let expected = AttachmentBlobPlaintextMetadata::from_hex(
        canonical.len() as u64,
        &hex_digest(Sha256::digest(canonical).as_slice()),
    )
    .unwrap();
    let (metadata, guard) = seal_delete_guard(
        &key,
        &context,
        &source_path,
        &guard_path,
        &expected,
        &tokio_util::sync::CancellationToken::new(),
    )
    .unwrap();
    guard.disarm();
    let staged = stage_delete_guard_restore(
        &key,
        &context,
        &guard_path,
        directory.path(),
        &metadata,
        &tokio_util::sync::CancellationToken::new(),
    )
    .unwrap();

    assert!(
        reconcile_staged_delete_guard(
            staged,
            &destination_path,
            canonical.len() as u64,
            &expected.sha256_hex(),
        )
        .unwrap()
        .is_empty()
    );
    assert_eq!(std::fs::read(destination_path).unwrap(), canonical);
}

#[test]
fn delete_guard_retry_keeps_the_plaintext_stage() {
    let directory = tempfile::tempdir().unwrap();
    let blocked_parent = directory.path().join("blocked-parent");
    let destination = blocked_parent.join("attachment.bin");
    let canonical = b"attachment retained across filesystem retry";
    let expected_sha256 = hex_digest(Sha256::digest(canonical).as_slice());
    let staged = tempfile::NamedTempFile::new_in(directory.path()).unwrap();
    std::fs::write(staged.path(), canonical).unwrap();
    std::fs::write(&blocked_parent, b"not a directory").unwrap();

    let (staged, conflicts) = match reconcile_staged_delete_guard_once(
        staged,
        &destination,
        canonical.len() as u64,
        &expected_sha256,
        Vec::new(),
    )
    .unwrap()
    {
        DeleteGuardReconcile::Retry {
            staged,
            conflicts,
            error: Error::Io(_),
        } => (staged, conflicts),
        _ => panic!("blocked parent should produce a retryable filesystem error"),
    };
    assert!(staged.path().exists());

    std::fs::remove_file(blocked_parent).unwrap();
    let conflicts = match reconcile_staged_delete_guard_once(
        staged,
        &destination,
        canonical.len() as u64,
        &expected_sha256,
        conflicts,
    )
    .unwrap()
    {
        DeleteGuardReconcile::Ready(conflicts) => conflicts,
        DeleteGuardReconcile::Retry { .. } => panic!("retry should restore the attachment"),
    };

    assert!(conflicts.is_empty());
    assert_eq!(std::fs::read(destination).unwrap(), canonical);
}

#[test]
fn cancelled_delete_guard_seal_removes_partial_ciphertext() {
    let directory = tempfile::tempdir().unwrap();
    let source_path = directory.path().join("source.bin");
    let guard_path = directory.path().join(format!("{}.anb1", Uuid::new_v4()));
    let canonical = b"attachment whose delete was cancelled";
    std::fs::write(&source_path, canonical).unwrap();
    let key = anlg_e2ee::RecoveryKey::generate()
        .unwrap()
        .workspace_key("workspace-a")
        .unwrap();
    let context =
        AttachmentBlobContext::new("workspace-a", "attachment-a", Uuid::new_v4().to_string())
            .unwrap();
    let expected = AttachmentBlobPlaintextMetadata::from_hex(
        canonical.len() as u64,
        &hex_digest(Sha256::digest(canonical).as_slice()),
    )
    .unwrap();
    let cancellation = tokio_util::sync::CancellationToken::new();
    cancellation.cancel();

    assert!(matches!(
        seal_delete_guard(
            &key,
            &context,
            &source_path,
            &guard_path,
            &expected,
            &cancellation,
        ),
        Err(Error::Cancelled)
    ));
    assert!(!guard_path.exists());
}

#[test]
fn cancelled_delete_guard_restore_removes_plaintext_stage() {
    let directory = tempfile::tempdir().unwrap();
    let source_path = directory.path().join("source.bin");
    let guard_path = directory.path().join(format!("{}.anb1", Uuid::new_v4()));
    let canonical = b"attachment whose restore was cancelled";
    std::fs::write(&source_path, canonical).unwrap();
    let key = anlg_e2ee::RecoveryKey::generate()
        .unwrap()
        .workspace_key("workspace-a")
        .unwrap();
    let context =
        AttachmentBlobContext::new("workspace-a", "attachment-a", Uuid::new_v4().to_string())
            .unwrap();
    let expected = AttachmentBlobPlaintextMetadata::from_hex(
        canonical.len() as u64,
        &hex_digest(Sha256::digest(canonical).as_slice()),
    )
    .unwrap();
    let (metadata, guard) = seal_delete_guard(
        &key,
        &context,
        &source_path,
        &guard_path,
        &expected,
        &tokio_util::sync::CancellationToken::new(),
    )
    .unwrap();
    guard.disarm();
    let cancellation = tokio_util::sync::CancellationToken::new();
    cancellation.cancel();

    assert!(matches!(
        stage_delete_guard_restore(
            &key,
            &context,
            &guard_path,
            directory.path(),
            &metadata,
            &cancellation,
        ),
        Err(Error::Cancelled)
    ));
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
}

#[test]
fn recoverable_delete_guard_drift_uses_a_retryable_message() {
    let message = Error::DeleteGuardChanged.to_string().to_ascii_lowercase();
    for permanent_marker in ["invalid", "mismatch", "path", "source"] {
        assert!(!message.contains(permanent_marker));
    }
}

#[tokio::test]
async fn delete_guard_directory_is_owner_only() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("delete-guards");

    create_delete_guard_root(&root).await.unwrap();
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
fn delete_guard_reconciliation_retains_only_live_references() {
    let directory = tempfile::tempdir().unwrap();
    let live = Uuid::new_v4().to_string();
    let orphan = Uuid::new_v4().to_string();
    std::fs::write(directory.path().join(format!("{live}.anb1")), b"live").unwrap();
    std::fs::write(directory.path().join(format!("{orphan}.anb1")), b"orphan").unwrap();
    std::fs::write(directory.path().join("malformed.anb1"), b"malformed").unwrap();

    assert_eq!(
        reconcile_delete_guard_files(
            directory.path(),
            &HashSet::from([live.clone()]),
            SystemTime::now() + Duration::from_secs(1),
        )
        .unwrap(),
        2
    );
    assert!(directory.path().join(format!("{live}.anb1")).is_file());
    assert!(!directory.path().join(format!("{orphan}.anb1")).exists());
    assert!(!directory.path().join("malformed.anb1").exists());
}

#[test]
fn delete_guard_reconciliation_does_not_race_a_new_guard() {
    let directory = tempfile::tempdir().unwrap();
    let guard_id = Uuid::new_v4().to_string();
    let guard_path = directory.path().join(format!("{guard_id}.anb1"));
    std::fs::write(&guard_path, b"new unlinked guard").unwrap();

    assert_eq!(
        reconcile_delete_guard_files(
            directory.path(),
            &HashSet::new(),
            SystemTime::now() - DELETE_GUARD_ORPHAN_GRACE,
        )
        .unwrap(),
        0
    );
    assert!(guard_path.is_file());
}
