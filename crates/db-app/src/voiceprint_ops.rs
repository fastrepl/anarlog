use sqlx::SqlitePool;

use crate::{
    NewVoiceprintExemplar, VOICEPRINT_KEYRING_SCOPE, VOICEPRINT_SYNC_SCOPE, VoiceprintExemplar,
    VoiceprintExemplarError, VoiceprintSecretRef,
};

const CONFIRMATION_SOURCES: &[&str] =
    &["accessibility_active_speaker", "manual_speaker_assignment"];

pub async fn insert_voiceprint_exemplar(
    pool: &SqlitePool,
    exemplar: NewVoiceprintExemplar<'_>,
) -> Result<VoiceprintExemplar, VoiceprintExemplarError> {
    validate_exemplar(&exemplar)?;
    let mut transaction = pool.begin().await?;

    let human_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1
           FROM humans
           WHERE id = ? AND workspace_id = ? AND deleted_at IS NULL
         )",
    )
    .bind(exemplar.human_id)
    .bind(exemplar.workspace_id)
    .fetch_one(&mut *transaction)
    .await?;
    if !human_exists {
        return Err(VoiceprintExemplarError::HumanNotFound);
    }

    let source_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1
           FROM sessions AS session
           JOIN transcripts AS transcript
             ON transcript.id = ?
            AND transcript.session_id = session.id
            AND transcript.workspace_id = session.workspace_id
            AND transcript.audio_attachment_id = ?
            AND transcript.deleted_at IS NULL
           JOIN session_attachments AS attachment
             ON attachment.id = transcript.audio_attachment_id
            AND attachment.session_id = session.id
            AND attachment.workspace_id = session.workspace_id
            AND attachment.deleted_at IS NULL
           WHERE session.id = ?
             AND session.workspace_id = ?
             AND session.deleted_at IS NULL
             AND EXISTS (
               SELECT 1
               FROM session_participants AS participant
               WHERE participant.session_id = session.id
                 AND participant.workspace_id = session.workspace_id
                 AND participant.human_id = ?
                 AND participant.deleted_at IS NULL
             )
         )",
    )
    .bind(exemplar.source_transcript_id)
    .bind(exemplar.source_attachment_id)
    .bind(exemplar.source_session_id)
    .bind(exemplar.workspace_id)
    .bind(exemplar.human_id)
    .fetch_one(&mut *transaction)
    .await?;
    if !source_exists {
        return Err(VoiceprintExemplarError::SourceNotFound);
    }

    sqlx::query(
        "INSERT INTO voiceprint_exemplars (
           id, workspace_id, human_id, keyring_scope, keyring_key, sync_scope,
           model_provider, model_version, capture_domain, confirmation_source,
           source_session_id, source_transcript_id, source_attachment_id,
           source_speaker_label, source_start_ms, source_end_ms, quality_score,
           label_confidence
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(exemplar.id)
    .bind(exemplar.workspace_id)
    .bind(exemplar.human_id)
    .bind(VOICEPRINT_KEYRING_SCOPE)
    .bind(exemplar.keyring_key)
    .bind(VOICEPRINT_SYNC_SCOPE)
    .bind(exemplar.model_provider)
    .bind(exemplar.model_version)
    .bind(exemplar.capture_domain)
    .bind(exemplar.confirmation_source)
    .bind(exemplar.source_session_id)
    .bind(exemplar.source_transcript_id)
    .bind(exemplar.source_attachment_id)
    .bind(exemplar.source_speaker_label)
    .bind(exemplar.source_start_ms)
    .bind(exemplar.source_end_ms)
    .bind(exemplar.quality_score)
    .bind(exemplar.label_confidence)
    .execute(&mut *transaction)
    .await?;

    let inserted = get_active_voiceprint_exemplar_in_transaction(&mut transaction, exemplar.id)
        .await?
        .ok_or(VoiceprintExemplarError::InvalidField("id"))?;
    transaction.commit().await?;
    Ok(inserted)
}

pub async fn get_active_voiceprint_exemplar(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<VoiceprintExemplar>, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let exemplar = get_active_voiceprint_exemplar_in_transaction(&mut transaction, id).await?;
    transaction.commit().await?;
    Ok(exemplar)
}

async fn get_active_voiceprint_exemplar_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    id: &str,
) -> Result<Option<VoiceprintExemplar>, sqlx::Error> {
    sqlx::query_as(
        "SELECT
           id, workspace_id, human_id, keyring_scope, keyring_key, sync_scope,
           model_provider, model_version, capture_domain, confirmation_source,
           source_session_id, source_transcript_id, source_attachment_id,
           source_speaker_label, source_start_ms, source_end_ms, quality_score,
           label_confidence, confirmed_at, created_at, updated_at, deleted_at
         FROM voiceprint_exemplars
         WHERE id = ? AND deleted_at IS NULL
         LIMIT 1",
    )
    .bind(id)
    .fetch_optional(&mut **transaction)
    .await
}

pub async fn list_active_voiceprint_exemplars_for_human(
    pool: &SqlitePool,
    workspace_id: &str,
    human_id: &str,
) -> Result<Vec<VoiceprintExemplar>, sqlx::Error> {
    sqlx::query_as(
        "SELECT
           id, workspace_id, human_id, keyring_scope, keyring_key, sync_scope,
           model_provider, model_version, capture_domain, confirmation_source,
           source_session_id, source_transcript_id, source_attachment_id,
           source_speaker_label, source_start_ms, source_end_ms, quality_score,
           label_confidence, confirmed_at, created_at, updated_at, deleted_at
         FROM voiceprint_exemplars
         WHERE workspace_id = ? AND human_id = ? AND deleted_at IS NULL
         ORDER BY created_at, id",
    )
    .bind(workspace_id)
    .bind(human_id)
    .fetch_all(pool)
    .await
}

pub async fn tombstone_voiceprint_exemplar(
    pool: &SqlitePool,
    workspace_id: &str,
    id: &str,
) -> Result<Option<VoiceprintSecretRef>, sqlx::Error> {
    sqlx::query_as(
        "UPDATE voiceprint_exemplars
         SET
           deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ? AND workspace_id = ? AND deleted_at IS NULL
         RETURNING keyring_scope, keyring_key",
    )
    .bind(id)
    .bind(workspace_id)
    .fetch_optional(pool)
    .await
}

pub async fn tombstone_voiceprint_exemplars_for_human(
    pool: &SqlitePool,
    workspace_id: &str,
    human_id: &str,
) -> Result<Vec<VoiceprintSecretRef>, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let secret_refs = sqlx::query_as(
        "SELECT keyring_scope, keyring_key
         FROM voiceprint_exemplars
         WHERE workspace_id = ? AND human_id = ? AND deleted_at IS NULL
         ORDER BY created_at, id",
    )
    .bind(workspace_id)
    .bind(human_id)
    .fetch_all(&mut *transaction)
    .await?;

    sqlx::query(
        "UPDATE voiceprint_exemplars
         SET
           deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE workspace_id = ? AND human_id = ? AND deleted_at IS NULL",
    )
    .bind(workspace_id)
    .bind(human_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(secret_refs)
}

pub async fn purge_tombstoned_voiceprint_exemplar(
    pool: &SqlitePool,
    workspace_id: &str,
    id: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM voiceprint_exemplars
         WHERE id = ? AND workspace_id = ? AND deleted_at IS NOT NULL",
    )
    .bind(id)
    .bind(workspace_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

fn validate_exemplar(exemplar: &NewVoiceprintExemplar<'_>) -> Result<(), VoiceprintExemplarError> {
    for (field, value) in [
        ("id", exemplar.id),
        ("workspace ID", exemplar.workspace_id),
        ("human ID", exemplar.human_id),
        ("keyring key", exemplar.keyring_key),
        ("model provider", exemplar.model_provider),
        ("model version", exemplar.model_version),
        ("capture domain", exemplar.capture_domain),
        ("source session ID", exemplar.source_session_id),
        ("source transcript ID", exemplar.source_transcript_id),
        ("source attachment ID", exemplar.source_attachment_id),
        ("source speaker label", exemplar.source_speaker_label),
    ] {
        if value.trim().is_empty() {
            return Err(VoiceprintExemplarError::InvalidField(field));
        }
    }

    if !CONFIRMATION_SOURCES.contains(&exemplar.confirmation_source) {
        return Err(VoiceprintExemplarError::InvalidField("confirmation source"));
    }
    if exemplar.keyring_key != exemplar.id {
        return Err(VoiceprintExemplarError::InvalidField("keyring key"));
    }
    if exemplar.source_start_ms < 0 || exemplar.source_end_ms <= exemplar.source_start_ms {
        return Err(VoiceprintExemplarError::InvalidField("source span"));
    }
    for (field, value) in [
        ("quality score", exemplar.quality_score),
        ("label confidence", exemplar.label_confidence),
    ] {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(VoiceprintExemplarError::InvalidField(field));
        }
    }

    Ok(())
}
