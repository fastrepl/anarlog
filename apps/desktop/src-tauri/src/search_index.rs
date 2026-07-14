use std::sync::Arc;
use std::time::Duration;

use chrono::DateTime;
use serde_json::{Map, Value};
use sqlx::{Row, SqlitePool};
use tauri::AppHandle;
use tauri_plugin_tantivy::{
    SearchDocument, SearchFilters, SearchOptions, SearchRequest, TantivyPluginExt,
};

// Increment when the SQLite-to-Tantivy document shape changes so existing indexes are rebuilt.
const PROJECTION_VERSION: i64 = 1;
const BATCH_SIZE: i64 = 8;
const RETRY_INTERVAL: Duration = Duration::from_secs(5);

type WorkerResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug)]
struct DirtyEntity {
    entity_type: String,
    entity_id: String,
    generation: i64,
}

enum IndexAction {
    Upsert(SearchDocument),
    Remove(String),
    Skip,
}

pub fn spawn(app: AppHandle, db: Arc<hypr_db_core::Db>) {
    tauri::async_runtime::spawn(async move {
        run(app, db).await;
    });
}

async fn run(app: AppHandle, db: Arc<hypr_db_core::Db>) {
    let mut changes = db.change_notifier().subscribe();

    wait_for_tantivy(&app).await;

    loop {
        match initialize(&app, db.pool()).await {
            Ok(()) => break,
            Err(error) => {
                tracing::error!(%error, "failed to initialize search index projection");
                tokio::time::sleep(RETRY_INTERVAL).await;
            }
        }
    }

    loop {
        if let Err(error) = drain_queue(&app, db.pool()).await {
            tracing::error!(%error, "failed to update search index projection");
        }

        tokio::select! {
            change = changes.recv() => {
                match change {
                    Ok(change) if change.table == "search_index_dirty" => {}
                    Ok(_) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
            _ = tokio::time::sleep(RETRY_INTERVAL) => {}
        }
    }
}

async fn wait_for_tantivy(app: &AppHandle) {
    loop {
        match index_document_count(app).await {
            Ok(_) => return,
            Err(tauri_plugin_tantivy::Error::CollectionNotFound(_)) => {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(error) => {
                tracing::warn!(%error, "search index is not ready");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

async fn initialize(app: &AppHandle, pool: &SqlitePool) -> WorkerResult<()> {
    let projection_version: i64 = sqlx::query_scalar(
        "SELECT projection_version FROM search_index_state WHERE id = 'default'",
    )
    .fetch_optional(pool)
    .await?
    .unwrap_or(0);

    if projection_version != PROJECTION_VERSION {
        return rebuild(app, pool).await;
    }

    drain_queue(app, pool).await?;

    let (database_count, pending_count) = projection_consistency_snapshot(pool).await?;
    if pending_count > 0 {
        return Ok(());
    }

    let index_count_matches = wait_for_index_count(app, database_count as usize).await?;
    if !index_count_matches {
        let index_count = index_document_count(app).await?;
        tracing::info!(
            database_count,
            index_count,
            "search index count does not match SQLite; rebuilding projection"
        );
        rebuild(app, pool).await?;
    }

    Ok(())
}

async fn rebuild(app: &AppHandle, pool: &SqlitePool) -> WorkerResult<()> {
    sqlx::query(
        "UPDATE search_index_state
         SET projection_version = 0,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = 'default'",
    )
    .execute(pool)
    .await?;

    app.tantivy().reindex(None).await?;
    enqueue_all_entities(pool).await?;
    drain_queue(app, pool).await?;

    sqlx::query(
        "UPDATE search_index_state
         SET projection_version = ?,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = 'default'",
    )
    .bind(PROJECTION_VERSION)
    .execute(pool)
    .await?;

    tracing::info!("rebuilt search index projection");
    Ok(())
}

async fn enqueue_all_entities(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    sqlx::query(
        "INSERT INTO search_index_dirty (entity_type, entity_id)
         SELECT 'session', id
         FROM sessions
         WHERE deleted_at IS NULL
         ON CONFLICT(entity_type, entity_id) DO UPDATE SET
           generation = search_index_dirty.generation + 1,
           queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO search_index_dirty (entity_type, entity_id)
         SELECT 'human', id
         FROM humans
         WHERE deleted_at IS NULL
         ON CONFLICT(entity_type, entity_id) DO UPDATE SET
           generation = search_index_dirty.generation + 1,
           queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO search_index_dirty (entity_type, entity_id)
         SELECT 'organization', id
         FROM organizations
         WHERE deleted_at IS NULL
         ON CONFLICT(entity_type, entity_id) DO UPDATE SET
           generation = search_index_dirty.generation + 1,
           queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await
}

async fn drain_queue(app: &AppHandle, pool: &SqlitePool) -> WorkerResult<()> {
    loop {
        let rows = sqlx::query(
            "SELECT entity_type, entity_id, generation
             FROM search_index_dirty
             ORDER BY queued_at, entity_type, entity_id
             LIMIT ?",
        )
        .bind(BATCH_SIZE)
        .fetch_all(pool)
        .await?;

        if rows.is_empty() {
            return Ok(());
        }

        let dirty_entities = rows
            .into_iter()
            .map(|row| DirtyEntity {
                entity_type: row.get("entity_type"),
                entity_id: row.get("entity_id"),
                generation: row.get("generation"),
            })
            .collect::<Vec<_>>();

        let mut documents = Vec::new();
        let mut removals = Vec::new();
        for dirty in &dirty_entities {
            match build_index_action(pool, dirty).await? {
                IndexAction::Upsert(document) => documents.push(document),
                IndexAction::Remove(id) => removals.push(id),
                IndexAction::Skip => {}
            }
        }

        if !documents.is_empty() {
            app.tantivy().update_documents(None, documents).await?;
        }
        for id in removals {
            app.tantivy().remove_document(None, id).await?;
        }

        acknowledge_dirty_entities(pool, &dirty_entities).await?;

        tokio::task::yield_now().await;
    }
}

async fn acknowledge_dirty_entities(
    pool: &SqlitePool,
    dirty_entities: &[DirtyEntity],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    for dirty in dirty_entities {
        sqlx::query(
            "DELETE FROM search_index_dirty
             WHERE entity_type = ? AND entity_id = ? AND generation = ?",
        )
        .bind(&dirty.entity_type)
        .bind(&dirty.entity_id)
        .bind(dirty.generation)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

async fn build_index_action(pool: &SqlitePool, dirty: &DirtyEntity) -> WorkerResult<IndexAction> {
    match dirty.entity_type.as_str() {
        "session" => build_session_document(pool, &dirty.entity_id).await,
        "human" => build_human_document(pool, &dirty.entity_id).await,
        "organization" => build_organization_document(pool, &dirty.entity_id).await,
        entity_type => {
            tracing::warn!(entity_type, "ignoring unknown search index entity type");
            Ok(IndexAction::Skip)
        }
    }
}

async fn build_session_document(pool: &SqlitePool, id: &str) -> WorkerResult<IndexAction> {
    let Some(session) = sqlx::query(
        "SELECT title, created_at, event_json
         FROM sessions
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(IndexAction::Remove(id.to_string()));
    };

    let raw_body: Option<String> = sqlx::query_scalar(
        "SELECT body
         FROM session_documents
         WHERE session_id = ? AND kind = 'note' AND deleted_at IS NULL
         ORDER BY CASE WHEN id = ? THEN 0 ELSE 1 END, created_at, id
         LIMIT 1",
    )
    .bind(id)
    .bind(id)
    .fetch_optional(pool)
    .await?;

    let enhanced_bodies: Vec<String> = sqlx::query_scalar(
        "SELECT body
         FROM session_documents
         WHERE session_id = ?
           AND kind IN ('summary', 'template_output')
           AND deleted_at IS NULL
         ORDER BY sort_order, created_at, id",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    let transcripts: Vec<String> = sqlx::query_scalar(
        "SELECT words_json
         FROM transcripts
         WHERE session_id = ? AND deleted_at IS NULL
         ORDER BY started_at_ms, created_at, id",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    let mut content_parts = Vec::with_capacity(1 + enhanced_bodies.len() + transcripts.len());
    if let Some(raw_body) = raw_body {
        content_parts.push(extract_plain_text(&raw_body));
    }
    content_parts.extend(enhanced_bodies.iter().map(|body| extract_plain_text(body)));
    content_parts.extend(
        transcripts
            .iter()
            .map(|transcript| flatten_transcript(transcript)),
    );

    let title: String = session.get("title");
    let created_at: String = session.get("created_at");
    let event_json: String = session.get("event_json");

    Ok(IndexAction::Upsert(SearchDocument {
        id: id.to_string(),
        doc_type: "session".to_string(),
        language: None,
        title: fallback_title(&title, "Untitled"),
        content: merge_content(content_parts.iter().map(String::as_str)),
        created_at: session_search_timestamp(&event_json, &created_at),
        facets: Vec::new(),
    }))
}

async fn build_human_document(pool: &SqlitePool, id: &str) -> WorkerResult<IndexAction> {
    let Some(human) = sqlx::query(
        "SELECT name, email, job_title, linkedin_username, created_at, memo
         FROM humans
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(IndexAction::Remove(id.to_string()));
    };

    let name: String = human.get("name");
    let email: String = human.get("email");
    let job_title: String = human.get("job_title");
    let linkedin_username: String = human.get("linkedin_username");
    let created_at: String = human.get("created_at");
    let memo: String = human.get("memo");

    Ok(IndexAction::Upsert(SearchDocument {
        id: id.to_string(),
        doc_type: "human".to_string(),
        language: None,
        title: fallback_title(&name, "Unknown"),
        content: merge_content(
            [email, job_title, linkedin_username, memo]
                .iter()
                .map(String::as_str),
        ),
        created_at: to_epoch_ms(&Value::String(created_at)),
        facets: Vec::new(),
    }))
}

async fn build_organization_document(pool: &SqlitePool, id: &str) -> WorkerResult<IndexAction> {
    let Some(organization) = sqlx::query(
        "SELECT name, created_at
         FROM organizations
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(IndexAction::Remove(id.to_string()));
    };

    let name: String = organization.get("name");
    let created_at: String = organization.get("created_at");

    Ok(IndexAction::Upsert(SearchDocument {
        id: id.to_string(),
        doc_type: "organization".to_string(),
        language: None,
        title: fallback_title(&name, "Unknown Organization"),
        content: String::new(),
        created_at: to_epoch_ms(&Value::String(created_at)),
        facets: Vec::new(),
    }))
}

async fn projection_consistency_snapshot(pool: &SqlitePool) -> Result<(i64, i64), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let active_count = sqlx::query_scalar(
        "SELECT
           (SELECT COUNT(*) FROM sessions WHERE deleted_at IS NULL) +
           (SELECT COUNT(*) FROM humans WHERE deleted_at IS NULL) +
           (SELECT COUNT(*) FROM organizations WHERE deleted_at IS NULL)",
    )
    .fetch_one(&mut *tx)
    .await?;
    let pending_count = sqlx::query_scalar("SELECT COUNT(*) FROM search_index_dirty")
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok((active_count, pending_count))
}

async fn index_document_count(app: &AppHandle) -> Result<usize, tauri_plugin_tantivy::Error> {
    let result = app
        .tantivy()
        .search(SearchRequest {
            query: String::new(),
            collection: None,
            filters: SearchFilters::default(),
            limit: 1,
            options: SearchOptions::default(),
        })
        .await?;
    Ok(result.count)
}

async fn wait_for_index_count(
    app: &AppHandle,
    expected: usize,
) -> Result<bool, tauri_plugin_tantivy::Error> {
    for _ in 0..40 {
        if index_document_count(app).await? == expected {
            return Ok(true);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    Ok(false)
}

fn fallback_title(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

fn merge_content<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    parts
        .into_iter()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn extract_plain_text(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() || !trimmed.starts_with('{') {
        return trimmed.to_string();
    }

    let Ok(parsed) = serde_json::from_str::<Value>(trimmed) else {
        return trimmed.to_string();
    };
    let Some(object) = parsed.as_object() else {
        return trimmed.to_string();
    };
    if object.get("type").and_then(Value::as_str) != Some("doc")
        || !object.get("content").is_some_and(Value::is_array)
    {
        return trimmed.to_string();
    }

    normalize_whitespace(&extract_tiptap_text(&parsed))
}

fn extract_tiptap_text(node: &Value) -> String {
    if let Some(text) = node.get("text").and_then(Value::as_str)
        && !text.is_empty()
    {
        return text.to_string();
    }

    node.get("content")
        .and_then(Value::as_array)
        .map(|children| {
            children
                .iter()
                .map(extract_tiptap_text)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default()
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn flatten_transcript(value: &str) -> String {
    let parsed =
        serde_json::from_str::<Value>(value).unwrap_or_else(|_| Value::String(value.to_string()));
    flatten_transcript_value(&parsed)
}

fn flatten_transcript_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Array(segments) => {
            let parts = segments
                .iter()
                .filter_map(|segment| match segment {
                    Value::String(value) => Some(value.clone()),
                    Value::Object(record) => Some(flatten_transcript_record(record)),
                    Value::Array(_) => Some(flatten_transcript_value(segment)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            merge_content(parts.iter().map(String::as_str))
        }
        Value::Object(record) => flatten_transcript_record_values(record),
        _ => String::new(),
    }
}

fn flatten_transcript_record(record: &Map<String, Value>) -> String {
    let preferred = record
        .get("text")
        .filter(|value| !value.is_null())
        .or_else(|| record.get("content"));
    if let Some(value) = preferred.and_then(Value::as_str) {
        return value.to_string();
    }

    flatten_transcript_record_values(record)
}

fn flatten_transcript_record_values(record: &Map<String, Value>) -> String {
    let parts = record
        .values()
        .map(flatten_nested_transcript_value)
        .collect::<Vec<_>>();
    merge_content(parts.iter().map(String::as_str))
}

fn flatten_nested_transcript_value(value: &Value) -> String {
    if let Value::String(value) = value {
        let parsed =
            serde_json::from_str::<Value>(value).unwrap_or_else(|_| Value::String(value.clone()));
        return flatten_transcript_value(&parsed);
    }

    flatten_transcript_value(value)
}

fn session_search_timestamp(event_json: &str, created_at: &str) -> i64 {
    if let Ok(event) = serde_json::from_str::<Value>(event_json)
        && let Some(started_at) = event.get("started_at")
    {
        let timestamp = to_epoch_ms(started_at);
        if timestamp > 0 {
            return timestamp;
        }
    }

    to_epoch_ms(&Value::String(created_at.to_string()))
}

fn to_epoch_ms(value: &Value) -> i64 {
    match value {
        Value::Number(value) => value.as_f64().unwrap_or(0.0) as i64,
        Value::String(value) => DateTime::parse_from_rfc3339(value)
            .map(|date| date.timestamp_millis())
            .ok()
            .or_else(|| value.parse::<f64>().ok().map(|value| value as i64))
            .unwrap_or(0),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn acknowledgement_does_not_drop_a_concurrent_change() {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        sqlx::query("INSERT INTO sessions (id, title) VALUES ('session-1', 'Planning')")
            .execute(db.pool())
            .await
            .unwrap();

        let queued_generation: i64 = sqlx::query_scalar(
            "SELECT generation FROM search_index_dirty
             WHERE entity_type = 'session' AND entity_id = 'session-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        sqlx::query("UPDATE sessions SET title = 'Updated' WHERE id = 'session-1'")
            .execute(db.pool())
            .await
            .unwrap();

        acknowledge_dirty_entities(
            db.pool(),
            &[DirtyEntity {
                entity_type: "session".to_string(),
                entity_id: "session-1".to_string(),
                generation: queued_generation,
            }],
        )
        .await
        .unwrap();

        let current_generation: i64 = sqlx::query_scalar(
            "SELECT generation FROM search_index_dirty
             WHERE entity_type = 'session' AND entity_id = 'session-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(current_generation, queued_generation + 1);

        acknowledge_dirty_entities(
            db.pool(),
            &[DirtyEntity {
                entity_type: "session".to_string(),
                entity_id: "session-1".to_string(),
                generation: current_generation,
            }],
        )
        .await
        .unwrap();
        let remaining: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM search_index_dirty
             WHERE entity_type = 'session' AND entity_id = 'session-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(remaining, 0);
    }

    #[test]
    fn extracts_text_only_from_valid_tiptap_documents() {
        assert_eq!(
            extract_plain_text(
                r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"first"},{"type":"text","text":"second"}]}]}"#,
            ),
            "first second"
        );
        assert_eq!(
            extract_plain_text(r#"{"type":"paragraph","text":"unchanged"}"#),
            r#"{"type":"paragraph","text":"unchanged"}"#
        );
        assert_eq!(extract_plain_text("  plain note  "), "plain note");
    }

    #[test]
    fn flattens_transcript_segments_with_text_and_content_preference() {
        assert_eq!(
            flatten_transcript(
                r#"[{"text":"hello","ignored":"x"},{"content":"world"},{"nested":{"text":"again"}},["nested","array"]]"#,
            ),
            "hello world again nested array"
        );
    }

    #[test]
    fn session_timestamp_prefers_event_start_and_falls_back_to_created_at() {
        assert_eq!(
            session_search_timestamp(
                r#"{"started_at":"2026-07-14T01:02:03Z"}"#,
                "2025-01-01T00:00:00Z",
            ),
            1_783_990_923_000
        );
        assert_eq!(
            session_search_timestamp("{}", "2025-01-01T00:00:00Z"),
            1_735_689_600_000
        );
        assert_eq!(session_search_timestamp(r#"{"started_at":1234}"#, ""), 1234);
    }
}
