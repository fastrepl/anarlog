//! `useFileUpload` + `catalogLocalNoteAttachment`: a pasted or dropped file
//! is written under the session's `attachments` folder through the shared
//! fs-sync core, then catalogued with the frontend's own statements.

use anyhow::Context as _;
use sha2::Digest as _;

use super::Store;

/// `MAX_IPC_ATTACHMENT_BYTES`
pub const MAX_ATTACHMENT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedAttachment {
    pub attachment_id: String,
    pub path: String,
    pub size_bytes: usize,
}

/// `enqueueReplacedAttachmentDeleteStatement` by relative path.
const REPLACED_DELETE_SQL: &str = r#"
      INSERT OR IGNORE INTO attachment_transfer_jobs (
        id,
        attachment_id,
        session_id,
        workspace_id,
        direction,
        expected_sha256,
        expected_size_bytes,
        object_key
      )
      SELECT ?, attachment.id, attachment.session_id, attachment.workspace_id,
        'delete', attachment.sha256, attachment.size_bytes,
        attachment.cloud_object_key
      FROM session_attachments AS attachment
      WHERE attachment.session_id = ?
        AND attachment.relative_path = ?
        AND (attachment.sha256 <> ? OR attachment.size_bytes <> ?)
        AND attachment.cloud_object_key <> ''
      ORDER BY attachment.deleted_at IS NULL DESC,
        attachment.updated_at DESC,
        attachment.id
      LIMIT 1
"#;

const UPDATE_EXISTING_SQL: &str = r#"
          UPDATE session_attachments
          SET
            filename = ?,
            content_type = ?,
            size_bytes = ?,
            cloud_object_key = CASE
              WHEN session_attachments.sha256 = ?
                AND session_attachments.size_bytes = ? THEN cloud_object_key
              ELSE ''
            END,
            storage_kind = CASE
              WHEN session_attachments.sha256 = ?
                AND session_attachments.size_bytes = ? THEN storage_kind
              ELSE 'local_file'
            END,
            sha256 = ?,
            source_type = 'note_upload',
            source_id = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            deleted_at = NULL
          WHERE id = (
            SELECT attachment.id
            FROM session_attachments AS attachment
            JOIN sessions AS session
              ON session.id = attachment.session_id
              AND session.deleted_at IS NULL
            WHERE attachment.session_id = ?
              AND attachment.relative_path = ?
            ORDER BY attachment.deleted_at IS NULL DESC,
              attachment.updated_at DESC,
              attachment.id
            LIMIT 1
          )
"#;

const INSERT_SQL: &str = r#"
          INSERT INTO session_attachments (
            id,
            workspace_id,
            session_id,
            filename,
            relative_path,
            content_type,
            size_bytes,
            sha256,
            storage_kind,
            cloud_object_key,
            source_type,
            source_id,
            metadata_json
          )
          SELECT
            ?,
            session.workspace_id,
            session.id,
            ?,
            ?,
            ?,
            ?,
            ?,
            'local_file',
            '',
            'note_upload',
            ?,
            '{}'
          FROM sessions AS session
          WHERE session.id = ?
            AND session.deleted_at IS NULL
            AND NOT EXISTS (
              SELECT 1
              FROM session_attachments AS attachment
              WHERE attachment.session_id = session.id
                AND attachment.relative_path = ?
                AND attachment.deleted_at IS NULL
            )
"#;

const LOCAL_STATE_SQL: &str = r#"
          INSERT INTO attachment_local_state (
            attachment_id,
            session_id,
            relative_path,
            availability,
            updated_at
          )
          SELECT
            attachment.id,
            attachment.session_id,
            attachment.relative_path,
            'present',
            strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          FROM session_attachments AS attachment
          WHERE attachment.session_id = ?
            AND attachment.relative_path = ?
            AND attachment.deleted_at IS NULL
          ORDER BY attachment.updated_at DESC, attachment.id
          LIMIT 1
          ON CONFLICT(attachment_id) DO UPDATE SET
            session_id = excluded.session_id,
            relative_path = excluded.relative_path,
            availability = excluded.availability,
            updated_at = excluded.updated_at
"#;

/// `enqueueAttachmentUploadStatement` by relative path.
const UPLOAD_JOB_SQL: &str = r#"
      INSERT OR IGNORE INTO attachment_transfer_jobs (
        id,
        attachment_id,
        session_id,
        workspace_id,
        direction,
        expected_sha256,
        expected_size_bytes
      )
      SELECT ?, attachment.id, attachment.session_id, attachment.workspace_id,
        'upload', attachment.sha256, attachment.size_bytes
      FROM session_attachments AS attachment
      JOIN attachment_local_state AS local
        ON local.attachment_id = attachment.id
        AND local.availability = 'present'
      WHERE attachment.session_id = ?
        AND attachment.relative_path = ?
        AND attachment.cloud_sync_enabled = 1
        AND attachment.cloud_object_key = ''
        AND attachment.deleted_at IS NULL
      ORDER BY attachment.updated_at DESC, attachment.id
      LIMIT 1
"#;

impl Store {
    /// `useFileUpload`: `attachmentSave`, then `catalogLocalNoteAttachment`;
    /// a catalogue failure removes the file again (`attachmentRemove`).
    pub fn save_note_attachment(
        &self,
        session_id: String,
        filename: String,
        content_type: String,
        bytes: Vec<u8>,
    ) -> tokio::task::JoinHandle<anyhow::Result<SavedAttachment>> {
        let db = self.db.clone();
        let session_dir = self.session_dir(&session_id);
        let lock = self.session_lock(&session_id);
        self.runtime.spawn(async move {
            if bytes.len() > MAX_ATTACHMENT_BYTES {
                anyhow::bail!("Attachments must be smaller than 4 MB");
            }
            let size_bytes = bytes.len();
            let sha256 = sha2::Sha256::digest(&bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            let saved = {
                let session_dir = session_dir.clone();
                let filename = filename.clone();
                tokio::task::spawn_blocking(move || {
                    anlg_fs_sync_core::attachments::save(&session_dir, &bytes, &filename)
                })
                .await
                .context("attachment save task")??
            };
            let _guard = lock.lock().await;
            let catalogued = catalog(
                db.pool(),
                &session_id,
                &saved.attachment_id,
                &filename,
                &content_type,
                size_bytes,
                &sha256,
            )
            .await;
            if let Err(error) = catalogued {
                let attachment_id = saved.attachment_id.clone();
                let removed = tokio::task::spawn_blocking(move || {
                    anlg_fs_sync_core::attachments::remove(&session_dir, &attachment_id)
                })
                .await;
                if !matches!(removed, Ok(Ok(()))) {
                    tracing::error!("[attachment] failed to roll back local file");
                }
                return Err(error);
            }
            Ok(SavedAttachment {
                attachment_id: saved.attachment_id,
                path: saved.path,
                size_bytes,
            })
        })
    }
}

/// `catalogLocalNoteAttachment`'s transaction.
async fn catalog(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    attachment_id: &str,
    filename: &str,
    content_type: &str,
    size_bytes: usize,
    sha256: &str,
) -> anyhow::Result<()> {
    let relative_path = format!("attachments/{attachment_id}");
    let size = size_bytes as i64;
    let mut tx = pool.begin().await?;
    sqlx::query(REPLACED_DELETE_SQL)
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(session_id)
        .bind(&relative_path)
        .bind(sha256)
        .bind(size)
        .execute(&mut *tx)
        .await?;
    let updated = sqlx::query(UPDATE_EXISTING_SQL)
        .bind(filename)
        .bind(content_type)
        .bind(size)
        .bind(sha256)
        .bind(size)
        .bind(sha256)
        .bind(size)
        .bind(sha256)
        .bind(attachment_id)
        .bind(session_id)
        .bind(&relative_path)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    let inserted = sqlx::query(INSERT_SQL)
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(filename)
        .bind(&relative_path)
        .bind(content_type)
        .bind(size)
        .bind(sha256)
        .bind(attachment_id)
        .bind(session_id)
        .bind(&relative_path)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    let local = sqlx::query(LOCAL_STATE_SQL)
        .bind(session_id)
        .bind(&relative_path)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if local != 1 {
        anyhow::bail!("attachment session is unavailable");
    }
    sqlx::query(UPLOAD_JOB_SQL)
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(session_id)
        .bind(&relative_path)
        .execute(&mut *tx)
        .await?;
    if updated + inserted != 1 {
        anyhow::bail!("attachment session is unavailable");
    }
    tx.commit().await?;
    Ok(())
}

/// `convertFileSrc(path)`: the asset protocol URL the web view stores in the
/// node's `src` (`asset://localhost/` outside Windows, `http://asset.localhost/`
/// there), with the path `encodeURIComponent`-escaped.
pub fn convert_file_src(path: &str) -> String {
    let encoded = encode_uri_component(path);
    if cfg!(windows) {
        format!("http://asset.localhost/{encoded}")
    } else {
        format!("asset://localhost/{encoded}")
    }
}

/// `encodeURIComponent`: everything but `A-Z a-z 0-9 - _ . ! ~ * ' ( )` is
/// percent-encoded as UTF-8.
pub fn encode_uri_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// `insertImage`: the `image` node for an uploaded file, attrs in schema order.
pub fn image_node(saved: &SavedAttachment) -> serde_json::Value {
    serde_json::json!({
        "type": "image",
        "attrs": {
            "src": convert_file_src(&saved.path),
            "alt": null,
            "title": null,
            "attachmentId": saved.attachment_id,
            "sharedAttachmentId": null,
            "editorWidth": crate::document::DEFAULT_IMAGE_WIDTH,
        }
    })
}

/// `insertFileAttachment`: the `fileAttachment` node, attrs in schema order.
pub fn file_attachment_node(
    saved: &SavedAttachment,
    name: &str,
    mime_type: &str,
) -> serde_json::Value {
    serde_json::json!({
        "type": "fileAttachment",
        "attrs": {
            "attachmentId": saved.attachment_id,
            "sharedAttachmentId": null,
            "name": name,
            "mimeType": mime_type,
            "src": convert_file_src(&saved.path),
            "path": saved.path,
            "size": saved.size_bytes,
        }
    })
}

/// `IMAGE_MIME_TYPES`
pub fn is_image_mime(mime_type: &str) -> bool {
    matches!(
        mime_type,
        "image/png" | "image/jpeg" | "image/gif" | "image/webp"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_urls_follow_convert_file_src() {
        assert_eq!(
            encode_uri_component("/home/a b/x (1).png~!*'"),
            "%2Fhome%2Fa%20b%2Fx%20(1).png~!*'"
        );
        assert_eq!(encode_uri_component("é"), "%C3%A9");
        if !cfg!(windows) {
            assert_eq!(
                convert_file_src("/tmp/image.png"),
                "asset://localhost/%2Ftmp%2Fimage.png"
            );
        }
    }

    #[test]
    fn nodes_keep_the_schema_attr_order() {
        let saved = SavedAttachment {
            attachment_id: "image.png".into(),
            path: "/v/s/attachments/image.png".into(),
            size_bytes: 756,
        };
        assert_eq!(
            image_node(&saved).to_string(),
            format!(
                r#"{{"type":"image","attrs":{{"src":"{}","alt":null,"title":null,"attachmentId":"image.png","sharedAttachmentId":null,"editorWidth":80}}}}"#,
                convert_file_src(&saved.path)
            )
        );
        assert_eq!(
            file_attachment_node(&saved, "notes.pdf", "application/pdf").to_string(),
            format!(
                r#"{{"type":"fileAttachment","attrs":{{"attachmentId":"image.png","sharedAttachmentId":null,"name":"notes.pdf","mimeType":"application/pdf","src":"{}","path":"/v/s/attachments/image.png","size":756}}}}"#,
                convert_file_src(&saved.path)
            )
        );
        assert!(is_image_mime("image/png"));
        assert!(!is_image_mime("image/svg+xml"));
    }
}
