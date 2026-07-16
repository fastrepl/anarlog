use std::collections::{BTreeMap, HashMap, HashSet};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hypr_e2ee::{OpenedField, WorkspaceKey};
use serde_json::{Value, json};
use sqlx::sqlite::SqliteRow;
use sqlx::{Column, QueryBuilder, Row, Sqlite, SqlitePool, Transaction, TypeInfo, ValueRef};

pub const E2EE_DOMAIN_TABLES: &[&str] = &[
    "action_items",
    "humans",
    "organizations",
    "session_attachments",
    "session_documents",
    "session_participants",
    "sessions",
    "transcripts",
];

const ROW_MANIFEST_FIELD: &str = "$row";

#[derive(Debug, thiserror::Error)]
pub enum E2eeReplicaError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Crypto(#[from] hypr_e2ee::Error),
    #[error("encrypted replica contains an invalid table or field")]
    InvalidField,
    #[error("encrypted replica contains an unsupported SQLite value")]
    UnsupportedValue,
    #[error("encrypted replica contains an invalid row")]
    InvalidRow,
    #[error("encrypted replica rollback was detected")]
    RollbackDetected,
}

pub type E2eeReplicaResult<T> = std::result::Result<T, E2eeReplicaError>;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct E2eeReplicaStats {
    pub encrypted_fields: u64,
    pub applied_fields: u64,
    pub skipped_local_changes: u64,
}

#[derive(Clone, sqlx::FromRow)]
struct LocalState {
    record_id: String,
    workspace_id: String,
    table_name: String,
    row_id: String,
    field_name: String,
    revision: i64,
    value_tag: String,
    payload_hash: String,
}

#[derive(sqlx::FromRow)]
struct EncryptedRecord {
    id: String,
    workspace_id: String,
    payload: String,
}

struct DecryptedRecord {
    record_id: String,
    workspace_id: String,
    payload_hash: String,
    field: OpenedField,
}

pub async fn encrypt_e2ee_replica_changes(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
) -> E2eeReplicaResult<E2eeReplicaStats> {
    if keys.is_empty() {
        return Ok(E2eeReplicaStats::default());
    }

    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let states = load_local_states(&mut transaction).await?;
    let mut active_manifests = HashSet::new();
    let mut stats = E2eeReplicaStats::default();

    for table in E2EE_DOMAIN_TABLES {
        let sql = format!("SELECT * FROM {table}");
        let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .fetch_all(&mut *transaction)
            .await?;

        for row in rows {
            let workspace_id: String = row.try_get("workspace_id")?;
            let Some(key) = keys.get(&workspace_id) else {
                continue;
            };
            let row_id: String = row.try_get("id")?;
            if row_id.is_empty() {
                return Err(E2eeReplicaError::InvalidRow);
            }

            let manifest_id = key.blind_field_id(table, &row_id, ROW_MANIFEST_FIELD);
            active_manifests.insert(manifest_id);
            if encrypt_field_if_changed(
                &mut transaction,
                &states,
                key,
                &workspace_id,
                table,
                &row_id,
                ROW_MANIFEST_FIELD,
                false,
                json!(true),
            )
            .await?
            {
                stats.encrypted_fields += 1;
            }

            for (index, column) in row.columns().iter().enumerate() {
                let field = column.name();
                if matches!(field, "id" | "workspace_id") {
                    continue;
                }
                let value = sqlite_value(&row, index)?;
                if encrypt_field_if_changed(
                    &mut transaction,
                    &states,
                    key,
                    &workspace_id,
                    table,
                    &row_id,
                    field,
                    false,
                    value,
                )
                .await?
                {
                    stats.encrypted_fields += 1;
                }
            }
        }
    }

    for state in states.values().filter(|state| {
        state.field_name == ROW_MANIFEST_FIELD
            && keys.contains_key(&state.workspace_id)
            && !active_manifests.contains(&state.record_id)
    }) {
        let key = &keys[&state.workspace_id];
        let deletion_tag = key.value_tag(
            &state.table_name,
            &state.row_id,
            ROW_MANIFEST_FIELD,
            true,
            &Value::Null,
        );
        if state.value_tag == deletion_tag {
            continue;
        }
        if encrypt_field_if_changed(
            &mut transaction,
            &states,
            key,
            &state.workspace_id,
            &state.table_name,
            &state.row_id,
            ROW_MANIFEST_FIELD,
            true,
            Value::Null,
        )
        .await?
        {
            stats.encrypted_fields += 1;
        }
    }

    transaction.commit().await?;
    Ok(stats)
}

pub async fn apply_e2ee_replica_changes(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
) -> E2eeReplicaResult<E2eeReplicaStats> {
    if keys.is_empty() {
        return Ok(E2eeReplicaStats::default());
    }

    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let records: Vec<EncryptedRecord> = sqlx::query_as(
        "SELECT id, workspace_id, payload FROM e2ee_records ORDER BY workspace_id, id",
    )
    .fetch_all(&mut *transaction)
    .await?;
    let mut states = load_local_states(&mut transaction).await?;
    let mut groups = BTreeMap::<(String, String, String), Vec<DecryptedRecord>>::new();

    for record in records {
        let Some(key) = keys.get(&record.workspace_id) else {
            continue;
        };
        let field = key.open_field(&record.workspace_id, &record.id, &record.payload)?;
        if !E2EE_DOMAIN_TABLES.contains(&field.table.as_str()) {
            return Err(E2eeReplicaError::InvalidField);
        }
        groups
            .entry((
                record.workspace_id.clone(),
                field.table.clone(),
                field.row_id.clone(),
            ))
            .or_default()
            .push(DecryptedRecord {
                record_id: record.id,
                workspace_id: record.workspace_id,
                payload_hash: hypr_e2ee::payload_hash(&record.payload),
                field,
            });
    }

    let mut stats = E2eeReplicaStats::default();
    let mut column_cache = HashMap::<String, HashSet<String>>::new();
    for ((workspace_id, table, row_id), mut records) in groups {
        let key = &keys[&workspace_id];
        let columns = match column_cache.get(&table) {
            Some(columns) => columns.clone(),
            None => {
                let columns = table_columns(&mut transaction, &table).await?;
                column_cache.insert(table.clone(), columns.clone());
                columns
            }
        };
        let Some(manifest_index) = records
            .iter()
            .position(|record| record.field.field == ROW_MANIFEST_FIELD)
        else {
            continue;
        };
        let manifest = records.swap_remove(manifest_index);
        let manifest_state = states.get(&manifest.record_id).cloned();
        reject_rollback(manifest_state.as_ref(), &manifest.field)?;
        let manifest_unchanged = manifest_state
            .as_ref()
            .is_some_and(|state| state.payload_hash == manifest.payload_hash);
        let row_exists = row_exists(&mut transaction, &table, &workspace_id, &row_id).await?;

        if !manifest_unchanged {
            let locally_changed = row_changed_since_snapshot(
                &mut transaction,
                key,
                &workspace_id,
                &table,
                &row_id,
                row_exists,
                &states,
            )
            .await?;
            if locally_changed {
                stats.skipped_local_changes += records.len() as u64 + 1;
                continue;
            }

            if manifest.field.deleted {
                delete_row(&mut transaction, &table, &workspace_id, &row_id).await?;
                let value_tag =
                    key.value_tag(&table, &row_id, ROW_MANIFEST_FIELD, true, &Value::Null);
                let state = LocalState {
                    record_id: manifest.record_id,
                    workspace_id,
                    table_name: table,
                    row_id,
                    field_name: ROW_MANIFEST_FIELD.to_string(),
                    revision: i64::try_from(manifest.field.revision)
                        .map_err(|_| E2eeReplicaError::InvalidRow)?,
                    value_tag,
                    payload_hash: manifest.payload_hash,
                };
                upsert_local_state(&mut transaction, &state).await?;
                stats.applied_fields += 1;
                continue;
            }

            if !row_exists {
                insert_row(&mut transaction, &table, &workspace_id, &row_id).await?;
            }
            let value_tag = key.value_tag(&table, &row_id, ROW_MANIFEST_FIELD, false, &json!(true));
            let state = LocalState {
                record_id: manifest.record_id,
                workspace_id: workspace_id.clone(),
                table_name: table.clone(),
                row_id: row_id.clone(),
                field_name: ROW_MANIFEST_FIELD.to_string(),
                revision: i64::try_from(manifest.field.revision)
                    .map_err(|_| E2eeReplicaError::InvalidRow)?,
                value_tag,
                payload_hash: manifest.payload_hash,
            };
            upsert_local_state(&mut transaction, &state).await?;
            states.insert(state.record_id.clone(), state);
            stats.applied_fields += 1;
        } else if !row_exists || manifest.field.deleted {
            continue;
        }

        for record in records {
            let field_name = record.field.field.as_str();
            if field_name == ROW_MANIFEST_FIELD
                || field_name == "id"
                || field_name == "workspace_id"
                || !columns.contains(field_name)
                || record.field.deleted
            {
                return Err(E2eeReplicaError::InvalidField);
            }
            if states
                .get(&record.record_id)
                .is_some_and(|state| state.payload_hash == record.payload_hash)
            {
                continue;
            }
            reject_rollback(states.get(&record.record_id), &record.field)?;

            if let Some(state) = states.get(&record.record_id) {
                let Some(current) =
                    read_field(&mut transaction, &table, &workspace_id, &row_id, field_name)
                        .await?
                else {
                    stats.skipped_local_changes += 1;
                    continue;
                };
                let current_tag = key.value_tag(&table, &row_id, field_name, false, &current);
                if current_tag != state.value_tag {
                    stats.skipped_local_changes += 1;
                    continue;
                }
            }

            update_field(
                &mut transaction,
                &table,
                &workspace_id,
                &row_id,
                field_name,
                &record.field.value,
            )
            .await?;
            let value_tag = key.value_tag(&table, &row_id, field_name, false, &record.field.value);
            let state = LocalState {
                record_id: record.record_id,
                workspace_id: record.workspace_id,
                table_name: table.clone(),
                row_id: row_id.clone(),
                field_name: field_name.to_string(),
                revision: i64::try_from(record.field.revision)
                    .map_err(|_| E2eeReplicaError::InvalidRow)?,
                value_tag,
                payload_hash: record.payload_hash,
            };
            upsert_local_state(&mut transaction, &state).await?;
            states.insert(state.record_id.clone(), state);
            stats.applied_fields += 1;
        }
    }

    transaction.commit().await?;
    Ok(stats)
}

#[allow(clippy::too_many_arguments)]
async fn encrypt_field_if_changed(
    transaction: &mut Transaction<'_, Sqlite>,
    states: &HashMap<String, LocalState>,
    key: &WorkspaceKey,
    workspace_id: &str,
    table: &str,
    row_id: &str,
    field: &str,
    deleted: bool,
    value: Value,
) -> E2eeReplicaResult<bool> {
    let record_id = key.blind_field_id(table, row_id, field);
    let value_tag = key.value_tag(table, row_id, field, deleted, &value);
    if states
        .get(&record_id)
        .is_some_and(|state| state.value_tag == value_tag)
    {
        return Ok(false);
    }

    let revision = states
        .get(&record_id)
        .map(|state| state.revision.saturating_add(1))
        .unwrap_or(1);
    let revision = u64::try_from(revision).map_err(|_| E2eeReplicaError::InvalidRow)?;
    let sealed = key.seal_field(workspace_id, table, row_id, field, revision, deleted, value)?;
    let payload_hash = hypr_e2ee::payload_hash(&sealed.payload);
    sqlx::query(
        "INSERT INTO e2ee_records (id, workspace_id, payload)
         VALUES (?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           payload = excluded.payload,
           updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE e2ee_records.workspace_id = excluded.workspace_id",
    )
    .bind(&sealed.record_id)
    .bind(workspace_id)
    .bind(&sealed.payload)
    .execute(&mut **transaction)
    .await?;
    upsert_local_state(
        transaction,
        &LocalState {
            record_id: sealed.record_id,
            workspace_id: workspace_id.to_string(),
            table_name: table.to_string(),
            row_id: row_id.to_string(),
            field_name: field.to_string(),
            revision: i64::try_from(revision).map_err(|_| E2eeReplicaError::InvalidRow)?,
            value_tag,
            payload_hash,
        },
    )
    .await?;
    Ok(true)
}

async fn load_local_states(
    transaction: &mut Transaction<'_, Sqlite>,
) -> E2eeReplicaResult<HashMap<String, LocalState>> {
    let states: Vec<LocalState> = sqlx::query_as(
        "SELECT record_id, workspace_id, table_name, row_id, field_name, revision, value_tag, payload_hash
         FROM e2ee_local_state",
    )
    .fetch_all(&mut **transaction)
    .await?;
    Ok(states
        .into_iter()
        .map(|state| (state.record_id.clone(), state))
        .collect())
}

async fn upsert_local_state(
    transaction: &mut Transaction<'_, Sqlite>,
    state: &LocalState,
) -> E2eeReplicaResult<()> {
    sqlx::query(
        "INSERT INTO e2ee_local_state (
           record_id, workspace_id, table_name, row_id, field_name, revision, value_tag, payload_hash
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(record_id) DO UPDATE SET
           workspace_id = excluded.workspace_id,
           table_name = excluded.table_name,
           row_id = excluded.row_id,
           field_name = excluded.field_name,
           revision = excluded.revision,
           value_tag = excluded.value_tag,
           payload_hash = excluded.payload_hash,
           updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    )
    .bind(&state.record_id)
    .bind(&state.workspace_id)
    .bind(&state.table_name)
    .bind(&state.row_id)
    .bind(&state.field_name)
    .bind(state.revision)
    .bind(&state.value_tag)
    .bind(&state.payload_hash)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn table_columns(
    transaction: &mut Transaction<'_, Sqlite>,
    table: &str,
) -> E2eeReplicaResult<HashSet<String>> {
    if !E2EE_DOMAIN_TABLES.contains(&table) {
        return Err(E2eeReplicaError::InvalidField);
    }
    let sql = format!("PRAGMA table_info({table})");
    let columns = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .fetch_all(&mut **transaction)
        .await?
        .into_iter()
        .map(|row| row.try_get("name"))
        .collect::<std::result::Result<Vec<String>, _>>()?;
    Ok(columns.into_iter().collect())
}

async fn row_exists(
    transaction: &mut Transaction<'_, Sqlite>,
    table: &str,
    workspace_id: &str,
    row_id: &str,
) -> E2eeReplicaResult<bool> {
    let sql = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ? AND workspace_id = ?)");
    Ok(sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(row_id)
        .bind(workspace_id)
        .fetch_one(&mut **transaction)
        .await?)
}

async fn insert_row(
    transaction: &mut Transaction<'_, Sqlite>,
    table: &str,
    workspace_id: &str,
    row_id: &str,
) -> E2eeReplicaResult<()> {
    let sql =
        format!("INSERT INTO {table} (id, workspace_id) VALUES (?, ?) ON CONFLICT(id) DO NOTHING");
    sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(row_id)
        .bind(workspace_id)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

async fn delete_row(
    transaction: &mut Transaction<'_, Sqlite>,
    table: &str,
    workspace_id: &str,
    row_id: &str,
) -> E2eeReplicaResult<()> {
    let sql = format!("DELETE FROM {table} WHERE id = ? AND workspace_id = ?");
    sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(row_id)
        .bind(workspace_id)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

async fn read_field(
    transaction: &mut Transaction<'_, Sqlite>,
    table: &str,
    workspace_id: &str,
    row_id: &str,
    field: &str,
) -> E2eeReplicaResult<Option<Value>> {
    let sql = format!("SELECT {field} FROM {table} WHERE id = ? AND workspace_id = ? LIMIT 1");
    let row = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(row_id)
        .bind(workspace_id)
        .fetch_optional(&mut **transaction)
        .await?;
    row.as_ref().map(|row| sqlite_value(row, 0)).transpose()
}

async fn update_field(
    transaction: &mut Transaction<'_, Sqlite>,
    table: &str,
    workspace_id: &str,
    row_id: &str,
    field: &str,
    value: &Value,
) -> E2eeReplicaResult<()> {
    let mut query = QueryBuilder::new(format!("UPDATE {table} SET {field} = "));
    push_json_bind(&mut query, value)?;
    query.push(" WHERE id = ").push_bind(row_id);
    query.push(" AND workspace_id = ").push_bind(workspace_id);
    query.build().execute(&mut **transaction).await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn row_changed_since_snapshot(
    transaction: &mut Transaction<'_, Sqlite>,
    key: &WorkspaceKey,
    workspace_id: &str,
    table: &str,
    row_id: &str,
    row_exists: bool,
    states: &HashMap<String, LocalState>,
) -> E2eeReplicaResult<bool> {
    let row_states = states.values().filter(|state| {
        state.workspace_id == workspace_id && state.table_name == table && state.row_id == row_id
    });
    let mut found_manifest = false;
    for state in row_states {
        if state.field_name == ROW_MANIFEST_FIELD {
            found_manifest = true;
            let value = if row_exists { json!(true) } else { Value::Null };
            let current_tag = key.value_tag(table, row_id, ROW_MANIFEST_FIELD, !row_exists, &value);
            if current_tag != state.value_tag {
                return Ok(true);
            }
            continue;
        }
        let Some(value) =
            read_field(transaction, table, workspace_id, row_id, &state.field_name).await?
        else {
            return Ok(true);
        };
        let current_tag = key.value_tag(table, row_id, &state.field_name, false, &value);
        if current_tag != state.value_tag {
            return Ok(true);
        }
    }
    Ok(row_exists && !found_manifest)
}

fn sqlite_value(row: &SqliteRow, index: usize) -> E2eeReplicaResult<Value> {
    let raw = row.try_get_raw(index)?;
    if raw.is_null() {
        return Ok(Value::Null);
    }

    match raw.type_info().name() {
        "INTEGER" => Ok(json!(row.try_get::<i64, _>(index)?)),
        "REAL" => Ok(json!(row.try_get::<f64, _>(index)?)),
        "TEXT" => Ok(json!(row.try_get::<String, _>(index)?)),
        "BLOB" => Ok(json!({
            "$anarlog_blob": URL_SAFE_NO_PAD.encode(row.try_get::<Vec<u8>, _>(index)?)
        })),
        _ => Err(E2eeReplicaError::UnsupportedValue),
    }
}

fn push_json_bind(query: &mut QueryBuilder<Sqlite>, value: &Value) -> E2eeReplicaResult<()> {
    match value {
        Value::Null => {
            query.push_bind(None::<String>);
        }
        Value::Bool(value) => {
            query.push_bind(i64::from(*value));
        }
        Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                query.push_bind(value);
            } else if let Some(value) = value.as_f64() {
                query.push_bind(value);
            } else {
                return Err(E2eeReplicaError::UnsupportedValue);
            }
        }
        Value::String(value) => {
            query.push_bind(value);
        }
        Value::Object(value) if value.len() == 1 && value.contains_key("$anarlog_blob") => {
            let bytes = value
                .get("$anarlog_blob")
                .and_then(Value::as_str)
                .ok_or(E2eeReplicaError::UnsupportedValue)
                .and_then(|value| {
                    URL_SAFE_NO_PAD
                        .decode(value)
                        .map_err(|_| E2eeReplicaError::UnsupportedValue)
                })?;
            query.push_bind(bytes);
        }
        Value::Array(_) | Value::Object(_) => {
            return Err(E2eeReplicaError::UnsupportedValue);
        }
    }
    Ok(())
}

fn reject_rollback(state: Option<&LocalState>, field: &OpenedField) -> E2eeReplicaResult<()> {
    let revision = i64::try_from(field.revision).map_err(|_| E2eeReplicaError::InvalidRow)?;
    if state.is_some_and(|state| revision < state.revision) {
        return Err(E2eeReplicaError::RollbackDetected);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_e2ee::RecoveryKey;

    async fn test_db() -> hypr_db_core::Db {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        crate::prepare_schema(&db).await.unwrap();
        db
    }

    fn keys(workspace_id: &str) -> HashMap<String, WorkspaceKey> {
        let recovery =
            RecoveryKey::parse("anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc")
                .unwrap();
        HashMap::from([(
            workspace_id.to_string(),
            recovery.workspace_key(workspace_id).unwrap(),
        )])
    }

    async fn copy_replica(source: &SqlitePool, target: &SqlitePool) {
        let records: Vec<(String, String, String)> =
            sqlx::query_as("SELECT id, workspace_id, payload FROM e2ee_records")
                .fetch_all(source)
                .await
                .unwrap();
        for (id, workspace_id, payload) in records {
            sqlx::query(
                "INSERT INTO e2ee_records (id, workspace_id, payload) VALUES (?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET payload = excluded.payload",
            )
            .bind(id)
            .bind(workspace_id)
            .bind(payload)
            .execute(target)
            .await
            .unwrap();
        }
    }

    #[tokio::test]
    async fn encrypts_only_opaque_records() {
        let db = test_db().await;
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Secret planning')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let stats = encrypt_e2ee_replica_changes(db.pool(), &keys("workspace-a"))
            .await
            .unwrap();
        let payloads: Vec<String> = sqlx::query_scalar("SELECT payload FROM e2ee_records")
            .fetch_all(db.pool())
            .await
            .unwrap();

        assert!(stats.encrypted_fields > 1);
        assert!(!payloads.is_empty());
        assert!(payloads.iter().all(|payload| {
            !payload.contains("Secret planning")
                && !payload.contains("session-1")
                && !payload.contains("sessions")
        }));
    }

    #[tokio::test]
    async fn reconstructs_every_protected_table_and_applies_deletions() {
        let workspace_keys = keys("workspace-a");
        let source = test_db().await;
        for (table, id) in [
            ("action_items", "action-1"),
            ("humans", "human-1"),
            ("organizations", "organization-1"),
            ("session_attachments", "attachment-1"),
            ("session_documents", "document-1"),
            ("session_participants", "participant-1"),
            ("sessions", "session-1"),
            ("transcripts", "transcript-1"),
        ] {
            let sql = format!("INSERT INTO {table} (id, workspace_id) VALUES (?, 'workspace-a')");
            sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(id)
                .execute(source.pool())
                .await
                .unwrap();
        }
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();

        let target = test_db().await;
        copy_replica(source.pool(), target.pool()).await;
        apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();
        for table in E2EE_DOMAIN_TABLES {
            let sql = format!("SELECT COUNT(*) FROM {table} WHERE workspace_id = 'workspace-a'");
            let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
                .fetch_one(target.pool())
                .await
                .unwrap();
            assert_eq!(count, 1, "{table} was not reconstructed");
        }

        sqlx::query("DELETE FROM sessions WHERE id = 'session-1'")
            .execute(source.pool())
            .await
            .unwrap();
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();
        copy_replica(source.pool(), target.pool()).await;
        apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();
        let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
            .fetch_one(target.pool())
            .await
            .unwrap();
        assert_eq!(sessions, 0);
    }

    #[tokio::test]
    async fn applies_remote_changes_and_preserves_concurrent_local_edits() {
        let workspace_keys = keys("workspace-a");
        let source = test_db().await;
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'First')",
        )
        .execute(source.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();

        let target = test_db().await;
        let records: Vec<(String, String, String)> =
            sqlx::query_as("SELECT id, workspace_id, payload FROM e2ee_records")
                .fetch_all(source.pool())
                .await
                .unwrap();
        for (id, workspace_id, payload) in records {
            sqlx::query("INSERT INTO e2ee_records (id, workspace_id, payload) VALUES (?, ?, ?)")
                .bind(id)
                .bind(workspace_id)
                .bind(payload)
                .execute(target.pool())
                .await
                .unwrap();
        }
        apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();
        let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
            .fetch_one(target.pool())
            .await
            .unwrap();
        assert_eq!(title, "First");

        sqlx::query("UPDATE sessions SET title = 'Remote' WHERE id = 'session-1'")
            .execute(source.pool())
            .await
            .unwrap();
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();
        let key = &workspace_keys["workspace-a"];
        let title_record_id = key.blind_field_id("sessions", "session-1", "title");
        let remote_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&title_record_id)
                .fetch_one(source.pool())
                .await
                .unwrap();
        sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
            .bind(remote_payload)
            .bind(title_record_id)
            .execute(target.pool())
            .await
            .unwrap();
        sqlx::query("UPDATE sessions SET title = 'Local' WHERE id = 'session-1'")
            .execute(target.pool())
            .await
            .unwrap();

        let stats = apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();
        let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
            .fetch_one(target.pool())
            .await
            .unwrap();

        assert_eq!(title, "Local");
        assert_eq!(stats.skipped_local_changes, 1);
    }

    #[tokio::test]
    async fn rejects_authenticated_payloads_in_the_wrong_workspace() {
        let db = test_db().await;
        let workspace_keys = keys("workspace-a");
        let key = &workspace_keys["workspace-a"];
        let sealed = key
            .seal_field(
                "workspace-a",
                "sessions",
                "session-1",
                ROW_MANIFEST_FIELD,
                1,
                false,
                json!(true),
            )
            .unwrap();
        sqlx::query(
            "INSERT INTO e2ee_records (id, workspace_id, payload) VALUES (?, 'workspace-b', ?)",
        )
        .bind(sealed.record_id)
        .bind(sealed.payload)
        .execute(db.pool())
        .await
        .unwrap();

        let mut wrong_keys = workspace_keys;
        wrong_keys.insert(
            "workspace-b".to_string(),
            RecoveryKey::parse("anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc")
                .unwrap()
                .workspace_key("workspace-b")
                .unwrap(),
        );
        assert!(
            apply_e2ee_replica_changes(db.pool(), &wrong_keys)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn rejects_replayed_older_field_revisions() {
        let db = test_db().await;
        let workspace_keys = keys("workspace-a");
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'First')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();
        let record_id =
            workspace_keys["workspace-a"].blind_field_id("sessions", "session-1", "title");
        let old_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&record_id)
                .fetch_one(db.pool())
                .await
                .unwrap();

        sqlx::query("UPDATE sessions SET title = 'Second' WHERE id = 'session-1'")
            .execute(db.pool())
            .await
            .unwrap();
        encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();
        apply_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();
        sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
            .bind(old_payload)
            .bind(record_id)
            .execute(db.pool())
            .await
            .unwrap();

        let error = apply_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap_err();
        assert!(matches!(error, E2eeReplicaError::RollbackDetected));
    }
}
