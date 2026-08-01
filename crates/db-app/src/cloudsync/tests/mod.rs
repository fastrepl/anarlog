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
