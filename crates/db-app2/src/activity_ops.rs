use sqlx::{Row, SqlitePool};

use crate::{ActivitySegmentRow, ActivitySignalRow};

const ACTIVITY_SIGNAL_SELECT: &str = "SELECT id, occurred_at_ms, transition_sequence, reason, app_id, bundle_id, app_name, activity_kind, window_title, url, domain, content_level, source, text_anchor_identity, fingerprint, payload_json, created_at FROM activity_signals";
const ACTIVITY_SEGMENT_SELECT: &str = "SELECT id, started_at_ms, ended_at_ms, duration_ms, date, semantic_key, app_id, bundle_id, app_name, activity_kind, title, url, domain, payload_json, source_signal_start_id, source_signal_end_id, linked_task_id, linked_daily_note_id, created_at, updated_at FROM activity_segments";

pub struct InsertActivitySignal<'a> {
    pub id: &'a str,
    pub occurred_at_ms: i64,
    pub transition_sequence: i64,
    pub reason: &'a str,
    pub app_id: &'a str,
    pub bundle_id: &'a str,
    pub app_name: &'a str,
    pub activity_kind: &'a str,
    pub window_title: &'a str,
    pub url: &'a str,
    pub domain: &'a str,
    pub content_level: &'a str,
    pub source: &'a str,
    pub text_anchor_identity: &'a str,
    pub fingerprint: &'a str,
    pub payload_json: &'a str,
}

pub struct UpsertActivitySegment<'a> {
    pub id: &'a str,
    pub started_at_ms: i64,
    pub ended_at_ms: i64,
    pub duration_ms: i64,
    pub date: &'a str,
    pub semantic_key: &'a str,
    pub app_id: &'a str,
    pub bundle_id: &'a str,
    pub app_name: &'a str,
    pub activity_kind: &'a str,
    pub title: &'a str,
    pub url: &'a str,
    pub domain: &'a str,
    pub payload_json: &'a str,
    pub source_signal_start_id: Option<&'a str>,
    pub source_signal_end_id: Option<&'a str>,
    pub linked_task_id: Option<&'a str>,
    pub linked_daily_note_id: Option<&'a str>,
}

pub async fn insert_activity_signal(
    pool: &SqlitePool,
    input: InsertActivitySignal<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO activity_signals ( \
            id, occurred_at_ms, transition_sequence, reason, app_id, bundle_id, app_name, activity_kind, \
            window_title, url, domain, content_level, source, text_anchor_identity, fingerprint, payload_json \
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(input.id)
    .bind(input.occurred_at_ms)
    .bind(input.transition_sequence)
    .bind(input.reason)
    .bind(input.app_id)
    .bind(input.bundle_id)
    .bind(input.app_name)
    .bind(input.activity_kind)
    .bind(input.window_title)
    .bind(input.url)
    .bind(input.domain)
    .bind(input.content_level)
    .bind(input.source)
    .bind(input.text_anchor_identity)
    .bind(input.fingerprint)
    .bind(input.payload_json)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_activity_signals_since(
    pool: &SqlitePool,
    occurred_at_ms: i64,
) -> Result<Vec<ActivitySignalRow>, sqlx::Error> {
    let rows = sqlx::query(&format!(
        "{ACTIVITY_SIGNAL_SELECT} WHERE occurred_at_ms >= ? ORDER BY occurred_at_ms, transition_sequence, id"
    ))
    .bind(occurred_at_ms)
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(map_activity_signal_row).collect())
}

pub async fn list_activity_signals_in_range(
    pool: &SqlitePool,
    start_ms: i64,
    end_ms: i64,
) -> Result<Vec<ActivitySignalRow>, sqlx::Error> {
    let rows = sqlx::query(&format!(
        "{ACTIVITY_SIGNAL_SELECT} WHERE occurred_at_ms >= ? AND occurred_at_ms < ? ORDER BY occurred_at_ms, transition_sequence, id"
    ))
    .bind(start_ms)
    .bind(end_ms)
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(map_activity_signal_row).collect())
}

pub async fn upsert_activity_segment(
    pool: &SqlitePool,
    input: UpsertActivitySegment<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO activity_segments ( \
            id, started_at_ms, ended_at_ms, duration_ms, date, semantic_key, app_id, bundle_id, \
            app_name, activity_kind, title, url, domain, payload_json, source_signal_start_id, \
            source_signal_end_id, linked_task_id, linked_daily_note_id, updated_at \
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')) \
         ON CONFLICT(id) DO UPDATE SET \
            started_at_ms = excluded.started_at_ms, \
            ended_at_ms = excluded.ended_at_ms, \
            duration_ms = excluded.duration_ms, \
            date = excluded.date, \
            semantic_key = excluded.semantic_key, \
            app_id = excluded.app_id, \
            bundle_id = excluded.bundle_id, \
            app_name = excluded.app_name, \
            activity_kind = excluded.activity_kind, \
            title = excluded.title, \
            url = excluded.url, \
            domain = excluded.domain, \
            payload_json = excluded.payload_json, \
            source_signal_start_id = excluded.source_signal_start_id, \
            source_signal_end_id = excluded.source_signal_end_id, \
            linked_task_id = excluded.linked_task_id, \
            linked_daily_note_id = excluded.linked_daily_note_id, \
            updated_at = excluded.updated_at",
    )
    .bind(input.id)
    .bind(input.started_at_ms)
    .bind(input.ended_at_ms)
    .bind(input.duration_ms)
    .bind(input.date)
    .bind(input.semantic_key)
    .bind(input.app_id)
    .bind(input.bundle_id)
    .bind(input.app_name)
    .bind(input.activity_kind)
    .bind(input.title)
    .bind(input.url)
    .bind(input.domain)
    .bind(input.payload_json)
    .bind(input.source_signal_start_id)
    .bind(input.source_signal_end_id)
    .bind(input.linked_task_id)
    .bind(input.linked_daily_note_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_activity_segment(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<ActivitySegmentRow>, sqlx::Error> {
    let row = sqlx::query(&format!("{ACTIVITY_SEGMENT_SELECT} WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?;

    Ok(row.as_ref().map(map_activity_segment_row))
}

pub async fn list_activity_segments_for_date(
    pool: &SqlitePool,
    date: &str,
) -> Result<Vec<ActivitySegmentRow>, sqlx::Error> {
    let rows = sqlx::query(&format!(
        "{ACTIVITY_SEGMENT_SELECT} WHERE date = ? ORDER BY started_at_ms, id"
    ))
    .bind(date)
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(map_activity_segment_row).collect())
}

pub async fn list_activity_segments_in_range(
    pool: &SqlitePool,
    start_ms: i64,
    end_ms: i64,
) -> Result<Vec<ActivitySegmentRow>, sqlx::Error> {
    let rows = sqlx::query(&format!(
        "{ACTIVITY_SEGMENT_SELECT} WHERE started_at_ms < ? AND ended_at_ms >= ? ORDER BY started_at_ms, id"
    ))
    .bind(end_ms)
    .bind(start_ms)
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(map_activity_segment_row).collect())
}

pub async fn link_activity_segment_to_task(
    pool: &SqlitePool,
    id: &str,
    task_id: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE activity_segments \
         SET linked_task_id = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
         WHERE id = ?",
    )
    .bind(task_id)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn link_activity_segment_to_daily_note(
    pool: &SqlitePool,
    id: &str,
    daily_note_id: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE activity_segments \
         SET linked_daily_note_id = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
         WHERE id = ?",
    )
    .bind(daily_note_id)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

fn map_activity_signal_row(row: &sqlx::sqlite::SqliteRow) -> ActivitySignalRow {
    ActivitySignalRow {
        id: row.get("id"),
        occurred_at_ms: row.get("occurred_at_ms"),
        transition_sequence: row.get("transition_sequence"),
        reason: row.get("reason"),
        app_id: row.get("app_id"),
        bundle_id: row.get("bundle_id"),
        app_name: row.get("app_name"),
        activity_kind: row.get("activity_kind"),
        window_title: row.get("window_title"),
        url: row.get("url"),
        domain: row.get("domain"),
        content_level: row.get("content_level"),
        source: row.get("source"),
        text_anchor_identity: row.get("text_anchor_identity"),
        fingerprint: row.get("fingerprint"),
        payload_json: row.get("payload_json"),
        created_at: row.get("created_at"),
    }
}

fn map_activity_segment_row(row: &sqlx::sqlite::SqliteRow) -> ActivitySegmentRow {
    ActivitySegmentRow {
        id: row.get("id"),
        started_at_ms: row.get("started_at_ms"),
        ended_at_ms: row.get("ended_at_ms"),
        duration_ms: row.get("duration_ms"),
        date: row.get("date"),
        semantic_key: row.get("semantic_key"),
        app_id: row.get("app_id"),
        bundle_id: row.get("bundle_id"),
        app_name: row.get("app_name"),
        activity_kind: row.get("activity_kind"),
        title: row.get("title"),
        url: row.get("url"),
        domain: row.get("domain"),
        payload_json: row.get("payload_json"),
        source_signal_start_id: row.get("source_signal_start_id"),
        source_signal_end_id: row.get("source_signal_end_id"),
        linked_task_id: row.get("linked_task_id"),
        linked_daily_note_id: row.get("linked_daily_note_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}
