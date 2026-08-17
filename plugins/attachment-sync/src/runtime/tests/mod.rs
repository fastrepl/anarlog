use super::*;

mod delete_guard;
mod transfer;

fn shared_upload_attachment(source_type: &str) -> SharedUploadAttachment {
    SharedUploadAttachment {
        attachment_id: "attachment-1".to_string(),
        session_id: "session-1".to_string(),
        workspace_id: "workspace-1".to_string(),
        relative_path: "audio.mp3".to_string(),
        source_type: source_type.to_string(),
        sha256: "a".repeat(64),
        size_bytes: 42,
        filename: "audio.mp3".to_string(),
        content_type: "audio/mpeg".to_string(),
        cloud_sync_enabled: 0,
        cloud_object_key: String::new(),
    }
}

#[test]
fn allows_local_session_audio_for_shared_uploads() {
    let attachment = shared_upload_attachment("session_audio");

    assert!(
        validate_shared_upload_version(
            &attachment,
            &attachment.sha256,
            42,
            &attachment.filename,
            &attachment.content_type,
            "",
        )
        .is_ok()
    );
}

#[test]
fn still_requires_private_backup_for_other_shared_uploads() {
    let attachment = shared_upload_attachment("note_upload");

    assert!(
        validate_shared_upload_version(
            &attachment,
            &attachment.sha256,
            42,
            &attachment.filename,
            &attachment.content_type,
            "",
        )
        .is_err()
    );
}
