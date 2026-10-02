use sqlx::SqliteConnection;

pub struct SessionAudioMove<'a> {
    pub source_session_id: &'a str,
    pub target_session_id: &'a str,
    pub rewrite_audio_ids: bool,
    pub source_audio_id: &'a str,
    pub target_audio_id: &'a str,
    pub now: &'a str,
}

pub async fn move_session_transcripts(
    conn: &mut SqliteConnection,
    move_request: &SessionAudioMove<'_>,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE transcripts
        SET
          session_id = ?,
          audio_attachment_id = CASE
            WHEN ? = 1 AND audio_attachment_id = ? THEN ?
            ELSE audio_attachment_id
          END,
          updated_at = ?
        WHERE session_id = ? AND deleted_at IS NULL",
    )
    .bind(move_request.target_session_id)
    .bind(i64::from(move_request.rewrite_audio_ids))
    .bind(move_request.source_audio_id)
    .bind(move_request.target_audio_id)
    .bind(move_request.now)
    .bind(move_request.source_session_id)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}

pub async fn move_session_summary_documents(
    conn: &mut SqliteConnection,
    source_session_id: &str,
    target_session_id: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE session_documents
        SET session_id = ?, updated_at = ?
        WHERE session_id = ?
          AND kind IN ('summary', 'template_output')
          AND deleted_at IS NULL",
    )
    .bind(target_session_id)
    .bind(now)
    .bind(source_session_id)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}

pub async fn move_session_action_items(
    conn: &mut SqliteConnection,
    source_session_id: &str,
    target_session_id: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE action_items
        SET session_id = ?, updated_at = ?
        WHERE session_id = ? AND deleted_at IS NULL",
    )
    .bind(target_session_id)
    .bind(now)
    .bind(source_session_id)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}

pub async fn move_session_voiceprint_exemplars(
    conn: &mut SqliteConnection,
    move_request: &SessionAudioMove<'_>,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE voiceprint_exemplars
        SET
          source_session_id = ?,
          source_attachment_id = CASE
            WHEN ? = 1 AND source_attachment_id = ? THEN ?
            ELSE source_attachment_id
          END,
          updated_at = ?
        WHERE source_session_id = ? AND deleted_at IS NULL",
    )
    .bind(move_request.target_session_id)
    .bind(i64::from(move_request.rewrite_audio_ids))
    .bind(move_request.source_audio_id)
    .bind(move_request.target_audio_id)
    .bind(move_request.now)
    .bind(move_request.source_session_id)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}

pub async fn move_session_voiceprint_candidates(
    conn: &mut SqliteConnection,
    move_request: &SessionAudioMove<'_>,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE voiceprint_candidates
        SET
          source_session_id = ?,
          source_attachment_id = CASE
            WHEN ? = 1 AND source_attachment_id = ? THEN ?
            ELSE source_attachment_id
          END,
          updated_at = ?
        WHERE source_session_id = ? AND deleted_at IS NULL",
    )
    .bind(move_request.target_session_id)
    .bind(i64::from(move_request.rewrite_audio_ids))
    .bind(move_request.source_audio_id)
    .bind(move_request.target_audio_id)
    .bind(move_request.now)
    .bind(move_request.source_session_id)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}

pub async fn rewrite_session_note_body(
    conn: &mut SqliteConnection,
    document_id: &str,
    session_id: &str,
    body: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE session_documents
        SET body = ?, body_format = 'prosemirror_json', updated_at = ?
        WHERE id = ?
          AND session_id = ?
          AND kind = 'note'
          AND deleted_at IS NULL",
    )
    .bind(body)
    .bind(now)
    .bind(document_id)
    .bind(session_id)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}
