use sqlx::SqliteConnection;

pub struct SessionDocumentBodyUpdate<'a> {
    pub id: &'a str,
    pub session_id: &'a str,
    pub current_body: &'a str,
    pub current_body_format: &'a str,
    pub next_body: &'a str,
    pub updated_at: &'a str,
}

pub struct TranscriptContentUpdate<'a> {
    pub id: &'a str,
    pub session_id: &'a str,
    pub current_words_json: &'a str,
    pub current_memo: &'a str,
    pub next_words_json: &'a str,
    pub next_memo: &'a str,
    pub updated_at: &'a str,
}

pub async fn update_session_title(
    conn: &mut SqliteConnection,
    session_id: &str,
    current_title: &str,
    next_title: &str,
    updated_at: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE sessions
         SET title = ?, updated_at = ?
         WHERE id = ? AND title = ? AND deleted_at IS NULL",
    )
    .bind(next_title)
    .bind(updated_at)
    .bind(session_id)
    .bind(current_title)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}

pub async fn update_title_document(
    conn: &mut SqliteConnection,
    update: SessionDocumentBodyUpdate<'_>,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE session_documents
         SET body = ?, body_format = 'prosemirror_json', updated_at = ?
         WHERE id = ?
           AND session_id = ?
           AND kind IN ('note', 'summary', 'template_output')
           AND body = ?
           AND body_format = ?
           AND deleted_at IS NULL",
    )
    .bind(update.next_body)
    .bind(update.updated_at)
    .bind(update.id)
    .bind(update.session_id)
    .bind(update.current_body)
    .bind(update.current_body_format)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}

pub async fn update_summary_document(
    conn: &mut SqliteConnection,
    update: SessionDocumentBodyUpdate<'_>,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE session_documents
         SET body = ?, body_format = 'prosemirror_json', updated_at = ?
         WHERE id = ?
           AND session_id = ?
           AND kind IN ('summary', 'template_output')
           AND body = ?
           AND body_format = ?
           AND deleted_at IS NULL",
    )
    .bind(update.next_body)
    .bind(update.updated_at)
    .bind(update.id)
    .bind(update.session_id)
    .bind(update.current_body)
    .bind(update.current_body_format)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}

pub async fn update_session_transcript(
    conn: &mut SqliteConnection,
    update: TranscriptContentUpdate<'_>,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE transcripts
         SET words_json = ?, memo = ?, updated_at = ?
         WHERE id = ?
           AND session_id = ?
           AND words_json = ?
           AND memo = ?
           AND deleted_at IS NULL",
    )
    .bind(update.next_words_json)
    .bind(update.next_memo)
    .bind(update.updated_at)
    .bind(update.id)
    .bind(update.session_id)
    .bind(update.current_words_json)
    .bind(update.current_memo)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}
