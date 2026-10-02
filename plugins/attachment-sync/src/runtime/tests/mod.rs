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
fn shared_upload_requires_private_backup_except_session_audio() {
    for (source_type, should_allow_without_private_backup) in
        [("session_audio", true), ("note_upload", false)]
    {
        let attachment = shared_upload_attachment(source_type);
        assert_eq!(
            validate_shared_upload_version(
                &attachment,
                &attachment.sha256,
                42,
                &attachment.filename,
                &attachment.content_type,
                "",
            )
            .is_ok(),
            should_allow_without_private_backup,
            "{source_type}"
        );
    }
}
