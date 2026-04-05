use sqlx::SqlitePool;

use crate::TimelineRow;

pub async fn list_timeline_by_human(
    pool: &SqlitePool,
    human_id: &str,
) -> Result<Vec<TimelineRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT human_id, source_type, source_id, happened_at, title FROM timeline WHERE human_id = ? ORDER BY happened_at DESC",
    )
    .bind(human_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(human_id, source_type, source_id, happened_at, title)| TimelineRow {
                human_id,
                source_type,
                source_id,
                happened_at,
                title,
            },
        )
        .collect())
}
