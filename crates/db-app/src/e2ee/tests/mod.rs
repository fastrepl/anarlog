use super::*;
use anlg_e2ee::RecoveryKey;

async fn test_db() -> anlg_db_core::Db {
    let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
    crate::prepare_schema(&db).await.unwrap();
    db
}

fn keys(workspace_id: &str) -> HashMap<String, anlg_e2ee::WorkspaceKeyring> {
    let recovery =
        RecoveryKey::parse("anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc").unwrap();
    HashMap::from([(
        workspace_id.to_string(),
        recovery.workspace_key(workspace_id).unwrap().into(),
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

// Model a field already emitted by Nightly before the compatible-writer rollout.
async fn seed_nightly_field(
    pool: &SqlitePool,
    workspace_keys: &HashMap<String, anlg_e2ee::WorkspaceKeyring>,
    table: &str,
    row_id: &str,
    field: &str,
    value: Value,
) {
    let workspace_id = "workspace-a";
    let key = workspace_keys[workspace_id].active();
    let writer_id = "11111111111111111111111111111111";
    let sealed = key
        .seal_field_at(
            workspace_id,
            table,
            row_id,
            field,
            writer_id,
            1,
            false,
            Some(1),
            value.clone(),
        )
        .unwrap();
    let state = LocalState {
        record_id: sealed.record_id,
        workspace_id: workspace_id.to_string(),
        table_name: table.to_string(),
        row_id: row_id.to_string(),
        field_name: field.to_string(),
        revision: 1,
        writer_id: writer_id.to_string(),
        value_tag: key.value_tag(table, row_id, field, false, &value),
        payload_hash: anlg_e2ee::payload_hash(&sealed.payload),
        payload: sealed.payload,
        edited_at_ms: Some(1),
        republish: false,
    };
    let mut transaction = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO e2ee_records (id, workspace_id, payload) VALUES (?, ?, ?)")
        .bind(&state.record_id)
        .bind(workspace_id)
        .bind(&state.payload)
        .execute(&mut *transaction)
        .await
        .unwrap();
    replica_storage::upsert_local_state(&mut transaction, &state)
        .await
        .unwrap();
    transaction.commit().await.unwrap();
}

mod block_merge;
mod chunked_fields;
mod compatibility;
mod convergence;
mod convergence_fuzz;
mod dirty_rows;
mod document_versions;
mod edit_conflicts;
mod library;
mod replica_apply;
mod revision_conflicts;
mod roundtrip;
mod session_deletion;
mod snapshots;
mod synced_tables;
mod witness_queue;
