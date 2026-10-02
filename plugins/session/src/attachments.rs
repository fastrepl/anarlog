use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use crate::transaction_utils::{js_iso8601_timestamp, rollback_row_count_mismatch};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CatalogNoteAttachmentRequest {
    pub session_id: String,
    pub attachment_id: String,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CatalogSessionAudioRequest {
    pub session_id: String,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SessionAudioRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SetAttachmentCloudSyncEnabledRequest {
    pub session_id: String,
    pub attachment_id: String,
    pub enabled: bool,
}

fn session_audio_attachment_id(session_id: &str) -> String {
    format!("session-audio:{session_id}")
}

pub async fn catalog_note_attachment(
    pool: &SqlitePool,
    request: CatalogNoteAttachmentRequest,
) -> Result<(), String> {
    let relative_path = format!("attachments/{}", request.attachment_id);
    let metadata_id = uuid::Uuid::new_v4().to_string();
    let delete_job_id = uuid::Uuid::new_v4().to_string();
    let upload_job_id = uuid::Uuid::new_v4().to_string();

    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    anlg_db_app::enqueue_replaced_attachment_delete_by_path(
        &mut transaction,
        &delete_job_id,
        &request.session_id,
        &relative_path,
        &request.sha256,
        request.size_bytes,
    )
    .await
    .map_err(|error| error.to_string())?;

    let updated = anlg_db_app::update_note_attachment(
        &mut transaction,
        &request.filename,
        &request.content_type,
        request.size_bytes,
        &request.sha256,
        &request.attachment_id,
        &request.session_id,
        &relative_path,
    )
    .await
    .map_err(|error| error.to_string())?;

    let inserted = anlg_db_app::insert_note_attachment(
        &mut transaction,
        &metadata_id,
        &request.filename,
        &relative_path,
        &request.content_type,
        request.size_bytes,
        &request.sha256,
        &request.attachment_id,
        &request.session_id,
    )
    .await
    .map_err(|error| error.to_string())?;

    let local_state = anlg_db_app::upsert_attachment_local_state_by_path(
        &mut transaction,
        &request.session_id,
        &relative_path,
    )
    .await
    .map_err(|error| error.to_string())?;
    if local_state != 1 {
        return Err(rollback_row_count_mismatch(transaction, 3, local_state, 1).await);
    }

    anlg_db_app::enqueue_attachment_upload_by_path(
        &mut transaction,
        &upload_job_id,
        &request.session_id,
        &relative_path,
    )
    .await
    .map_err(|error| error.to_string())?;

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())?;

    if updated + inserted != 1 {
        return Err("attachment session is unavailable".to_string());
    }
    Ok(())
}

pub async fn catalog_session_audio(
    pool: &SqlitePool,
    request: CatalogSessionAudioRequest,
) -> Result<(), String> {
    let attachment_id = session_audio_attachment_id(&request.session_id);
    let delete_job_id = uuid::Uuid::new_v4().to_string();
    let upload_job_id = uuid::Uuid::new_v4().to_string();

    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    anlg_db_app::enqueue_replaced_attachment_delete_by_id(
        &mut transaction,
        &delete_job_id,
        &request.session_id,
        &attachment_id,
        &request.sha256,
        request.size_bytes,
    )
    .await
    .map_err(|error| error.to_string())?;

    let updated = anlg_db_app::update_session_audio_attachment(
        &mut transaction,
        &request.filename,
        &request.content_type,
        request.size_bytes,
        &request.sha256,
        &attachment_id,
        &request.session_id,
    )
    .await
    .map_err(|error| error.to_string())?;

    let inserted = anlg_db_app::insert_session_audio_attachment(
        &mut transaction,
        &attachment_id,
        &request.filename,
        &request.content_type,
        request.size_bytes,
        &request.sha256,
        &request.session_id,
    )
    .await
    .map_err(|error| error.to_string())?;

    let local_state = anlg_db_app::upsert_attachment_local_state_by_id(
        &mut transaction,
        &attachment_id,
        &request.session_id,
    )
    .await
    .map_err(|error| error.to_string())?;
    if local_state != 1 {
        return Err(rollback_row_count_mismatch(transaction, 3, local_state, 1).await);
    }

    anlg_db_app::enqueue_attachment_upload_by_id(
        &mut transaction,
        &upload_job_id,
        &request.session_id,
        &attachment_id,
    )
    .await
    .map_err(|error| error.to_string())?;

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())?;

    if updated + inserted != 1 {
        return Err("audio session is unavailable".to_string());
    }
    Ok(())
}

pub async fn mark_session_audio_transcription_complete(
    pool: &SqlitePool,
    request: SessionAudioRequest,
) -> Result<(), String> {
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    anlg_db_app::mark_session_audio_transcription_complete(&mut transaction, &request.session_id)
        .await
        .map_err(|error| error.to_string())?;

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}

pub async fn set_attachment_cloud_sync_enabled(
    pool: &SqlitePool,
    request: SetAttachmentCloudSyncEnabledRequest,
) -> Result<(), String> {
    let now = js_iso8601_timestamp();
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    let updated = anlg_db_app::set_attachment_cloud_sync_flag(
        &mut transaction,
        i64::from(request.enabled),
        &now,
        &request.attachment_id,
        &request.session_id,
    )
    .await
    .map_err(|error| error.to_string())?;

    if request.enabled {
        anlg_db_app::complete_attachment_delete_jobs(
            &mut transaction,
            &now,
            &request.attachment_id,
        )
        .await
        .map_err(|error| error.to_string())?;
        anlg_db_app::enqueue_attachment_upload_by_id(
            &mut transaction,
            &uuid::Uuid::new_v4().to_string(),
            &request.session_id,
            &request.attachment_id,
        )
        .await
        .map_err(|error| error.to_string())?;
        anlg_db_app::enqueue_attachment_download_job_enabled(
            &mut transaction,
            &uuid::Uuid::new_v4().to_string(),
            &request.attachment_id,
            &request.session_id,
        )
        .await
        .map_err(|error| error.to_string())?;
    } else {
        anlg_db_app::complete_attachment_upload_download_jobs(
            &mut transaction,
            &now,
            &request.attachment_id,
        )
        .await
        .map_err(|error| error.to_string())?;
        anlg_db_app::enqueue_attachment_download_job_disabled(
            &mut transaction,
            &uuid::Uuid::new_v4().to_string(),
            &request.attachment_id,
            &request.session_id,
        )
        .await
        .map_err(|error| error.to_string())?;
        anlg_db_app::enqueue_attachment_delete_job_disabled(
            &mut transaction,
            &uuid::Uuid::new_v4().to_string(),
            &request.attachment_id,
            &request.session_id,
        )
        .await
        .map_err(|error| error.to_string())?;
    }

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())?;

    if updated != 1 {
        return Err("attachment is unavailable".to_string());
    }
    Ok(())
}

pub async fn tombstone_session_audio(
    pool: &SqlitePool,
    request: SessionAudioRequest,
) -> Result<(), String> {
    let attachment_id = session_audio_attachment_id(&request.session_id);
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    anlg_db_app::tombstone_session_audio_attachment(
        &mut transaction,
        &attachment_id,
        &request.session_id,
    )
    .await
    .map_err(|error| error.to_string())?;

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}

pub async fn mark_session_audio_absent(
    pool: &SqlitePool,
    request: SessionAudioRequest,
) -> Result<(), String> {
    anlg_db_app::mark_session_audio_absent(pool, &request.session_id)
        .await
        .map_err(|error| error.to_string())
}
