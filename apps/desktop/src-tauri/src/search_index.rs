use std::sync::Arc;
use std::time::Duration;

use chrono::DateTime;
use serde_json::{Map, Value};
use sqlx::{Connection, Row, SqlitePool};
use tauri::AppHandle;
use tauri_plugin_tantivy::{
    SearchDocument, SearchFilters, SearchOptions, SearchRequest, TantivyPluginExt,
};

// Increment when the SQLite-to-Tantivy document shape changes so existing indexes are rebuilt.
const PROJECTION_VERSION: i64 = 4;
const BATCH_SIZE: i64 = 64;
const DIRTY_DEBOUNCE: Duration = Duration::from_millis(250);
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
enum DrainOutcome {
    Complete,
    Deferred,
}

pub fn spawn(app: AppHandle, db: Arc<anlg_db_core::Db>) {
    tauri::async_runtime::spawn(async move {
        run(app, db).await;
    });
}

async fn run(app: AppHandle, db: Arc<anlg_db_core::Db>) {
    let mut changes = db.change_notifier().subscribe();

    wait_for_tantivy(&app).await;

    loop {
        match initialize(&app, db.pool()).await {
            Ok(DrainOutcome::Complete) => break,
            Ok(DrainOutcome::Deferred) => {
                tokio::time::sleep(RETRY_INTERVAL).await;
            }
            Err(error) => {
                tracing::error!(%error, "failed to initialize search index projection");
                tokio::time::sleep(RETRY_INTERVAL).await;
            }
        }
    }

    loop {
        match drain_queue(&app, db.pool()).await {
            Ok(DrainOutcome::Complete | DrainOutcome::Deferred) => {}
            Err(error) => {
                tracing::error!(%error, "failed to update search index projection");
            }
        }

        tokio::select! {
            change = changes.recv() => {
                match change {
                    Ok(change) if change.table == "search_index_dirty" => {
                        tokio::time::sleep(DIRTY_DEBOUNCE).await;
                        loop {
                            match changes.try_recv() {
                                Ok(_) | Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {}
                                Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
                                Err(tokio::sync::broadcast::error::TryRecvError::Closed) => break,
                            }
                        }
                    }
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

async fn initialize(app: &AppHandle, pool: &SqlitePool) -> WorkerResult<DrainOutcome> {
    let projection_version: i64 = sqlx::query_scalar(
        "SELECT projection_version FROM search_index_state WHERE id = 'default'",
    )
    .fetch_optional(pool)
    .await?
    .unwrap_or(0);

    if projection_version != PROJECTION_VERSION {
        rebuild(app, pool).await?;
        return Ok(DrainOutcome::Complete);
    }

    if drain_queue(app, pool).await? == DrainOutcome::Deferred {
        return Ok(DrainOutcome::Deferred);
    }

    let (database_count, pending_count) = projection_consistency_snapshot(pool).await?;
    if pending_count > 0 {
        return Ok(DrainOutcome::Deferred);
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

    Ok(DrainOutcome::Complete)
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
    while drain_queue(app, pool).await? == DrainOutcome::Deferred {
        tokio::time::sleep(RETRY_INTERVAL).await;
    }

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

async fn drain_queue(app: &AppHandle, pool: &SqlitePool) -> WorkerResult<DrainOutcome> {
    loop {
        let Some(mut connection) = pool.try_acquire() else {
            return Ok(DrainOutcome::Deferred);
        };
        let rows = sqlx::query(
            "SELECT entity_type, entity_id, generation
             FROM search_index_dirty
             WHERE generation > acknowledged_generation
             ORDER BY queued_at, entity_type, entity_id
             LIMIT ?",
        )
        .bind(BATCH_SIZE)
        .fetch_all(&mut *connection)
        .await?;
        connection.return_to_pool().await;

        if rows.is_empty() {
            return Ok(DrainOutcome::Complete);
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
        let mut processed_entities = Vec::new();
        for dirty in dirty_entities {
            let Some(mut connection) = pool.try_acquire() else {
                break;
            };
            let action = build_index_action(&mut connection, &dirty).await?;
            connection.return_to_pool().await;
            match action {
                IndexAction::Upsert(document) => documents.push(document),
                IndexAction::Remove(id) => removals.push(id),
                IndexAction::Skip => {}
            }
            processed_entities.push(dirty);
        }

        if processed_entities.is_empty() {
            return Ok(DrainOutcome::Deferred);
        }

        if !documents.is_empty() || !removals.is_empty() {
            app.tantivy()
                .apply_document_batch(None, documents, removals)
                .await?;
        }

        if !try_acknowledge_dirty_entities(pool, &processed_entities).await? {
            return Ok(DrainOutcome::Deferred);
        }

        tokio::task::yield_now().await;
    }
}

async fn try_acknowledge_dirty_entities(
    pool: &SqlitePool,
    dirty_entities: &[DirtyEntity],
) -> Result<bool, sqlx::Error> {
    let Some(mut connection) = pool.try_acquire() else {
        return Ok(false);
    };
    let mut tx = connection.begin().await?;
    for dirty in dirty_entities {
        sqlx::query(
            "UPDATE search_index_dirty
             SET acknowledged_generation = generation
             WHERE entity_type = ?
               AND entity_id = ?
               AND generation = ?
               AND acknowledged_generation < generation",
        )
        .bind(&dirty.entity_type)
        .bind(&dirty.entity_id)
        .bind(dirty.generation)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    connection.return_to_pool().await;
    Ok(true)
}

async fn build_index_action(
    connection: &mut sqlx::SqliteConnection,
    dirty: &DirtyEntity,
) -> WorkerResult<IndexAction> {
    match dirty.entity_type.as_str() {
        "session" => build_session_document(connection, &dirty.entity_id).await,
        "human" => build_human_document(connection, &dirty.entity_id).await,
        "organization" => build_organization_document(connection, &dirty.entity_id).await,
        entity_type => {
            tracing::warn!(entity_type, "ignoring unknown search index entity type");
            Ok(IndexAction::Skip)
        }
    }
}

async fn build_session_document(
    connection: &mut sqlx::SqliteConnection,
    id: &str,
) -> WorkerResult<IndexAction> {
    let Some(session) = sqlx::query(
        "SELECT title, created_at, event_json, source_apps_json, locked
         FROM sessions
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(&mut *connection)
    .await?
    else {
        return Ok(IndexAction::Remove(id.to_string()));
    };

    let locked: i64 = session.get("locked");
    if locked != 0 {
        let title: String = session.get("title");
        let created_at: String = session.get("created_at");
        let event_json: String = session.get("event_json");
        return Ok(IndexAction::Upsert(SearchDocument {
            id: id.to_string(),
            doc_type: "session".to_string(),
            language: None,
            title: fallback_title(&title, "Untitled"),
            content: String::new(),
            created_at: session_search_timestamp(&event_json, &created_at),
            facets: Vec::new(),
        }));
    }

    let raw_body: Option<String> = sqlx::query_scalar(
        "SELECT body
         FROM session_documents
         WHERE session_id = ? AND kind = 'note' AND deleted_at IS NULL
         ORDER BY CASE WHEN id = ? THEN 0 ELSE 1 END, created_at, id
         LIMIT 1",
    )
    .bind(id)
    .bind(id)
    .fetch_optional(&mut *connection)
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
    .fetch_all(&mut *connection)
    .await?;

    let transcripts: Vec<String> = sqlx::query_scalar(
        "SELECT words_json
         FROM transcripts
         WHERE session_id = ? AND deleted_at IS NULL
         ORDER BY started_at_ms, created_at, id",
    )
    .bind(id)
    .fetch_all(&mut *connection)
    .await?;

    let meeting_chat_messages: Vec<String> = sqlx::query_scalar(
        "SELECT body
         FROM session_documents
         WHERE session_id = ? AND kind = 'meeting_chat' AND deleted_at IS NULL
         ORDER BY sort_order, created_at, id",
    )
    .bind(id)
    .fetch_all(&mut *connection)
    .await?;

    let source_apps_json: String = session.get("source_apps_json");

    let mut content_parts = Vec::with_capacity(
        2 + enhanced_bodies.len() + meeting_chat_messages.len() + transcripts.len(),
    );
    if let Some(raw_body) = raw_body {
        content_parts.push(extract_plain_text(&raw_body));
    }
    content_parts.extend(enhanced_bodies.iter().map(|body| extract_plain_text(body)));
    content_parts.extend(
        meeting_chat_messages
            .iter()
            .map(|message| flatten_meeting_chat(message)),
    );
    content_parts.extend(
        transcripts
            .iter()
            .map(|transcript| flatten_transcript(transcript)),
    );
    content_parts.push(flatten_source_apps(&source_apps_json));

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

async fn build_human_document(
    connection: &mut sqlx::SqliteConnection,
    id: &str,
) -> WorkerResult<IndexAction> {
    let Some(human) = sqlx::query(
        "SELECT name, email, job_title, linkedin_username, created_at, memo
         FROM humans
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(&mut *connection)
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

async fn build_organization_document(
    connection: &mut sqlx::SqliteConnection,
    id: &str,
) -> WorkerResult<IndexAction> {
    let Some(organization) = sqlx::query(
        "SELECT name, created_at
         FROM organizations
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(&mut *connection)
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
    let pending_count = sqlx::query_scalar(
        "SELECT COUNT(*) FROM search_index_dirty
         WHERE generation > acknowledged_generation",
    )
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

fn flatten_meeting_chat(value: &str) -> String {
    let Ok(Value::Object(record)) = serde_json::from_str::<Value>(value) else {
        return extract_plain_text(value);
    };

    let mut parts = ["platform", "sender", "timestamp", "text"]
        .into_iter()
        .filter_map(|key| record.get(key).and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    parts.extend(
        record
            .get("links")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string),
    );

    merge_content(parts.iter().map(String::as_str))
}

fn flatten_source_apps(value: &str) -> String {
    let Ok(Value::Array(source_apps)) = serde_json::from_str::<Value>(value) else {
        return String::new();
    };

    let parts = source_apps
        .iter()
        .filter_map(Value::as_object)
        .flat_map(|source| {
            ["app", "name", "platform"]
                .into_iter()
                .filter_map(|key| source.get(key).and_then(Value::as_str))
        })
        .collect::<Vec<_>>();
    merge_content(parts)
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
        let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
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
        let mut connection = db.pool().acquire().await.unwrap();
        connection.return_to_pool().await;

        assert!(
            try_acknowledge_dirty_entities(
                db.pool(),
                &[DirtyEntity {
                    entity_type: "session".to_string(),
                    entity_id: "session-1".to_string(),
                    generation: queued_generation,
                }],
            )
            .await
            .unwrap()
        );

        let (current_generation, acknowledged_generation): (i64, i64) = sqlx::query_as(
            "SELECT generation, acknowledged_generation FROM search_index_dirty
             WHERE entity_type = 'session' AND entity_id = 'session-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(current_generation, queued_generation + 1);
        assert_eq!(acknowledged_generation, 0);
        let mut connection = db.pool().acquire().await.unwrap();
        connection.return_to_pool().await;

        assert!(
            try_acknowledge_dirty_entities(
                db.pool(),
                &[DirtyEntity {
                    entity_type: "session".to_string(),
                    entity_id: "session-1".to_string(),
                    generation: current_generation,
                }],
            )
            .await
            .unwrap()
        );
        let remaining: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM search_index_dirty
             WHERE entity_type = 'session'
               AND entity_id = 'session-1'
               AND generation > acknowledged_generation",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(remaining, 0);

        let acknowledged_generation: i64 = sqlx::query_scalar(
            "SELECT acknowledged_generation FROM search_index_dirty
             WHERE entity_type = 'session' AND entity_id = 'session-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(acknowledged_generation, current_generation);
    }

    #[tokio::test]
    async fn acknowledgement_defers_when_the_database_pool_is_busy() {
        let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
        let mut held_connection = db.pool().acquire().await.unwrap();
        let started = std::time::Instant::now();

        let acknowledged = try_acknowledge_dirty_entities(db.pool(), &[])
            .await
            .unwrap();

        assert!(!acknowledged);
        assert!(started.elapsed() < Duration::from_millis(100));
        held_connection.return_to_pool().await;
    }

    #[tokio::test]
    async fn session_projection_includes_ordered_live_meeting_chat() {
        let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        sqlx::query("INSERT INTO sessions (id, title) VALUES ('session-1', 'Planning')")
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query(
            r#"
            INSERT INTO session_documents (
                id, session_id, kind, body_format, body, sort_order, deleted_at
            ) VALUES
                (
                    'chat-later', 'session-1', 'meeting_chat', 'json',
                    '{"platform":"zoom","sender":"Grace","timestamp":"10:02","text":"second message","links":["https://example.com/second"]}',
                    20, NULL
                ),
                (
                    'chat-first', 'session-1', 'meeting_chat', 'json',
                    '{"platform":"zoom","sender":"Ada","timestamp":"10:01","text":"first message","links":[]}',
                    10, NULL
                ),
                (
                    'chat-deleted', 'session-1', 'meeting_chat', 'json',
                    '{"platform":"zoom","sender":"Linus","timestamp":"10:03","text":"deleted message","links":[]}',
                    30, '2026-08-23T00:00:00Z'
                )
            "#,
        )
        .execute(db.pool())
        .await
        .unwrap();

        let mut connection = db.pool().acquire().await.unwrap();
        let IndexAction::Upsert(document) = build_session_document(&mut connection, "session-1")
            .await
            .unwrap()
        else {
            panic!("expected the session to be indexed");
        };

        let first = document.content.find("first message").unwrap();
        let second = document.content.find("second message").unwrap();
        assert!(first < second);
        assert!(document.content.contains("Ada"));
        assert!(document.content.contains("Grace"));
        assert!(document.content.contains("https://example.com/second"));
        assert!(!document.content.contains("deleted message"));
    }

    #[tokio::test]
    async fn session_projection_includes_raw_and_normalized_meeting_apps() {
        let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        sqlx::query(
            r#"INSERT INTO sessions (id, title, source_apps_json)
               VALUES (
                 'session-1',
                 'Planning',
                 '[{"app":"com.google.Chrome","name":"Google Chrome","platform":"Google Meet"}]'
               )"#,
        )
        .execute(db.pool())
        .await
        .unwrap();

        let mut connection = db.pool().acquire().await.unwrap();
        let IndexAction::Upsert(document) = build_session_document(&mut connection, "session-1")
            .await
            .unwrap()
        else {
            panic!("expected the session to be indexed");
        };

        assert!(document.content.contains("com.google.Chrome"));
        assert!(document.content.contains("Google Chrome"));
        assert!(document.content.contains("Google Meet"));
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
    fn flattens_meeting_chat_metadata_text_and_links() {
        assert_eq!(
            flatten_meeting_chat(
                r#"{"platform":"zoom","sender":"Ada","timestamp":"10:42 AM","text":"Here is the doc","links":["https://example.com/spec"]}"#,
            ),
            "zoom Ada 10:42 AM Here is the doc https://example.com/spec"
        );
        assert_eq!(flatten_meeting_chat("plain chat"), "plain chat");
    }

    #[test]
    fn flattens_source_app_metadata() {
        assert_eq!(
            flatten_source_apps(
                r#"[{"app":"slack","name":"Slack.exe","platform":"Slack"},{"app":"legacy"}]"#,
            ),
            "slack Slack.exe Slack legacy"
        );
        assert_eq!(flatten_source_apps("not json"), "");
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
