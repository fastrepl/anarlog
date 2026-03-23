use sqlx::SqlitePool;

pub async fn set_meeting_visibility(
    pool: &SqlitePool,
    meeting_id: &str,
    visibility: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE meetings SET visibility = ? WHERE id = ?")
        .bind(visibility)
        .bind(meeting_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE words SET visibility = ? WHERE meeting_id = ?")
        .bind(visibility)
        .bind(meeting_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE speaker_hints SET visibility = ? WHERE meeting_id = ?")
        .bind(visibility)
        .bind(meeting_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE notes SET visibility = ? WHERE meeting_id = ?")
        .bind(visibility)
        .bind(meeting_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
