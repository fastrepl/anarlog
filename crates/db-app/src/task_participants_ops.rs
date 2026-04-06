use sqlx::{Row, SqlitePool};

use crate::TaskParticipantRow;

pub async fn add_task_participant(
    pool: &SqlitePool,
    task_id: &str,
    human_id: &str,
    source: &str,
) -> Result<(), sqlx::Error> {
    upsert_task_participant(pool, task_id, human_id, source, "", "public").await
}

pub async fn upsert_task_participant(
    pool: &SqlitePool,
    task_id: &str,
    human_id: &str,
    source: &str,
    user_id: &str,
    visibility: &str,
) -> Result<(), sqlx::Error> {
    let id = format!("{task_id}:{human_id}");
    sqlx::query(
        "INSERT OR REPLACE INTO task_participants (id, task_id, human_id, source, user_id, visibility) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(task_id)
    .bind(human_id)
    .bind(source)
    .bind(user_id)
    .bind(visibility)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn exclude_task_participant(
    pool: &SqlitePool,
    task_id: &str,
    human_id: &str,
    user_id: &str,
) -> Result<(), sqlx::Error> {
    upsert_task_participant(pool, task_id, human_id, "excluded", user_id, "public").await
}

pub async fn remove_task_participant(
    pool: &SqlitePool,
    task_id: &str,
    human_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM task_participants WHERE task_id = ? AND human_id = ?")
        .bind(task_id)
        .bind(human_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_task_participants(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<Vec<TaskParticipantRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT id, task_id, human_id, source, user_id, visibility FROM task_participants WHERE task_id = ?",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, task_id, human_id, source, user_id, visibility)| TaskParticipantRow {
                id,
                task_id,
                human_id,
                source,
                user_id,
                visibility,
            },
        )
        .collect())
}

pub async fn sync_task_participants_from_event(
    pool: &SqlitePool,
    task_id: &str,
    event_id: &str,
) -> Result<usize, sqlx::Error> {
    let participant_rows = sqlx::query(
        "SELECT human_id, user_id FROM event_participants WHERE event_id = ? AND human_id IS NOT NULL",
    )
    .bind(event_id)
    .fetch_all(pool)
    .await?;

    let mut synced = 0;
    for row in participant_rows {
        let human_id: String = row.get("human_id");
        let user_id: String = row.get("user_id");
        let existing =
            sqlx::query("SELECT source FROM task_participants WHERE task_id = ? AND human_id = ?")
                .bind(task_id)
                .bind(&human_id)
                .fetch_optional(pool)
                .await?;

        if let Some(existing) = existing {
            let source: String = existing.get("source");
            if source == "excluded" {
                continue;
            }
        }

        upsert_task_participant(pool, task_id, &human_id, "event", &user_id, "public").await?;
        synced += 1;
    }

    Ok(synced)
}

pub async fn list_tasks_by_human(
    pool: &SqlitePool,
    human_id: &str,
) -> Result<Vec<TaskParticipantRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT id, task_id, human_id, source, user_id, visibility FROM task_participants WHERE human_id = ?",
    )
    .bind(human_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, task_id, human_id, source, user_id, visibility)| TaskParticipantRow {
                id,
                task_id,
                human_id,
                source,
                user_id,
                visibility,
            },
        )
        .collect())
}
