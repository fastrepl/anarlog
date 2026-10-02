use sqlx::SqlitePool;

use crate::CAPTURE_LIFECYCLE_SETTING_PREFIX;

const SESSION_AUDIO_ATTACHMENT_PREFIX: &str = "session-audio:";

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SessionAudioRetentionCandidate {
    pub session_id: String,
    pub created_at_ms: Option<i64>,
    pub has_words: bool,
}

pub async fn get_app_setting_json(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT value_json FROM app_settings WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

macro_rules! session_audio_retention_candidates_sql {
    ($tail:literal) => {
        concat!(
            "SELECT
           session.id AS session_id,
           CAST(round((julianday(session.created_at) - 2440587.5) * 86400000.0) AS INTEGER)
             AS created_at_ms,
           EXISTS(
             SELECT 1
             FROM transcripts AS transcript
             WHERE transcript.session_id = session.id
               AND transcript.deleted_at IS NULL
               AND json_valid(transcript.words_json)
               AND json_array_length(transcript.words_json) > 0
           ) AS has_words
         FROM sessions AS session
         WHERE session.deleted_at IS NULL
           AND NOT EXISTS(
             SELECT 1
             FROM session_attachments AS audio
             WHERE audio.session_id = session.id
               AND audio.source_type = 'session_audio'
               AND audio.source_id = 'primary'
               AND audio.deleted_at IS NULL
               AND json_valid(audio.metadata_json)
               AND json_extract(audio.metadata_json, '$.transcript_status') = 'processing'
           )
           AND NOT EXISTS(
             SELECT 1
             FROM app_settings AS capture
             WHERE capture.id = ? || session.id
           ) ",
            $tail
        )
    };
}

/// Live sessions whose local audio is not held by transcript processing or a
/// pending capture lifecycle marker.
pub async fn list_session_audio_retention_candidates(
    pool: &SqlitePool,
) -> Result<Vec<SessionAudioRetentionCandidate>, sqlx::Error> {
    sqlx::query_as(session_audio_retention_candidates_sql!(
        "ORDER BY session.created_at, session.id"
    ))
    .bind(CAPTURE_LIFECYCLE_SETTING_PREFIX)
    .fetch_all(pool)
    .await
}

pub async fn get_session_audio_retention_candidate(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<SessionAudioRetentionCandidate>, sqlx::Error> {
    sqlx::query_as(session_audio_retention_candidates_sql!(
        "AND session.id = ?"
    ))
    .bind(CAPTURE_LIFECYCLE_SETTING_PREFIX)
    .bind(session_id)
    .fetch_optional(pool)
    .await
}

/// Sessions whose primary audio attachment is tombstoned but may still exist
/// on this device.
pub async fn list_logically_deleted_session_audio(
    pool: &SqlitePool,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT DISTINCT attachment.session_id
         FROM session_attachments AS attachment
         LEFT JOIN attachment_local_state AS local
           ON local.attachment_id = attachment.id
         WHERE attachment.source_type = 'session_audio'
           AND attachment.source_id = 'primary'
           AND attachment.deleted_at IS NOT NULL
           AND COALESCE(local.availability, 'present') != 'absent'
         ORDER BY attachment.session_id",
    )
    .fetch_all(pool)
    .await
}

pub async fn session_audio_is_logically_deleted(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1
           FROM session_attachments AS attachment
           WHERE attachment.id = ?
             AND attachment.session_id = ?
             AND attachment.deleted_at IS NOT NULL
             AND NOT EXISTS(
               SELECT 1
               FROM attachment_local_state AS local
               WHERE local.attachment_id = attachment.id
                 AND local.availability = 'absent'
             )
         )",
    )
    .bind(session_audio_attachment_id(session_id))
    .bind(session_id)
    .fetch_one(pool)
    .await
}

/// The session has transcript words and no primary audio transcription in progress.
pub async fn session_audio_is_processed(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT
           EXISTS(
             SELECT 1
             FROM transcripts
             WHERE session_id = ?
               AND deleted_at IS NULL
               AND json_valid(words_json)
               AND json_array_length(words_json) > 0
           )
           AND NOT EXISTS(
             SELECT 1
             FROM session_attachments
             WHERE session_id = ?
               AND source_type = 'session_audio'
               AND source_id = 'primary'
               AND deleted_at IS NULL
               AND json_valid(metadata_json)
               AND json_extract(metadata_json, '$.transcript_status') = 'processing'
           )",
    )
    .bind(session_id)
    .bind(session_id)
    .fetch_one(pool)
    .await
}

pub async fn mark_session_audio_absent(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO attachment_local_state (
           attachment_id, session_id, relative_path, availability, updated_at
         ) VALUES (?, ?, '', 'absent', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(attachment_id) DO UPDATE SET
           session_id = excluded.session_id,
           availability = excluded.availability,
           updated_at = excluded.updated_at",
    )
    .bind(session_audio_attachment_id(session_id))
    .bind(session_id)
    .execute(pool)
    .await?;
    Ok(())
}

fn session_audio_attachment_id(session_id: &str) -> String {
    format!("{SESSION_AUDIO_ATTACHMENT_PREFIX}{session_id}")
}
