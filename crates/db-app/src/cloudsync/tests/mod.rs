use super::*;
use sqlx::SqlitePool;

mod binding;
mod projection;
mod recovery;

async fn test_db() -> anlg_db_core::Db {
    let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
    crate::prepare_schema(&db).await.unwrap();
    db
}

fn recovery_key(workspace_id: &str) -> anlg_e2ee::WorkspaceKey {
    anlg_e2ee::RecoveryKey::parse("anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc")
        .unwrap()
        .workspace_key(workspace_id)
        .unwrap()
}

async fn mark_full_resync_pending(pool: &SqlitePool, generation: &str) {
    sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
        .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
        .bind(serde_json::to_string(generation).unwrap())
        .execute(pool)
        .await
        .unwrap();
}

fn projection(
    account_user_id: &str,
    workspaces: Vec<CloudsyncWorkspaceProjectionEntry>,
) -> CloudsyncWorkspaceProjection {
    CloudsyncWorkspaceProjection {
        account_user_id: account_user_id.to_string(),
        personal_workspace_id: account_user_id.to_string(),
        workspaces,
    }
}

fn projected_workspace(
    id: &str,
    owner_user_id: &str,
    kind: &str,
    membership_id: &str,
    role: &str,
    name: &str,
) -> CloudsyncWorkspaceProjectionEntry {
    CloudsyncWorkspaceProjectionEntry {
        id: id.to_string(),
        owner_user_id: owner_user_id.to_string(),
        kind: kind.to_string(),
        name: name.to_string(),
        membership_id: membership_id.to_string(),
        role: role.to_string(),
        membership_created_at: "2026-07-16T00:01:00Z".to_string(),
        membership_updated_at: "2026-07-16T00:02:00Z".to_string(),
        created_at: "2026-07-16T00:00:00Z".to_string(),
        updated_at: "2026-07-16T00:00:00Z".to_string(),
    }
}

async fn seed_legacy_workspace_projection_rows(pool: &SqlitePool, last_row_index: i64) {
    sqlx::query(
        "WITH RECURSIVE sequence(row_index) AS (
           VALUES (0)
           UNION ALL
           SELECT row_index + 1
           FROM sequence
           WHERE row_index < ?
         )
         INSERT INTO workspaces (id, owner_user_id, kind, name)
         SELECT
           printf('legacy-workspace-%04d', row_index),
           'legacy-owner',
           'shared',
           'Legacy'
         FROM sequence",
    )
    .bind(last_row_index)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "WITH RECURSIVE sequence(row_index) AS (
           VALUES (0)
           UNION ALL
           SELECT row_index + 1
           FROM sequence
           WHERE row_index < ?
         )
         INSERT INTO workspace_memberships (id, workspace_id, user_id, role)
         SELECT
           printf('legacy-membership-%04d', row_index),
           printf('legacy-workspace-%04d', row_index),
           'user-a',
           'member'
         FROM sequence",
    )
    .bind(last_row_index)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn retire_legacy_cloudsync_tables_drops_cloudsync_metadata_and_preserves_data() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("retire.db");
    {
        let db = anlg_db_core::Db::open(anlg_db_core::DbOpenOptions {
            storage: anlg_db_core::DbStorage::Local(&db_path),
            cloudsync_enabled: true,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(1),
        })
        .await
        .unwrap();
        crate::prepare_schema(&db).await.unwrap();
        db.cloudsync_init("sessions", None, None).await.unwrap();
        db.cloudsync_init("e2ee_records", None, None).await.unwrap();
        assert!(
            anlg_db_core::cloudsync_is_enabled_on(db.pool(), "e2ee_records")
                .await
                .unwrap()
        );
        sqlx::query(
            "INSERT INTO sessions(id, workspace_id, owner_user_id, title) VALUES('s1', 'ws-1', 'u-1', 'Kept Note')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO e2ee_records(id, workspace_id, payload) VALUES('r1', 'ws-1', '{\"version\":1}')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let retired = retire_legacy_cloudsync_tables(&db).await.unwrap();
        assert!(retired >= 1);
        assert!(
            !anlg_db_core::cloudsync_is_enabled_on(db.pool(), "e2ee_records")
                .await
                .unwrap()
        );
        assert_eq!(retire_legacy_cloudsync_tables(&db).await.unwrap(), 0);
        db.pool().close().await;
    }

    let plain = anlg_db_core::Db::connect_local_plain(&db_path)
        .await
        .unwrap();
    crate::prepare_schema(&plain).await.unwrap();
    let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 's1'")
        .fetch_one(plain.pool())
        .await
        .unwrap();
    assert_eq!(title, "Kept Note");
    let payload: String = sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = 'r1'")
        .fetch_one(plain.pool())
        .await
        .unwrap();
    assert_eq!(payload, "{\"version\":1}");
}
