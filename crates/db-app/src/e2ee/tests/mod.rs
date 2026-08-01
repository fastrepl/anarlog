use super::*;
use anlg_e2ee::RecoveryKey;

async fn test_db() -> anlg_db_core::Db {
    let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
    crate::prepare_schema(&db).await.unwrap();
    db
}

fn keys(workspace_id: &str) -> HashMap<String, WorkspaceKey> {
    let recovery =
        RecoveryKey::parse("anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc").unwrap();
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

mod convergence;
mod dirty_rows;
mod replica_apply;
mod revision_conflicts;
mod roundtrip;
mod snapshots;
mod witness_queue;
