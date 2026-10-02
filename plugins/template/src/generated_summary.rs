use std::collections::HashSet;

use crate::transaction_utils::{js_iso8601_timestamp, rollback_row_count_mismatch};
use anlg_db_app::{
    GeneratedSummaryBodyUpdate, PendingAutoEnhanceMatch, delete_pending_auto_enhance,
    update_generated_summary_body, upsert_session_tag, upsert_tag,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

const DEFAULT_USER_ID: &str = "00000000-0000-0000-0000-000000000000";
const PENDING_AUTO_ENHANCE_SETTING_PREFIX: &str = "auto_enhance_pending:";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct PendingAutoEnhanceGuard {
    pub generation: String,
    pub expected_body: String,
    pub expected_body_format: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SaveGeneratedSummaryRequest {
    pub session_id: String,
    pub owner_user_id: String,
    pub note_id: String,
    pub current_body: String,
    pub current_body_format: String,
    pub next_body: String,
    pub tag_names: Vec<String>,
    pub pending_auto_enhance: Option<PendingAutoEnhanceGuard>,
}

pub async fn save_generated_summary(
    pool: &SqlitePool,
    request: SaveGeneratedSummaryRequest,
) -> Result<(), String> {
    let SaveGeneratedSummaryRequest {
        session_id,
        owner_user_id,
        note_id,
        current_body,
        current_body_format,
        next_body,
        tag_names,
        pending_auto_enhance,
    } = request;

    let timestamp = js_iso8601_timestamp();
    let trimmed_user_id = owner_user_id.trim();
    let user_id = if trimmed_user_id.is_empty() {
        DEFAULT_USER_ID
    } else {
        trimmed_user_id
    };
    let pending_setting_id = format!("{PENDING_AUTO_ENHANCE_SETTING_PREFIX}{session_id}");
    let pending_match = pending_auto_enhance
        .as_ref()
        .map(|pending| PendingAutoEnhanceMatch {
            pending_setting_id: &pending_setting_id,
            generation: &pending.generation,
            expected_body: &pending.expected_body,
            expected_body_format: &pending.expected_body_format,
        });
    let mut unique_tags = HashSet::new();
    let tag_names = tag_names
        .into_iter()
        .filter(|tag_name| !tag_name.is_empty() && unique_tags.insert(tag_name.clone()))
        .collect::<Vec<_>>();

    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    let updated = update_generated_summary_body(
        &mut transaction,
        GeneratedSummaryBodyUpdate {
            note_id: &note_id,
            session_id: &session_id,
            current_body: &current_body,
            current_body_format: &current_body_format,
            next_body: &next_body,
            updated_at: &timestamp,
            pending_auto_enhance: pending_match,
        },
    )
    .await
    .map_err(|error| error.to_string())?;
    if updated != 1 {
        return Err(rollback_row_count_mismatch(transaction, 0, updated, 1).await);
    }

    let deleted = delete_pending_auto_enhance(
        &mut transaction,
        &pending_setting_id,
        &note_id,
        &current_body,
        pending_match.as_ref(),
    )
    .await
    .map_err(|error| error.to_string())?;
    if pending_match.is_some() && deleted != 1 {
        return Err(rollback_row_count_mismatch(transaction, 1, deleted, 1).await);
    }

    let mut statement_index = 2;
    for tag_name in tag_names {
        let tag_rows = upsert_tag(&mut transaction, user_id, &tag_name, &timestamp)
            .await
            .map_err(|error| error.to_string())?;
        if tag_rows != 1 {
            return Err(
                rollback_row_count_mismatch(transaction, statement_index, tag_rows, 1).await,
            );
        }
        statement_index += 1;

        let session_tag_rows = upsert_session_tag(
            &mut transaction,
            &session_id,
            user_id,
            &tag_name,
            &timestamp,
        )
        .await
        .map_err(|error| error.to_string())?;
        if session_tag_rows != 1 {
            return Err(rollback_row_count_mismatch(
                transaction,
                statement_index,
                session_tag_rows,
                1,
            )
            .await);
        }
        statement_index += 1;
    }

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_db_core::Db;
    use serde_json::json;
    use sqlx::Row;

    async fn test_db() -> Db {
        let db = Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id)
             VALUES ('session-1', 'workspace-1', 'user-1')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO session_documents (
                id, workspace_id, session_id, kind, body, body_format
             ) VALUES (
                'note-1', 'workspace-1', 'session-1',
                'summary', 'old body', 'markdown'
             )",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db
    }

    fn request(tag_names: Vec<&str>) -> SaveGeneratedSummaryRequest {
        SaveGeneratedSummaryRequest {
            session_id: "session-1".to_string(),
            owner_user_id: " user-1 ".to_string(),
            note_id: "note-1".to_string(),
            current_body: "old body".to_string(),
            current_body_format: "markdown".to_string(),
            next_body: r#"{"type":"doc","content":[]}"#.to_string(),
            tag_names: tag_names.into_iter().map(str::to_string).collect(),
            pending_auto_enhance: None,
        }
    }

    async fn insert_pending_setting(db: &Db, value: serde_json::Value) {
        sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
            .bind("auto_enhance_pending:session-1")
            .bind(value.to_string())
            .execute(db.pool())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn saves_summary_tags_and_removes_matching_pending_setting_without_guard() {
        let db = test_db().await;
        insert_pending_setting(
            &db,
            json!({
                "noteId": "note-1",
                "body": "old body",
                "bodyFormat": "markdown"
            }),
        )
        .await;

        let mut request = request(vec!["launch", "launch", "", "prep"]);
        request.owner_user_id = "  ".to_string();
        save_generated_summary(db.pool(), request).await.unwrap();

        let row = sqlx::query(
            "SELECT body, body_format, updated_at
             FROM session_documents WHERE id = 'note-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(
            row.try_get::<String, _>("body").unwrap(),
            r#"{"type":"doc","content":[]}"#
        );
        assert_eq!(
            row.try_get::<String, _>("body_format").unwrap(),
            "prosemirror_json"
        );
        let updated_at: String = row.try_get("updated_at").unwrap();
        assert_eq!(updated_at.len(), 24);
        assert_eq!(&updated_at[19..20], ".");
        assert!(updated_at.ends_with('Z'));

        let tag_rows: Vec<(String, String)> =
            sqlx::query_as("SELECT id, owner_user_id FROM tags ORDER BY id")
                .fetch_all(db.pool())
                .await
                .unwrap();
        assert_eq!(
            tag_rows,
            [
                (
                    "launch".to_string(),
                    "00000000-0000-0000-0000-000000000000".to_string()
                ),
                (
                    "prep".to_string(),
                    "00000000-0000-0000-0000-000000000000".to_string()
                ),
            ]
        );
        let session_tag_ids: Vec<String> =
            sqlx::query_scalar("SELECT id FROM session_tags ORDER BY id")
                .fetch_all(db.pool())
                .await
                .unwrap();
        assert_eq!(session_tag_ids, ["session-1:launch", "session-1:prep"]);
        let pending_count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM app_settings
             WHERE id = 'auto_enhance_pending:session-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(pending_count, 0);
    }

    #[tokio::test]
    async fn matching_pending_guard_saves_summary_and_removes_setting() {
        let db = test_db().await;
        insert_pending_setting(
            &db,
            json!({
                "noteId": "note-1",
                "generation": "gen-1",
                "body": "old body",
                "bodyFormat": "markdown"
            }),
        )
        .await;
        let mut request = request(vec![]);
        request.pending_auto_enhance = Some(PendingAutoEnhanceGuard {
            generation: "gen-1".to_string(),
            expected_body: "old body".to_string(),
            expected_body_format: "markdown".to_string(),
        });

        save_generated_summary(db.pool(), request).await.unwrap();

        let row = sqlx::query(
            "SELECT body, body_format
             FROM session_documents WHERE id = 'note-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(
            row.try_get::<String, _>("body").unwrap(),
            r#"{"type":"doc","content":[]}"#
        );
        assert_eq!(
            row.try_get::<String, _>("body_format").unwrap(),
            "prosemirror_json"
        );
        let pending_count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM app_settings
             WHERE id = 'auto_enhance_pending:session-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(pending_count, 0);
    }

    #[tokio::test]
    async fn stale_current_body_rolls_back_without_inserting_tags() {
        let db = test_db().await;
        let mut request = request(vec!["launch"]);
        request.current_body = "stale body".to_string();

        let error = save_generated_summary(db.pool(), request)
            .await
            .unwrap_err();
        assert_eq!(error, "transaction statement 0 affected 0 rows; expected 1");

        let body: String =
            sqlx::query_scalar("SELECT body FROM session_documents WHERE id = 'note-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let tag_count: i64 = sqlx::query_scalar("SELECT count(*) FROM tags")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(body, "old body");
        assert_eq!(tag_count, 0);
    }

    #[tokio::test]
    async fn stale_pending_generation_rolls_back_and_keeps_the_setting() {
        let db = test_db().await;
        insert_pending_setting(
            &db,
            json!({
                "noteId": "note-1",
                "generation": "stored-generation",
                "body": "old body",
                "bodyFormat": "markdown"
            }),
        )
        .await;
        let mut request = request(vec!["launch"]);
        request.pending_auto_enhance = Some(PendingAutoEnhanceGuard {
            generation: "requested-generation".to_string(),
            expected_body: "old body".to_string(),
            expected_body_format: "markdown".to_string(),
        });

        let error = save_generated_summary(db.pool(), request)
            .await
            .unwrap_err();
        assert_eq!(error, "transaction statement 0 affected 0 rows; expected 1");

        let body: String =
            sqlx::query_scalar("SELECT body FROM session_documents WHERE id = 'note-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let pending_count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM app_settings
             WHERE id = 'auto_enhance_pending:session-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        let tag_count: i64 = sqlx::query_scalar("SELECT count(*) FROM tags")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(body, "old body");
        assert_eq!(pending_count, 1);
        assert_eq!(tag_count, 0);
    }

    #[tokio::test]
    async fn soft_deleted_session_rejects_the_summary_save() {
        let db = test_db().await;
        sqlx::query("UPDATE sessions SET deleted_at = 'deleted' WHERE id = 'session-1'")
            .execute(db.pool())
            .await
            .unwrap();

        let error = save_generated_summary(db.pool(), request(vec!["launch"]))
            .await
            .unwrap_err();
        assert_eq!(error, "transaction statement 0 affected 0 rows; expected 1");

        let body: String =
            sqlx::query_scalar("SELECT body FROM session_documents WHERE id = 'note-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let tag_count: i64 = sqlx::query_scalar("SELECT count(*) FROM tags")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(body, "old body");
        assert_eq!(tag_count, 0);
    }
}
