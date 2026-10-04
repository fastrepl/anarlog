use sqlx::{Row, Sqlite, Transaction};

/// Physical local deletion after the caller has locked the database and
/// verified the exported snapshot. Refuse replicas and shared/cloud content;
/// their deletion requires the owning sync protocol rather than a local purge.
pub async fn purge_local_session(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> Result<(), sqlx::Error> {
    let cloud_disabled: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM app_settings WHERE id='cloud_sync_enabled' AND value_json='false')").fetch_one(&mut **tx).await?;
    let cloud_attachment: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM session_attachments WHERE session_id=? AND cloud_object_key<>'')").bind(id).fetch_one(&mut **tx).await?;
    let shared: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM shared_session_cache WHERE session_id=?)")
            .bind(id)
            .fetch_one(&mut **tx)
            .await?;
    let replicas: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM e2ee_local_state WHERE (table_name='sessions' AND row_id=?) OR (table_name='transcripts' AND row_id IN (SELECT id FROM transcripts WHERE session_id=?)) OR (table_name='session_documents' AND row_id IN (SELECT id FROM session_documents WHERE session_id=?)) OR (table_name='session_attachments' AND row_id IN (SELECT id FROM session_attachments WHERE session_id=?)))")
        .bind(id).bind(id).bind(id).bind(id).fetch_one(&mut **tx).await?;
    if !cloud_disabled || cloud_attachment || shared || replicas {
        return Err(sqlx::Error::Protocol(
            "local purge requires unsynced local content".into(),
        ));
    }
    // Resolve exact owners while transcript rows still exist. Prefix matching
    // would include another session whose ID contains this ID plus a colon.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DELETE FROM app_settings WHERE ({})",
        super::session_hold::capture_stop_session_predicate()
    )))
    .bind(id)
    .execute(&mut **tx)
    .await?;
    // Live partials are keyed by transcript, not session. Removing only the
    // final transcript would leave the original words in this journal.
    sqlx::query("DELETE FROM transcript_live_state WHERE transcript_id IN (SELECT id FROM transcripts WHERE session_id=?)")
        .bind(id).execute(&mut **tx).await?;
    sqlx::query("DELETE FROM voiceprint_exemplars WHERE source_session_id=?")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    // Only names enumerated by SQLite and quoted as identifiers are used.
    // This includes document history, proposals and local attachment jobs.
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
    )
    .fetch_all(&mut **tx)
    .await?;
    for table in tables {
        let quoted = format!("\"{}\"", table.replace('"', "\"\""));
        let columns = sqlx::query(sqlx::AssertSqlSafe(format!("PRAGMA table_info({quoted})")))
            .fetch_all(&mut **tx)
            .await?;
        if columns
            .iter()
            .any(|r| r.get::<String, _>("name") == "session_id")
        {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "DELETE FROM {quoted} WHERE session_id=?"
            )))
            .bind(id)
            .execute(&mut **tx)
            .await?;
        }
    }
    sqlx::query("DELETE FROM entity_mentions WHERE (source_type='session' AND source_id=?) OR (target_type='session' AND target_id=?)").bind(id).bind(id).execute(&mut **tx).await?;
    sqlx::query("DELETE FROM app_settings WHERE id=?")
        .bind(format!("auto_enhance_pending:{id}"))
        .execute(&mut **tx)
        .await?;
    sqlx::query("DELETE FROM sessions WHERE id=?")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
