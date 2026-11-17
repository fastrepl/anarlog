use std::{io::Read, path::PathBuf, sync::Arc};

use anlg_db_execute::TransactionStatement;
use anlg_fs_sync_core::FsSyncCore;
use desktop_runtime::{CancellationToken, RuntimeHandle, ServiceError, SessionId};
use serde_json::json;
use sha2::{Digest, Sha256};

const MAX_BYTES: usize = 4 * 1024 * 1024;

enum Source {
    File(PathBuf),
    Bytes { name: String, bytes: Vec<u8> },
}

#[derive(Clone)]
pub struct AttachmentService {
    runtime: RuntimeHandle,
    vault: Arc<PathBuf>,
}

#[derive(Clone)]
pub struct AttachmentPreview {
    pub id: String,
    pub name: String,
    pub mime: String,
    pub size: u64,
    pub path: Arc<PathBuf>,
}

fn failure(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Failed(error.to_string().into())
}

impl AttachmentService {
    pub fn new(runtime: RuntimeHandle, vault: PathBuf) -> Self {
        Self {
            runtime,
            vault: Arc::new(vault),
        }
    }

    pub async fn import(
        &self,
        session: SessionId,
        source: PathBuf,
    ) -> Result<AttachmentPreview, String> {
        self.import_source(session, Source::File(source)).await
    }

    pub async fn import_bytes(
        &self,
        session: SessionId,
        name: String,
        bytes: Vec<u8>,
    ) -> Result<AttachmentPreview, String> {
        self.import_source(session, Source::Bytes { name, bytes })
            .await
    }

    async fn import_source(
        &self,
        session: SessionId,
        source: Source,
    ) -> Result<AttachmentPreview, String> {
        let vault = self.vault.clone();
        self.runtime.submit(move |services| async move {
            let worker_session = session.clone();
            let worker_vault = vault.clone();
            let preview = tokio::task::spawn_blocking(move || {
                let (name, bytes) = match source {
                    Source::File(source) => {
                        let name = source.file_name().and_then(|name| name.to_str()).ok_or_else(|| failure("Invalid filename"))?.to_owned();
                        let mut bytes = Vec::new();
                        std::fs::File::open(&source).map_err(failure)?.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes).map_err(failure)?;
                        (name, bytes)
                    }
                    Source::Bytes { name, bytes } => (name, bytes),
                };
                if bytes.len() > MAX_BYTES { return Err(failure("Attachments must be smaller than 4 MB")); }
                let checksum = Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect::<String>();
                let mime = mime_guess::from_path(&name).first_or_octet_stream().to_string();
                let saved = FsSyncCore::new((*worker_vault).clone()).attachment_save(&worker_session.0, &bytes, &name).map_err(failure)?;
                Ok((AttachmentPreview { id: saved.attachment_id, name, mime, size: bytes.len() as u64, path: Arc::new(saved.path.into()) }, checksum))
            }).await.map_err(failure)??;
            let (preview, checksum) = preview;
            let metadata_id = uuid::Uuid::new_v4().to_string();
            let relative = format!("attachments/{}", preview.id);
            let statement = |sql: &str, params, expected_rows_affected| TransactionStatement {
                sql: sql.into(), params, expected_rows_affected,
            };
            let result = services.executor.execute_transaction(vec![
                statement("INSERT INTO session_attachments
                  (id,workspace_id,session_id,filename,relative_path,content_type,size_bytes,sha256,source_type,source_id)
                  SELECT ?,workspace_id,id,?,?,?,?,?,'note_upload',? FROM sessions WHERE id=? AND deleted_at IS NULL",
                  vec![json!(metadata_id),json!(preview.name),json!(relative),json!(preview.mime),json!(preview.size),json!(checksum),json!(preview.id),json!(session)], Some(1)),
                statement("INSERT INTO attachment_local_state (attachment_id,session_id,relative_path,availability)
                  VALUES (?,?,?,'present')", vec![json!(metadata_id),json!(session),json!(relative)], Some(1)),
                statement("INSERT OR IGNORE INTO attachment_transfer_jobs
                  (id,attachment_id,session_id,workspace_id,direction,expected_sha256,expected_size_bytes)
                  SELECT ?,id,session_id,workspace_id,'upload',sha256,size_bytes FROM session_attachments
                  WHERE id=? AND cloud_sync_enabled=1 AND cloud_object_key='' AND deleted_at IS NULL",
                  vec![json!(uuid::Uuid::new_v4().to_string()),json!(metadata_id)], None),
            ]).await;
            if let Err(error) = result {
                let cleanup_id = preview.id.clone();
                tokio::task::spawn_blocking(move || FsSyncCore::new((*vault).clone()).attachment_remove(&session.0, &cleanup_id))
                    .await.map_err(failure)?.map_err(failure)?;
                return Err(failure(error));
            }
            Ok(preview)
        }).map_err(|error| error.to_string())?.receive().await.map_err(|error| error.to_string())
    }

    pub async fn resolve(
        &self,
        session: SessionId,
        id: String,
        cancel: CancellationToken,
    ) -> Result<AttachmentPreview, String> {
        if id.is_empty() || id.contains(['/', '\\', '\0']) || matches!(id.as_str(), "." | "..") {
            return Err("Invalid attachment identity".into());
        }
        let vault = self.vault.clone();
        self.runtime.read(cancel, move |services| async move {
            let rows = services.executor.execute(
                "SELECT filename,content_type,size_bytes,relative_path FROM session_attachments
                 WHERE session_id=? AND (relative_path=? OR id=?) AND deleted_at IS NULL ORDER BY updated_at DESC,id LIMIT 1".into(),
                vec![json!(session),json!(format!("attachments/{id}")),json!(id)],
            ).await.map_err(failure)?;
            let row = rows.first().ok_or_else(|| failure("Attachment is not catalogued in this session"))?;
            let name = row["filename"].as_str().unwrap_or(&id).to_owned();
            let mime = row["content_type"].as_str().unwrap_or("").to_owned();
            let size = row["size_bytes"].as_u64().unwrap_or(0);
            let local_id = row["relative_path"].as_str().and_then(|path| path.strip_prefix("attachments/"))
                .filter(|id| !id.is_empty() && !id.contains(['/', '\\', '\0']) && !matches!(*id, "." | ".."))
                .ok_or_else(|| failure("Attachment has no valid local reference"))?.to_owned();
            tokio::task::spawn_blocking(move || {
                let core = FsSyncCore::new((*vault).clone());
                let info = core.attachment_list(&session.0).map_err(failure)?
                    .into_iter().find(|info| info.attachment_id == local_id).ok_or_else(|| failure("Attachment is not available locally"))?;
                let path = PathBuf::from(info.path).canonicalize().map_err(failure)?;
                let base = vault.canonicalize().map_err(failure)?;
                let session_base = core.resolve_session_dir(&session.0).map_err(failure)?.join("attachments").canonicalize().map_err(failure)?;
                if !path.starts_with(base) || !path.starts_with(session_base) { return Err(failure("Attachment resolves outside this session")); }
                Ok(AttachmentPreview { id, name, mime, size, path: Arc::new(path) })
            }).await.map_err(failure)?
        }).map_err(|error| error.to_string())?.receive().await.map_err(|error| error.to_string())
    }
}
