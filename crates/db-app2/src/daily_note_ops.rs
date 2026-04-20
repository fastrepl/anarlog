use sqlx::SqlitePool;

use crate::{DailyNote, UpsertDailyNote};

pub async fn get_daily_note(
    pool: &SqlitePool,
    date: &str,
) -> Result<Option<DailyNote>, sqlx::Error> {
    sqlx::query_as::<_, DailyNote>("SELECT * FROM daily_notes WHERE date = ?")
        .bind(date)
        .fetch_optional(pool)
        .await
}

pub async fn upsert_daily_note(
    pool: &SqlitePool,
    input: UpsertDailyNote<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO daily_notes \
         (date, content, updated_at) \
         VALUES (?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
         ON CONFLICT(date) DO UPDATE SET \
           content = excluded.content, \
           updated_at = excluded.updated_at",
    )
    .bind(input.date)
    .bind(input.content)
    .execute(pool)
    .await?;

    Ok(())
}
