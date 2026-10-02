use sqlx::SqliteConnection;

const NOTE_CONFLICT_FIELDS_SQL: &str = "
  (table_name = 'session_documents' AND field_name = 'body')
  OR (table_name = 'sessions' AND field_name = 'title')
";

#[allow(clippy::too_many_arguments)]
pub async fn insert_pending_session_proposal(
    conn: &mut SqliteConnection,
    id: &str,
    session_id: &str,
    kind: &str,
    target_id: &str,
    base_updated_at: &str,
    current_markdown: &str,
    proposed_markdown: &str,
    source: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO session_proposals (
          id, session_id, kind, target_id, base_updated_at,
          current_markdown, proposed_markdown, status, source
        ) VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', ?)",
    )
    .bind(id)
    .bind(session_id)
    .bind(kind)
    .bind(target_id)
    .bind(base_updated_at)
    .bind(current_markdown)
    .bind(proposed_markdown)
    .bind(source)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}

pub async fn set_pending_session_proposal_status(
    conn: &mut SqliteConnection,
    proposal_id: &str,
    status: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE session_proposals
        SET status = ?, updated_at = ?
        WHERE id = ? AND status = 'pending'",
    )
    .bind(status)
    .bind(now)
    .bind(proposal_id)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}

pub async fn resolve_session_note_conflicts(
    conn: &mut SqliteConnection,
    session_id: &str,
) -> Result<u64, sqlx::Error> {
    let sql = format!(
        "UPDATE e2ee_field_conflicts
        SET resolved_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE row_id = ?
          AND resolved_at IS NULL
          AND ({NOTE_CONFLICT_FIELDS_SQL})"
    );
    let result = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(session_id)
        .execute(&mut *conn)
        .await?;

    Ok(result.rows_affected())
}

pub async fn resolve_session_conflict(
    conn: &mut SqliteConnection,
    conflict_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE e2ee_field_conflicts
        SET resolved_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ? AND resolved_at IS NULL",
    )
    .bind(conflict_id)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}
