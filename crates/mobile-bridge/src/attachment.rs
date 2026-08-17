use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use anlg_attachment_sync_core::{
    CacheFileGuard, DownloadObject, TransferPaths, download_to_path, persist_staged_attachment,
    private_cache_path, stage_attachment_restore, valid_sha256, validate_signed_download_url,
};
use anlg_e2ee::{
    AttachmentBlobCiphertextMetadata, AttachmentBlobContext, AttachmentBlobMetadata,
    AttachmentBlobPlaintextMetadata,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tokio_util::sync::CancellationToken;
use uuid::{Uuid, Version};

use crate::error::{BridgeError, attachment_error, serialization_error};

const FORMAT_VERSION: u8 = 1;

#[derive(Clone)]
pub(crate) struct RestoreOperation {
    cancellation: CancellationToken,
    phase: Arc<Mutex<RestorePhase>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RestorePhase {
    Running,
    Committing,
    Cancelled,
}

impl RestoreOperation {
    pub(crate) fn new() -> Self {
        Self {
            cancellation: CancellationToken::new(),
            phase: Arc::new(Mutex::new(RestorePhase::Running)),
        }
    }

    fn cancellation(&self) -> &CancellationToken {
        &self.cancellation
    }

    fn begin_commit(&self) -> Result<(), BridgeError> {
        let mut phase = self
            .phase
            .lock()
            .map_err(|_| attachment_error("attachment restore state is unavailable"))?;
        if *phase != RestorePhase::Running || self.cancellation.is_cancelled() {
            return Err(attachment_error("attachment restore was cancelled"));
        }
        *phase = RestorePhase::Committing;
        Ok(())
    }

    pub(crate) fn cancel(&self) -> bool {
        self.cancellation.cancel();
        let Ok(mut phase) = self.phase.lock() else {
            return true;
        };
        if *phase != RestorePhase::Running {
            return false;
        }
        *phase = RestorePhase::Cancelled;
        true
    }
}

#[derive(Clone)]
pub(crate) struct AttachmentStorage {
    documents_root: PathBuf,
    transfer_paths: TransferPaths,
}

impl AttachmentStorage {
    pub(crate) fn new(documents_root: String, cache_root: String) -> Result<Self, BridgeError> {
        let documents_root = canonical_directory(documents_root)?;
        let cache_root = canonical_directory(cache_root)?;
        Ok(Self {
            transfer_paths: TransferPaths::new(cache_root, documents_root.clone()),
            documents_root,
        })
    }

    fn destination(&self, attachment: &AttachmentRecord) -> Result<PathBuf, BridgeError> {
        validate_session_id(&attachment.session_id)?;
        validate_audio_relative_path(&attachment.relative_path)?;
        Ok(self
            .documents_root
            .join("sessions")
            .join(&attachment.session_id)
            .join(&attachment.relative_path))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RestoreAttachmentRequest {
    session_id: String,
    attachment_id: String,
    object_id: String,
    object_key: String,
    signed_url: String,
    supabase_url: String,
    ciphertext_sha256: String,
    ciphertext_size_bytes: u64,
    format_version: u8,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RestoredAttachment {
    attachment_id: String,
    session_id: String,
    relative_path: String,
    size_bytes: u64,
    sha256: String,
}

#[derive(Debug, Clone, FromRow)]
struct AttachmentRecord {
    attachment_id: String,
    session_id: String,
    workspace_id: String,
    relative_path: String,
    source_type: String,
    sha256: String,
    size_bytes: i64,
    cloud_object_key: String,
}

pub(crate) async fn restore_attachment(
    db: Arc<anlg_db_core::Db>,
    e2ee_sync_hook: Arc<anlg_db_sync::E2eeSyncHook>,
    storage: AttachmentStorage,
    request_json: String,
    operation: RestoreOperation,
) -> Result<String, BridgeError> {
    let request: RestoreAttachmentRequest =
        serde_json::from_str(&request_json).map_err(|error| {
            BridgeError::InvalidAttachmentRequestJson {
                reason: error.to_string(),
            }
        })?;
    validate_request(&request)?;
    let attachment = load_attachment(db.pool(), &request).await?;
    validate_attachment(&attachment, &request)?;

    let plaintext_size = u64::try_from(attachment.size_bytes).map_err(attachment_error)?;
    let plaintext = AttachmentBlobPlaintextMetadata::from_hex(plaintext_size, &attachment.sha256)
        .map_err(attachment_error)?;
    let key = e2ee_sync_hook
        .workspace_key(&attachment.workspace_id)
        .ok_or_else(|| attachment_error("attachment workspace key is unavailable"))?;
    if key
        .attachment_blob_ciphertext_size(
            &attachment.workspace_id,
            &attachment.attachment_id,
            plaintext.size_bytes,
        )
        .map_err(attachment_error)?
        != request.ciphertext_size_bytes
    {
        return Err(attachment_error("attachment ciphertext size is invalid"));
    }

    let url = validate_signed_download_url(
        &request.signed_url,
        &request.supabase_url,
        DownloadObject::Private(&request.object_key),
    )
    .map_err(attachment_error)?;
    let cache_root = storage.transfer_paths.private_cache_root();
    tokio::fs::create_dir_all(&cache_root)
        .await
        .map_err(attachment_error)?;
    let cache_id = Uuid::new_v4().to_string();
    let cache_path = private_cache_path(&cache_root, &cache_id).map_err(attachment_error)?;
    let _cache_guard = CacheFileGuard::new(cache_path.clone());
    download_to_path(
        url,
        &cache_path,
        request.ciphertext_size_bytes,
        &request.ciphertext_sha256,
        true,
        operation.cancellation(),
    )
    .await
    .map_err(attachment_error)?;

    let destination = storage.destination(&attachment)?;
    let destination_parent = destination
        .parent()
        .ok_or_else(|| attachment_error("attachment destination is invalid"))?
        .to_path_buf();
    let context = AttachmentBlobContext::new(
        attachment.workspace_id.clone(),
        attachment.attachment_id.clone(),
        request.object_id.clone(),
    )
    .map_err(attachment_error)?;
    let expected = AttachmentBlobMetadata {
        version: request.format_version,
        plaintext,
        ciphertext: AttachmentBlobCiphertextMetadata::from_hex(
            request.ciphertext_size_bytes,
            &request.ciphertext_sha256,
        )
        .map_err(attachment_error)?,
    };
    let cache_path_for_restore = cache_path.clone();
    let staged = tokio::task::spawn_blocking(move || {
        stage_attachment_restore(
            &key,
            &context,
            &cache_path_for_restore,
            &destination_parent,
            &expected,
        )
    })
    .await
    .map_err(attachment_error)?
    .map_err(attachment_error)?;

    let mut transaction = db
        .pool()
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(attachment_error)?;
    let canonical: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1 FROM session_attachments
           WHERE id = ? AND session_id = ? AND workspace_id = ?
             AND relative_path = ? AND source_type = ? AND sha256 = ?
             AND size_bytes = ? AND cloud_object_key = ? AND deleted_at IS NULL
         )",
    )
    .bind(&attachment.attachment_id)
    .bind(&attachment.session_id)
    .bind(&attachment.workspace_id)
    .bind(&attachment.relative_path)
    .bind(&attachment.source_type)
    .bind(&attachment.sha256)
    .bind(attachment.size_bytes)
    .bind(&attachment.cloud_object_key)
    .fetch_one(&mut *transaction)
    .await
    .map_err(attachment_error)?;
    if !canonical {
        return Err(attachment_error("attachment changed during restore"));
    }
    operation.begin_commit()?;
    persist_staged_attachment(staged, &destination).map_err(attachment_error)?;
    let local_state = sqlx::query(
        "INSERT INTO attachment_local_state (
           attachment_id, session_id, relative_path, availability, updated_at
         ) VALUES (?, ?, ?, 'present', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(attachment_id) DO UPDATE SET
           session_id = excluded.session_id,
           relative_path = excluded.relative_path,
           availability = excluded.availability,
           updated_at = excluded.updated_at",
    )
    .bind(&attachment.attachment_id)
    .bind(&attachment.session_id)
    .bind(&attachment.relative_path)
    .execute(&mut *transaction)
    .await
    .map_err(attachment_error)?;
    if local_state.rows_affected() != 1 {
        return Err(attachment_error("attachment local state was not updated"));
    }
    transaction.commit().await.map_err(attachment_error)?;

    serde_json::to_string(&RestoredAttachment {
        attachment_id: attachment.attachment_id,
        session_id: attachment.session_id,
        relative_path: attachment.relative_path,
        size_bytes: plaintext_size,
        sha256: attachment.sha256,
    })
    .map_err(serialization_error)
}

async fn load_attachment(
    pool: &sqlx::SqlitePool,
    request: &RestoreAttachmentRequest,
) -> Result<AttachmentRecord, BridgeError> {
    sqlx::query_as(
        "SELECT id AS attachment_id, session_id, workspace_id, relative_path,
                source_type, sha256, size_bytes, cloud_object_key
         FROM session_attachments
         WHERE id = ? AND session_id = ? AND source_type = 'session_audio'
           AND deleted_at IS NULL",
    )
    .bind(&request.attachment_id)
    .bind(&request.session_id)
    .fetch_one(pool)
    .await
    .map_err(attachment_error)
}

fn validate_request(request: &RestoreAttachmentRequest) -> Result<(), BridgeError> {
    validate_session_id(&request.session_id)?;
    if request.attachment_id != format!("session-audio:{}", request.session_id)
        || request.format_version != FORMAT_VERSION
        || !valid_sha256(&request.ciphertext_sha256)
        || request.ciphertext_size_bytes == 0
    {
        return Err(attachment_error("attachment restore metadata is invalid"));
    }
    validate_private_object_identity(&request.object_id, &request.object_key)
}

fn validate_attachment(
    attachment: &AttachmentRecord,
    request: &RestoreAttachmentRequest,
) -> Result<(), BridgeError> {
    if attachment.attachment_id != request.attachment_id
        || attachment.session_id != request.session_id
        || attachment.workspace_id.is_empty()
        || attachment.source_type != "session_audio"
        || !valid_sha256(&attachment.sha256)
        || attachment.size_bytes <= 0
        || attachment.cloud_object_key != request.object_key
    {
        return Err(attachment_error("attachment restore state is invalid"));
    }
    Ok(())
}

fn canonical_directory(value: String) -> Result<PathBuf, BridgeError> {
    let path = PathBuf::from(value);
    if !path.is_absolute() || path.components().any(|part| part == Component::ParentDir) {
        return Err(attachment_error("attachment storage path is invalid"));
    }
    std::fs::create_dir_all(&path).map_err(attachment_error)?;
    std::fs::canonicalize(path).map_err(attachment_error)
}

fn validate_session_id(value: &str) -> Result<(), BridgeError> {
    let uuid = Uuid::parse_str(value).map_err(attachment_error)?;
    if uuid.to_string() != value || uuid.get_version() != Some(Version::Random) {
        return Err(attachment_error("attachment session id is invalid"));
    }
    Ok(())
}

fn validate_private_object_identity(object_id: &str, object_key: &str) -> Result<(), BridgeError> {
    let object_uuid = Uuid::parse_str(object_id).map_err(attachment_error)?;
    let (owner, filename) = object_key
        .split_once('/')
        .ok_or_else(|| attachment_error("attachment object key is invalid"))?;
    let owner_uuid = Uuid::parse_str(owner).map_err(attachment_error)?;
    if object_uuid.to_string() != object_id
        || object_uuid.get_version() != Some(Version::Random)
        || owner_uuid.to_string() != owner
        || filename != format!("{object_id}.anb1")
    {
        return Err(attachment_error("attachment object identity is invalid"));
    }
    Ok(())
}

fn validate_audio_relative_path(value: &str) -> Result<(), BridgeError> {
    let mut components = Path::new(value).components();
    let Some(Component::Normal(filename)) = components.next() else {
        return Err(attachment_error("attachment relative path is invalid"));
    };
    if components.next().is_some() {
        return Err(attachment_error("attachment relative path is invalid"));
    }
    let Some(filename) = filename.to_str() else {
        return Err(attachment_error("attachment filename is invalid"));
    };
    let Some(extension) = filename.strip_prefix("audio.") else {
        return Err(attachment_error("attachment filename is invalid"));
    };
    if !matches!(
        extension,
        "aac" | "caf" | "flac" | "m4a" | "mp3" | "mp4" | "ogg" | "wav" | "webm"
    ) {
        return Err(attachment_error("attachment filename is invalid"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_attachment_sync_core::{hex_digest, seal_attachment_to_cache};
    use sha2::{Digest, Sha256};
    use std::io::{Read, Write};

    #[test]
    fn rejects_paths_outside_the_session_audio_layout() {
        for path in [
            "../audio.wav",
            "attachments/audio.wav",
            "restored-audio.wav",
        ] {
            assert!(validate_audio_relative_path(path).is_err());
        }
        assert!(validate_audio_relative_path("audio.wav").is_ok());
        assert!(validate_audio_relative_path("audio.m4a").is_ok());
    }

    #[test]
    fn private_object_identity_is_bound_to_its_key() {
        let owner = Uuid::new_v4();
        let object = Uuid::new_v4();
        let key = format!("{owner}/{object}.anb1");

        assert!(validate_private_object_identity(&object.to_string(), &key).is_ok());
        assert!(validate_private_object_identity(&Uuid::new_v4().to_string(), &key).is_err());
    }

    #[test]
    fn cancellation_cannot_interrupt_the_commit_phase() {
        let cancelled = RestoreOperation::new();
        assert!(cancelled.cancel());
        assert!(cancelled.begin_commit().is_err());

        let committing = RestoreOperation::new();
        committing.begin_commit().unwrap();
        assert!(!committing.cancel());
    }

    #[tokio::test]
    async fn restores_encrypted_audio_and_commits_local_state() {
        let directory = tempfile::tempdir().unwrap();
        let documents = directory.path().join("documents");
        let cache = directory.path().join("cache");
        std::fs::create_dir_all(&documents).unwrap();
        std::fs::create_dir_all(&cache).unwrap();
        let db = Arc::new(
            crate::db::open_app_db(&directory.path().join("app.db"), false)
                .await
                .unwrap(),
        );
        let recovery_key = anlg_e2ee::RecoveryKey::generate().unwrap();
        let workspace_id = Uuid::new_v4().to_string();
        let session_id = Uuid::new_v4().to_string();
        let attachment_id = format!("session-audio:{session_id}");
        let owner_id = Uuid::new_v4();
        let object_id = Uuid::new_v4();
        let object_key = format!("{owner_id}/{object_id}.anb1");
        let plaintext = b"mobile encrypted recording";
        let plaintext_sha256 = hex_digest(Sha256::digest(plaintext).as_slice());
        let source = directory.path().join("source.wav");
        let ciphertext = directory.path().join("ciphertext.anb1");
        std::fs::write(&source, plaintext).unwrap();
        let workspace_key = recovery_key.workspace_key(&workspace_id).unwrap();
        let context = AttachmentBlobContext::new(
            workspace_id.clone(),
            attachment_id.clone(),
            object_id.to_string(),
        )
        .unwrap();
        let expected_plaintext =
            AttachmentBlobPlaintextMetadata::from_hex(plaintext.len() as u64, &plaintext_sha256)
                .unwrap();
        let (blob, guard) = seal_attachment_to_cache(
            workspace_key,
            context,
            source,
            ciphertext.clone(),
            expected_plaintext,
        )
        .await
        .unwrap();
        guard.disarm();

        sqlx::query("INSERT INTO sessions (id, workspace_id) VALUES (?, ?)")
            .bind(&session_id)
            .bind(&workspace_id)
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO session_attachments (
               id, workspace_id, session_id, filename, relative_path,
               content_type, size_bytes, sha256, cloud_object_key, source_type
             ) VALUES (?, ?, ?, 'audio.wav', 'audio.wav', 'audio/wav', ?, ?, ?, 'session_audio')",
        )
        .bind(&attachment_id)
        .bind(&workspace_id)
        .bind(&session_id)
        .bind(i64::try_from(plaintext.len()).unwrap())
        .bind(&plaintext_sha256)
        .bind(&object_key)
        .execute(db.pool())
        .await
        .unwrap();

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let body = std::fs::read(ciphertext).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request).unwrap();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(&body).unwrap();
        });

        let hook = Arc::new(anlg_db_sync::E2eeSyncHook::default());
        hook.set_personal_workspace(&workspace_id, &recovery_key)
            .unwrap();
        let request = serde_json::json!({
            "sessionId": session_id,
            "attachmentId": attachment_id,
            "objectId": object_id.to_string(),
            "objectKey": object_key,
            "signedUrl": format!(
                "http://{address}/storage/v1/object/sign/attachment-backups/{object_key}?token=test"
            ),
            "supabaseUrl": format!("http://{address}"),
            "ciphertextSha256": blob.ciphertext.sha256_hex(),
            "ciphertextSizeBytes": blob.ciphertext.size_bytes,
            "formatVersion": blob.version,
        });
        let storage = AttachmentStorage::new(
            documents.to_string_lossy().into_owned(),
            cache.to_string_lossy().into_owned(),
        )
        .unwrap();

        let restored: serde_json::Value = serde_json::from_str(
            &restore_attachment(
                Arc::clone(&db),
                hook,
                storage,
                request.to_string(),
                RestoreOperation::new(),
            )
            .await
            .unwrap(),
        )
        .unwrap();
        server.join().unwrap();

        assert_eq!(restored["attachmentId"], attachment_id);
        assert_eq!(
            std::fs::read(
                documents
                    .join("sessions")
                    .join(&session_id)
                    .join("audio.wav")
            )
            .unwrap(),
            plaintext
        );
        let local_state: (String, String) = sqlx::query_as(
            "SELECT availability, relative_path FROM attachment_local_state WHERE attachment_id = ?",
        )
        .bind(&attachment_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(
            local_state,
            ("present".to_string(), "audio.wav".to_string())
        );
    }
}
