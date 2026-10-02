use sqlx::{QueryBuilder, Row, Sqlite, SqliteConnection};

use crate::transcript_batch_ops::load_pending_delta_jsons;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRenderTranscriptRow {
    pub id: String,
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,
    pub words_json: String,
    pub speaker_hints_json: String,
    pub pending_delta_jsons: Vec<String>,
}

pub async fn load_session_render_transcripts(
    conn: &mut SqliteConnection,
    session_id: &str,
) -> Result<Vec<SessionRenderTranscriptRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, started_at_ms, ended_at_ms, words_json, speaker_hints_json
         FROM transcripts
         WHERE session_id = ? AND deleted_at IS NULL
         ORDER BY started_at_ms, id",
    )
    .bind(session_id)
    .fetch_all(&mut *conn)
    .await?;

    let mut transcripts = Vec::with_capacity(rows.len());
    for row in rows {
        let id: String = row.try_get("id")?;
        let pending_delta_jsons = load_pending_delta_jsons(conn, &id).await?;
        transcripts.push(SessionRenderTranscriptRow {
            id,
            started_at_ms: row.try_get("started_at_ms")?,
            ended_at_ms: row.try_get("ended_at_ms")?,
            words_json: row.try_get("words_json")?,
            speaker_hints_json: row.try_get("speaker_hints_json")?,
            pending_delta_jsons,
        });
    }
    Ok(transcripts)
}

pub async fn load_session_render_participant_ids(
    conn: &mut SqliteConnection,
    session_id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT participant.human_id
         FROM session_participants AS participant
         JOIN sessions AS session ON session.id = participant.session_id
         LEFT JOIN humans AS human
           ON human.id = participant.human_id
          AND human.deleted_at IS NULL
         WHERE session.id = ?
           AND session.deleted_at IS NULL
           AND participant.human_id <> ''
           AND participant.source <> 'excluded'
           AND participant.deleted_at IS NULL
           AND (
             participant.human_id = session.owner_user_id
             OR NULLIF(lower(COALESCE(NULLIF(human.email, ''), participant.email)), '') IS NULL
             OR NOT EXISTS (
               SELECT 1
               FROM humans AS self_human
               WHERE self_human.id = session.owner_user_id
                 AND self_human.deleted_at IS NULL
                 AND NULLIF(lower(self_human.email), '') IS NOT NULL
                 AND lower(self_human.email) =
                     lower(COALESCE(NULLIF(human.email, ''), participant.email))
             )
           )",
    )
    .bind(session_id)
    .fetch_all(&mut *conn)
    .await
}

pub async fn load_render_humans(
    conn: &mut SqliteConnection,
    human_ids: &[String],
) -> Result<Vec<(String, String)>, sqlx::Error> {
    if human_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut query = QueryBuilder::<Sqlite>::new(
        "SELECT id, name FROM humans
         WHERE name <> '' AND id IN (",
    );
    let mut separated = query.separated(", ");
    for human_id in human_ids {
        separated.push_bind(human_id);
    }
    separated.push_unseparated(") ORDER BY id");
    query
        .build_query_as::<(String, String)>()
        .fetch_all(&mut *conn)
        .await
}

#[cfg(test)]
mod tests {
    use anlg_db_core::Db;

    use super::*;
    use crate::prepare_schema;

    async fn test_db() -> Db {
        let db = Db::connect_memory_plain().await.unwrap();
        prepare_schema(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id)
             VALUES ('session-1', 'workspace-1', 'owner-human')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO humans (id, name, email)
             VALUES
                ('owner-human', 'Owner', 'owner@example.com'),
                ('participant-human', 'Participant', 'participant@example.com'),
                ('duplicate-self', 'Duplicate self', 'OWNER@example.com'),
                ('excluded-human', 'Excluded', 'excluded@example.com')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO session_participants
                (id, session_id, human_id, display_name, email, source, created_at)
             VALUES
                ('participant', 'session-1', 'participant-human', '', '', 'manual', '2026-01-01'),
                ('duplicate', 'session-1', 'duplicate-self', '', '', 'manual', '2026-01-01'),
                ('excluded', 'session-1', 'excluded-human', '', '', 'excluded', '2026-01-01')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db
    }

    #[tokio::test]
    async fn session_render_participants_exclude_source_and_self_email_duplicates() {
        let db = test_db().await;
        let mut conn = db.pool().acquire().await.unwrap();

        let participant_ids = load_session_render_participant_ids(&mut conn, "session-1")
            .await
            .unwrap();

        assert_eq!(participant_ids, vec!["participant-human"]);
    }

    #[tokio::test]
    async fn historical_transcript_names_survive_contact_deletion() {
        let db = test_db().await;
        sqlx::query("UPDATE humans SET deleted_at = '2026-10-01' WHERE id = 'participant-human'")
            .execute(db.pool())
            .await
            .unwrap();
        let mut conn = db.pool().acquire().await.unwrap();
        assert_eq!(
            load_render_humans(&mut conn, &["participant-human".to_owned()])
                .await
                .unwrap(),
            vec![("participant-human".to_owned(), "Participant".to_owned())]
        );
    }
}
