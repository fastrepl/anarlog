use sqlx::SqliteConnection;

pub const PILOT_METADATA_LIMIT: usize = 20_000;

#[derive(sqlx::FromRow)]
pub struct PilotSettingRow {
    pub id: String,
    pub value_json: String,
}

/// Only capture clocks and their measurement ledger; never notes, transcripts,
/// attendee fields, provider configuration or lifecycle markers' raw memo.
pub async fn list_pilot_setting_rows(
    conn: &mut SqliteConnection,
) -> Result<Vec<PilotSettingRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT substr(id,1,1100) AS id, substr(value_json,1,2049) AS value_json FROM app_settings
         WHERE id GLOB 'capture_usage:*' OR id GLOB 'capture_stop:*'
         ORDER BY id LIMIT ?",
    )
    .bind((PILOT_METADATA_LIMIT + 1) as i64)
    .fetch_all(&mut *conn)
    .await
}

pub async fn count_pilot_pending_sessions(
    conn: &mut SqliteConnection,
    until_ms: i64,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM (
          SELECT substr(id,length('capture_lifecycle_pending:')+1) AS session_id
          FROM app_settings
          WHERE id GLOB 'capture_lifecycle_pending:*'
            AND CASE WHEN json_valid(value_json) THEN
              CASE WHEN json_type(value_json,'$.startedAt') IN ('integer','real') THEN
                COALESCE(json_extract(value_json,'$.startedAt') < ?,1)
                ELSE 1 END
              ELSE 1 END
          UNION
          SELECT substr(id,length('capture_audio_saved:')+1) AS session_id
          FROM app_settings WHERE id GLOB 'capture_audio_saved:*'
        )",
    )
    .bind(until_ms)
    .fetch_one(&mut *conn)
    .await
}

pub async fn list_pilot_legacy_session_candidates(
    conn: &mut SqliteConnection,
    from_ms: i64,
    until_ms: i64,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT DISTINCT substr(session_id,1,501) AS session_id FROM transcripts
         WHERE source IN ('live_capture','batch_transcription')
           AND started_at_ms >= ? AND started_at_ms < ?
         ORDER BY session_id LIMIT ?",
    )
    .bind(from_ms)
    .bind(until_ms)
    .bind((PILOT_METADATA_LIMIT + 1) as i64)
    .fetch_all(&mut *conn)
    .await
}
