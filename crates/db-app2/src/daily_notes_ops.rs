use sqlx::{Row, SqlitePool};

use crate::DailyNoteRow;

pub async fn get_daily_note(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<DailyNoteRow>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, date, content, user_id, created_at, updated_at FROM daily_notes WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.as_ref().map(map_daily_note_row))
}

pub async fn get_daily_note_by_date(
    pool: &SqlitePool,
    date: &str,
    user_id: &str,
) -> Result<Option<DailyNoteRow>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, date, content, user_id, created_at, updated_at \
         FROM daily_notes \
         WHERE date = ? AND user_id = ?",
    )
    .bind(date)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.as_ref().map(map_daily_note_row))
}

pub async fn upsert_daily_note(
    pool: &SqlitePool,
    id: &str,
    date: &str,
    content: &str,
    user_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO daily_notes (id, date, content, user_id, updated_at) \
         VALUES (?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')) \
         ON CONFLICT(id) DO UPDATE SET \
           date = excluded.date, \
           content = excluded.content, \
           user_id = excluded.user_id, \
           updated_at = excluded.updated_at",
    )
    .bind(id)
    .bind(date)
    .bind(content)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_or_create_daily_note(
    pool: &SqlitePool,
    id: &str,
    date: &str,
    user_id: &str,
) -> Result<DailyNoteRow, sqlx::Error> {
    sqlx::query("INSERT OR IGNORE INTO daily_notes (id, date, user_id) VALUES (?, ?, ?)")
        .bind(id)
        .bind(date)
        .bind(user_id)
        .execute(pool)
        .await?;

    if let Some(note) = get_daily_note_by_date(pool, date, user_id).await? {
        return Ok(note);
    }

    Err(sqlx::Error::RowNotFound)
}

pub async fn update_daily_note_content(
    pool: &SqlitePool,
    id: &str,
    content: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE daily_notes \
         SET content = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
         WHERE id = ?",
    )
    .bind(content)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_daily_notes_in_range(
    pool: &SqlitePool,
    start_date: &str,
    end_date: &str,
    user_id: &str,
) -> Result<Vec<DailyNoteRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, date, content, user_id, created_at, updated_at \
         FROM daily_notes \
         WHERE date >= ? AND date <= ? AND user_id = ? \
         ORDER BY date",
    )
    .bind(start_date)
    .bind(end_date)
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(map_daily_note_row).collect())
}

fn map_daily_note_row(row: &sqlx::sqlite::SqliteRow) -> DailyNoteRow {
    DailyNoteRow {
        id: row.get("id"),
        date: row.get("date"),
        content: row.get("content"),
        user_id: row.get("user_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}
