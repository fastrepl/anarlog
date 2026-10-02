use anlg_db_app::{
    SessionDocumentBodyUpdate, TranscriptContentUpdate, update_session_title,
    update_session_transcript, update_summary_document,
    update_title_document as update_generated_title_document,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use crate::transaction_utils::{js_iso8601_timestamp, rollback_row_count_mismatch};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SessionDocumentUpdate {
    pub id: String,
    pub current_body: String,
    pub current_body_format: String,
    pub next_body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SaveGeneratedTitleRequest {
    pub session_id: String,
    pub current_title: String,
    pub next_title: String,
    pub documents: Vec<SessionDocumentUpdate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct TitleCorrection {
    pub current_title: String,
    pub next_title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct TranscriptCorrection {
    pub id: String,
    pub current_words_json: String,
    pub current_memo: String,
    pub next_words_json: String,
    pub next_memo: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SessionContentCorrectionsRequest {
    pub session_id: String,
    pub summaries: Vec<SessionDocumentUpdate>,
    pub transcripts: Vec<TranscriptCorrection>,
    pub title: Option<TitleCorrection>,
}

pub async fn save_generated_title(
    pool: &SqlitePool,
    request: SaveGeneratedTitleRequest,
) -> Result<(), String> {
    let timestamp = js_iso8601_timestamp();
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    let updated = update_session_title(
        &mut transaction,
        &request.session_id,
        &request.current_title,
        &request.next_title,
        &timestamp,
    )
    .await
    .map_err(|error| error.to_string())?;
    if updated != 1 {
        return Err(rollback_row_count_mismatch(transaction, 0, updated, 1).await);
    }

    for (index, document) in request.documents.iter().enumerate() {
        let updated =
            update_title_document(&mut transaction, &request.session_id, document, &timestamp)
                .await
                .map_err(|error| error.to_string())?;
        if updated != 1 {
            return Err(rollback_row_count_mismatch(transaction, index + 1, updated, 1).await);
        }
    }

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}

pub async fn apply_session_content_corrections(
    pool: &SqlitePool,
    request: SessionContentCorrectionsRequest,
) -> Result<(), String> {
    let update_title = request
        .title
        .as_ref()
        .is_some_and(|title| title.current_title != title.next_title);
    if !update_title && request.summaries.is_empty() && request.transcripts.is_empty() {
        return Ok(());
    }

    let timestamp = js_iso8601_timestamp();
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;
    let mut statement_index = 0;

    if update_title {
        let title = request.title.as_ref().unwrap();
        let updated = update_session_title(
            &mut transaction,
            &request.session_id,
            &title.current_title,
            &title.next_title,
            &timestamp,
        )
        .await
        .map_err(|error| error.to_string())?;
        if updated != 1 {
            return Err(
                rollback_row_count_mismatch(transaction, statement_index, updated, 1).await,
            );
        }
        statement_index += 1;
    }

    for summary in &request.summaries {
        let updated = update_summary_document(
            &mut transaction,
            SessionDocumentBodyUpdate {
                id: &summary.id,
                session_id: &request.session_id,
                current_body: &summary.current_body,
                current_body_format: &summary.current_body_format,
                next_body: &summary.next_body,
                updated_at: &timestamp,
            },
        )
        .await
        .map_err(|error| error.to_string())?;
        if updated != 1 {
            return Err(
                rollback_row_count_mismatch(transaction, statement_index, updated, 1).await,
            );
        }
        statement_index += 1;
    }

    for transcript in &request.transcripts {
        let updated = update_session_transcript(
            &mut transaction,
            TranscriptContentUpdate {
                id: &transcript.id,
                session_id: &request.session_id,
                current_words_json: &transcript.current_words_json,
                current_memo: &transcript.current_memo,
                next_words_json: &transcript.next_words_json,
                next_memo: &transcript.next_memo,
                updated_at: &timestamp,
            },
        )
        .await
        .map_err(|error| error.to_string())?;
        if updated != 1 {
            return Err(
                rollback_row_count_mismatch(transaction, statement_index, updated, 1).await,
            );
        }
        statement_index += 1;
    }

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}

async fn update_title_document(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    session_id: &str,
    document: &SessionDocumentUpdate,
    timestamp: &str,
) -> Result<u64, sqlx::Error> {
    update_generated_title_document(
        transaction,
        SessionDocumentBodyUpdate {
            id: &document.id,
            session_id,
            current_body: &document.current_body,
            current_body_format: &document.current_body_format,
            next_body: &document.next_body,
            updated_at: timestamp,
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_db_core::Db;
    use sqlx::Row;

    async fn test_db() -> Db {
        let db = Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-1', 'user-1', '')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO session_documents (
                id, workspace_id, session_id, kind, body, body_format
             ) VALUES
                ('note-1', 'workspace-1', 'session-1', 'note', 'old note', 'markdown'),
                ('summary-1', 'workspace-1', 'session-1', 'summary', 'old summary', 'markdown')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO transcripts (
                id, workspace_id, owner_user_id, session_id, words_json, memo
             ) VALUES (
                'transcript-1', 'workspace-1', 'user-1', 'session-1',
                '[{\"word\":\"old\"}]', 'old memo'
             )",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db
    }

    fn document(
        id: &str,
        current_body: &str,
        current_body_format: &str,
        next_body: &str,
    ) -> SessionDocumentUpdate {
        SessionDocumentUpdate {
            id: id.to_string(),
            current_body: current_body.to_string(),
            current_body_format: current_body_format.to_string(),
            next_body: next_body.to_string(),
        }
    }

    #[tokio::test]
    async fn saves_generated_title_and_note_document() {
        let db = test_db().await;
        save_generated_title(
            db.pool(),
            SaveGeneratedTitleRequest {
                session_id: "session-1".to_string(),
                current_title: String::new(),
                next_title: "Planning".to_string(),
                documents: vec![document(
                    "note-1",
                    "old note",
                    "markdown",
                    r#"{"type":"doc","content":[]}"#,
                )],
            },
        )
        .await
        .unwrap();

        let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let document_row =
            sqlx::query("SELECT body, body_format FROM session_documents WHERE id = 'note-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(title, "Planning");
        assert_eq!(
            document_row.try_get::<String, _>("body").unwrap(),
            r#"{"type":"doc","content":[]}"#
        );
        assert_eq!(
            document_row.try_get::<String, _>("body_format").unwrap(),
            "prosemirror_json"
        );
    }

    #[tokio::test]
    async fn stale_document_rolls_back_generated_title() {
        let db = test_db().await;
        let error = save_generated_title(
            db.pool(),
            SaveGeneratedTitleRequest {
                session_id: "session-1".to_string(),
                current_title: String::new(),
                next_title: "Planning".to_string(),
                documents: vec![document(
                    "note-1",
                    "stale note",
                    "markdown",
                    r#"{"type":"doc","content":[]}"#,
                )],
            },
        )
        .await
        .unwrap_err();

        let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let body: String =
            sqlx::query_scalar("SELECT body FROM session_documents WHERE id = 'note-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(error, "transaction statement 1 affected 0 rows; expected 1");
        assert_eq!(title, "");
        assert_eq!(body, "old note");
    }

    #[tokio::test]
    async fn applies_title_summary_and_transcript_corrections() {
        let db = test_db().await;
        apply_session_content_corrections(
            db.pool(),
            SessionContentCorrectionsRequest {
                session_id: "session-1".to_string(),
                summaries: vec![document(
                    "summary-1",
                    "old summary",
                    "markdown",
                    r#"{"type":"doc","content":[]}"#,
                )],
                transcripts: vec![TranscriptCorrection {
                    id: "transcript-1".to_string(),
                    current_words_json: r#"[{"word":"old"}]"#.to_string(),
                    current_memo: "old memo".to_string(),
                    next_words_json: r#"[{"word":"new"}]"#.to_string(),
                    next_memo: "new memo".to_string(),
                }],
                title: Some(TitleCorrection {
                    current_title: String::new(),
                    next_title: "Corrected".to_string(),
                }),
            },
        )
        .await
        .unwrap();

        let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let summary =
            sqlx::query("SELECT body, body_format FROM session_documents WHERE id = 'summary-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let transcript =
            sqlx::query("SELECT words_json, memo FROM transcripts WHERE id = 'transcript-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(title, "Corrected");
        assert_eq!(
            summary.try_get::<String, _>("body").unwrap(),
            r#"{"type":"doc","content":[]}"#
        );
        assert_eq!(
            summary.try_get::<String, _>("body_format").unwrap(),
            "prosemirror_json"
        );
        assert_eq!(
            transcript.try_get::<String, _>("words_json").unwrap(),
            r#"[{"word":"new"}]"#
        );
        assert_eq!(transcript.try_get::<String, _>("memo").unwrap(), "new memo");
    }

    #[tokio::test]
    async fn stale_transcript_memo_rolls_back_title_and_summary() {
        let db = test_db().await;
        let error = apply_session_content_corrections(
            db.pool(),
            SessionContentCorrectionsRequest {
                session_id: "session-1".to_string(),
                summaries: vec![document(
                    "summary-1",
                    "old summary",
                    "markdown",
                    r#"{"type":"doc","content":[]}"#,
                )],
                transcripts: vec![TranscriptCorrection {
                    id: "transcript-1".to_string(),
                    current_words_json: r#"[{"word":"old"}]"#.to_string(),
                    current_memo: "stale memo".to_string(),
                    next_words_json: r#"[{"word":"new"}]"#.to_string(),
                    next_memo: "new memo".to_string(),
                }],
                title: Some(TitleCorrection {
                    current_title: String::new(),
                    next_title: "Corrected".to_string(),
                }),
            },
        )
        .await
        .unwrap_err();

        let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let summary_body: String =
            sqlx::query_scalar("SELECT body FROM session_documents WHERE id = 'summary-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(error, "transaction statement 2 affected 0 rows; expected 1");
        assert_eq!(title, "");
        assert_eq!(summary_body, "old summary");
    }
}
