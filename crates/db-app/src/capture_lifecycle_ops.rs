use sqlx::SqlitePool;

pub const CAPTURE_LIFECYCLE_SETTING_PREFIX: &str = "capture_lifecycle_pending:";
pub const CAPTURE_AUDIO_SAVED_SETTING_PREFIX: &str = "capture_audio_saved:";

pub async fn upsert_capture_lifecycle_marker(
    pool: &SqlitePool,
    session_id: &str,
    value_json: &str,
    replace_transcript_id: &str,
) -> Result<bool, sqlx::Error> {
    let id = format!("{CAPTURE_LIFECYCLE_SETTING_PREFIX}{session_id}");
    let result = sqlx::query(
        "INSERT INTO app_settings (id, value_json, updated_at)
         VALUES (?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(id) DO UPDATE SET
           value_json = excluded.value_json,
           updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE json_valid(app_settings.value_json)
           AND json_extract(app_settings.value_json, '$.transcriptId') IN (
             json_extract(excluded.value_json, '$.transcriptId'),
             ?
           )",
    )
    .bind(id)
    .bind(value_json)
    .bind(replace_transcript_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn delete_capture_lifecycle_marker(
    pool: &SqlitePool,
    session_id: &str,
    transcript_id: &str,
) -> Result<bool, sqlx::Error> {
    let id = format!("{CAPTURE_LIFECYCLE_SETTING_PREFIX}{session_id}");
    let result = sqlx::query(
        "DELETE FROM app_settings
         WHERE id = ?
           AND json_valid(value_json)
           AND json_extract(
             CASE
               WHEN json_valid(value_json) THEN value_json
               ELSE '{}'
             END,
             '$.transcriptId'
           ) = ?",
    )
    .bind(id)
    .bind(transcript_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn get_capture_lifecycle_marker_json(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<String>, sqlx::Error> {
    let id = format!("{CAPTURE_LIFECYCLE_SETTING_PREFIX}{session_id}");
    sqlx::query_scalar("SELECT value_json FROM app_settings WHERE id = ? LIMIT 1")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn list_capture_lifecycle_marker_jsons(
    pool: &SqlitePool,
) -> Result<Vec<(String, String)>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT id, value_json
         FROM app_settings
         WHERE id GLOB 'capture_lifecycle_pending:*'
         ORDER BY updated_at, id",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|(id, value_json)| {
            id.strip_prefix(CAPTURE_LIFECYCLE_SETTING_PREFIX)
                .map(|session_id| (session_id.to_string(), value_json))
        })
        .collect())
}

pub async fn mark_capture_audio_saved(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<(), sqlx::Error> {
    let id = format!("{CAPTURE_AUDIO_SAVED_SETTING_PREFIX}{session_id}");
    sqlx::query(
        "INSERT INTO app_settings (id, value_json, updated_at)
         VALUES (?, '{}', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(id) DO UPDATE SET updated_at = excluded.updated_at",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn clear_capture_audio_saved(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<(), sqlx::Error> {
    let id = format!("{CAPTURE_AUDIO_SAVED_SETTING_PREFIX}{session_id}");
    sqlx::query("DELETE FROM app_settings WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
