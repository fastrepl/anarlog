use crate::CloudsyncTokenConfigurationResult;

use super::support::{
    setup_enabled_cloudsync_runtime, setup_runtime, setup_witness, unreachable_witness,
};

#[tokio::test]
async fn account_binding_is_durable_without_rekeying_local_rows() {
    let (_dir, runtime) = setup_runtime().await;
    let local_workspace = anlg_db_app::ensure_cloudsync_workspace_binding(runtime.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO humans (id, workspace_id, owner_user_id, name)
         VALUES (?, ?, ?, 'Local user')",
    )
    .bind(&local_workspace)
    .bind(&local_workspace)
    .bind(&local_workspace)
    .execute(runtime.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
         VALUES ('session', ?, ?, 'Session')",
    )
    .bind(&local_workspace)
    .bind(&local_workspace)
    .execute(runtime.pool())
    .await
    .unwrap();

    assert!(
        runtime
            .bind_cloudsync_account("user-a".to_string())
            .await
            .unwrap()
    );

    let binding: (String, String) = sqlx::query_as(
        "SELECT json_extract(value_json, '$.workspace_id'),
                json_extract(value_json, '$.account_user_id')
         FROM app_settings WHERE id = 'cloudsync_workspace_binding'",
    )
    .fetch_one(runtime.pool())
    .await
    .unwrap();
    let human: (String, String, String) = sqlx::query_as(
        "SELECT id, workspace_id, owner_user_id FROM humans WHERE name = 'Local user'",
    )
    .fetch_one(runtime.pool())
    .await
    .unwrap();
    let session: (String, String) =
        sqlx::query_as("SELECT workspace_id, owner_user_id FROM sessions WHERE id = 'session'")
            .fetch_one(runtime.pool())
            .await
            .unwrap();

    assert_eq!(binding, (local_workspace.clone(), "user-a".to_string()));
    assert_eq!(
        human,
        (
            local_workspace.clone(),
            local_workspace.clone(),
            local_workspace.clone(),
        )
    );
    assert_eq!(session, (local_workspace.clone(), local_workspace));
    assert!(
        !runtime
            .bind_cloudsync_account("user-b".to_string())
            .await
            .unwrap()
    );
    assert_eq!(
        runtime.cloudsync_status().await.unwrap()["configured"],
        false
    );
}

#[tokio::test]
async fn token_configuration_rejects_local_only_runtime_before_rekeying() {
    let (_dir, runtime) = setup_runtime().await;
    let local_workspace = anlg_db_app::ensure_cloudsync_workspace_binding(runtime.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
         VALUES ('session', ?, ?, 'Session')",
    )
    .bind(&local_workspace)
    .bind(&local_workspace)
    .execute(runtime.pool())
    .await
    .unwrap();

    let error = runtime
        .configure_cloudsync_token(
            "managed-database-id".to_string(),
            "token".to_string(),
            "user-a".to_string(),
            unreachable_witness("user-a"),
        )
        .await
        .unwrap_err();

    let binding: (String, Option<String>) = sqlx::query_as(
        "SELECT json_extract(value_json, '$.workspace_id'),
                json_extract(value_json, '$.account_user_id')
         FROM app_settings WHERE id = 'cloudsync_workspace_binding'",
    )
    .fetch_one(runtime.pool())
    .await
    .unwrap();
    let session: (String, String) =
        sqlx::query_as("SELECT workspace_id, owner_user_id FROM sessions WHERE id = 'session'")
            .fetch_one(runtime.pool())
            .await
            .unwrap();

    assert!(matches!(
        error,
        crate::Error::Cloudsync(anlg_db_core::CloudsyncRuntimeError::Unavailable)
    ));
    assert_eq!(binding, (local_workspace.clone(), None));
    assert_eq!(session, (local_workspace.clone(), local_workspace));
    assert_eq!(
        runtime.cloudsync_status().await.unwrap()["configured"],
        false
    );
}

#[tokio::test]
async fn invalid_projection_does_not_claim_an_unbound_database() {
    let (_dir, runtime) = setup_enabled_cloudsync_runtime().await;
    sqlx::query("DELETE FROM app_settings WHERE id = 'cloudsync_workspace_binding'")
        .execute(runtime.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO sessions (id, title) VALUES ('session', 'Session')")
        .execute(runtime.pool())
        .await
        .unwrap();

    let error = runtime
        .configure_cloudsync_token_with_projection(
            "managed-database-id".to_string(),
            "token".to_string(),
            "user-a".to_string(),
            Some(anlg_db_app::CloudsyncWorkspaceProjection {
                account_user_id: "user-a".to_string(),
                personal_workspace_id: "user-a".to_string(),
                workspaces: vec![],
            }),
            unreachable_witness("user-a"),
        )
        .await
        .unwrap_err();

    let binding_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM app_settings WHERE id = 'cloudsync_workspace_binding'",
    )
    .fetch_one(runtime.pool())
    .await
    .unwrap();
    let session: (String, String) =
        sqlx::query_as("SELECT workspace_id, owner_user_id FROM sessions WHERE id = 'session'")
            .fetch_one(runtime.pool())
            .await
            .unwrap();

    assert!(matches!(
        error,
        crate::Error::CloudsyncWorkspace(
            anlg_db_app::CloudsyncWorkspaceError::InvalidWorkspaceProjection
        )
    ));
    assert_eq!(binding_count, 0);
    assert_eq!(session, (String::new(), String::new()));
}

#[tokio::test]
async fn token_configuration_claims_workspace_and_can_be_suspended() {
    let (_dir, runtime) = setup_enabled_cloudsync_runtime().await;
    let (_witness_server, witness) = setup_witness("user-a").await;
    let local_workspace = anlg_db_app::ensure_cloudsync_workspace_binding(runtime.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
         VALUES ('session', ?, ?, 'Session')",
    )
    .bind(&local_workspace)
    .bind(&local_workspace)
    .execute(runtime.pool())
    .await
    .unwrap();

    assert!(
        runtime
            .bind_cloudsync_account("user-a".to_string())
            .await
            .unwrap()
    );

    assert_eq!(
        runtime
            .configure_cloudsync_token(
                "managed-database-id".to_string(),
                "token".to_string(),
                "user-a".to_string(),
                witness,
            )
            .await
            .unwrap(),
        CloudsyncTokenConfigurationResult::Configured
    );
    runtime.suspend_cloudsync().await.unwrap();

    let session: (String, String) =
        sqlx::query_as("SELECT workspace_id, owner_user_id FROM sessions WHERE id = 'session'")
            .fetch_one(runtime.pool())
            .await
            .unwrap();
    let binding: (String, String) = sqlx::query_as(
        "SELECT json_extract(value_json, '$.workspace_id'),
                json_extract(value_json, '$.account_user_id')
         FROM app_settings WHERE id = 'cloudsync_workspace_binding'",
    )
    .fetch_one(runtime.pool())
    .await
    .unwrap();

    assert_eq!(session, ("user-a".to_string(), "user-a".to_string()));
    assert_eq!(binding, ("user-a".to_string(), "user-a".to_string()));
}

#[tokio::test]
async fn token_refresh_restarts_pending_full_resync_with_new_credentials() {
    let (_dir, runtime) = setup_enabled_cloudsync_runtime().await;
    let (_witness_server, witness) = setup_witness("user-a").await;
    let local_workspace = anlg_db_app::ensure_cloudsync_workspace_binding(runtime.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
         VALUES ('session', ?, ?, 'Session')",
    )
    .bind(&local_workspace)
    .bind(&local_workspace)
    .execute(runtime.pool())
    .await
    .unwrap();
    assert!(
        runtime
            .bind_cloudsync_account("user-a".to_string())
            .await
            .unwrap()
    );
    let projection = anlg_db_app::CloudsyncWorkspaceProjection {
        account_user_id: "user-a".to_string(),
        personal_workspace_id: "user-a".to_string(),
        workspaces: vec![anlg_db_app::CloudsyncWorkspaceProjectionEntry {
            id: "user-a".to_string(),
            owner_user_id: "user-a".to_string(),
            kind: "personal".to_string(),
            name: "Personal".to_string(),
            membership_id: "membership-personal".to_string(),
            role: "owner".to_string(),
            membership_created_at: "2026-07-01T01:00:00Z".to_string(),
            membership_updated_at: "2026-07-16T01:00:00Z".to_string(),
            created_at: "2026-07-01T00:00:00Z".to_string(),
            updated_at: "2026-07-16T00:00:00Z".to_string(),
        }],
    };

    assert_eq!(
        runtime
            .configure_cloudsync_token_with_projection(
                "managed-database-id".to_string(),
                "initial-token".to_string(),
                "user-a".to_string(),
                Some(projection.clone()),
                witness.clone(),
            )
            .await
            .unwrap(),
        CloudsyncTokenConfigurationResult::Configured
    );
    let (generation, auth) = runtime.cloudsync_full_resync_task_snapshot().await.unwrap();
    assert_eq!(
        auth,
        anlg_db_core::CloudsyncAuth::Token {
            token: "initial-token".to_string(),
        }
    );

    assert_eq!(
        runtime
            .configure_cloudsync_token_with_projection(
                "managed-database-id".to_string(),
                "refreshed-token".to_string(),
                "user-a".to_string(),
                Some(projection),
                witness,
            )
            .await
            .unwrap(),
        CloudsyncTokenConfigurationResult::Configured
    );
    let (refreshed_generation, refreshed_auth) =
        runtime.cloudsync_full_resync_task_snapshot().await.unwrap();
    assert_eq!(refreshed_generation, generation);
    assert_eq!(
        refreshed_auth,
        anlg_db_core::CloudsyncAuth::Token {
            token: "refreshed-token".to_string(),
        }
    );

    runtime.suspend_cloudsync().await.unwrap();
}

#[tokio::test]
async fn token_configuration_projects_server_workspaces_after_account_claim() {
    let (_dir, runtime) = setup_enabled_cloudsync_runtime().await;
    let (_witness_server, witness) = setup_witness("user-a").await;
    assert!(
        runtime
            .bind_cloudsync_account("user-a".to_string())
            .await
            .unwrap()
    );

    let projection = anlg_db_app::CloudsyncWorkspaceProjection {
        account_user_id: "user-a".to_string(),
        personal_workspace_id: "user-a".to_string(),
        workspaces: vec![
            anlg_db_app::CloudsyncWorkspaceProjectionEntry {
                id: "user-a".to_string(),
                owner_user_id: "user-a".to_string(),
                kind: "personal".to_string(),
                name: "Personal".to_string(),
                membership_id: "membership-personal".to_string(),
                role: "owner".to_string(),
                membership_created_at: "2026-07-01T01:00:00Z".to_string(),
                membership_updated_at: "2026-07-16T01:00:00Z".to_string(),
                created_at: "2026-07-01T00:00:00Z".to_string(),
                updated_at: "2026-07-16T00:00:00Z".to_string(),
            },
            anlg_db_app::CloudsyncWorkspaceProjectionEntry {
                id: "workspace-shared".to_string(),
                owner_user_id: "user-b".to_string(),
                kind: "shared".to_string(),
                name: "Shared".to_string(),
                membership_id: "membership-shared".to_string(),
                role: "member".to_string(),
                membership_created_at: "2026-07-02T01:00:00Z".to_string(),
                membership_updated_at: "2026-07-15T01:00:00Z".to_string(),
                created_at: "2026-07-02T00:00:00Z".to_string(),
                updated_at: "2026-07-15T00:00:00Z".to_string(),
            },
        ],
    };

    assert_eq!(
        runtime
            .configure_cloudsync_token_with_projection(
                "managed-database-id".to_string(),
                "token".to_string(),
                "user-a".to_string(),
                Some(projection),
                witness,
            )
            .await
            .unwrap(),
        CloudsyncTokenConfigurationResult::Configured
    );
    runtime.suspend_cloudsync().await.unwrap();

    let workspaces: Vec<(String, String)> =
        sqlx::query_as("SELECT id, name FROM workspaces ORDER BY id")
            .fetch_all(runtime.pool())
            .await
            .unwrap();
    let memberships: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT workspace_id, user_id, role FROM workspace_memberships ORDER BY workspace_id",
    )
    .fetch_all(runtime.pool())
    .await
    .unwrap();
    let writable_workspace_ids: Vec<String> = sqlx::query_scalar(
        "SELECT allowed_workspace_id
         FROM cloudsync_writable_workspaces
         ORDER BY allowed_workspace_id",
    )
    .fetch_all(runtime.pool())
    .await
    .unwrap();

    assert_eq!(
        workspaces,
        vec![
            ("user-a".to_string(), "Personal".to_string()),
            ("workspace-shared".to_string(), "Shared".to_string()),
        ]
    );
    assert_eq!(
        memberships,
        vec![
            (
                "user-a".to_string(),
                "user-a".to_string(),
                "owner".to_string(),
            ),
            (
                "workspace-shared".to_string(),
                "user-a".to_string(),
                "member".to_string(),
            ),
        ]
    );
    assert_eq!(writable_workspace_ids, vec!["user-a".to_string()]);
    assert!(
        anlg_db_app::cloudsync_write_filter_installed(runtime.pool(), "user-a")
            .await
            .unwrap()
    );
    assert!(runtime.cloudsync_write_filters_match().await.unwrap());
}

#[tokio::test]
async fn token_configuration_account_mismatch_preserves_workspace_projection() {
    let (_dir, runtime) = setup_enabled_cloudsync_runtime().await;
    assert!(
        runtime
            .bind_cloudsync_account("user-a".to_string())
            .await
            .unwrap()
    );
    anlg_db_app::replace_cloudsync_workspace_projection(
        runtime.pool(),
        &anlg_db_app::CloudsyncWorkspaceProjection {
            account_user_id: "user-a".to_string(),
            personal_workspace_id: "user-a".to_string(),
            workspaces: vec![anlg_db_app::CloudsyncWorkspaceProjectionEntry {
                id: "user-a".to_string(),
                owner_user_id: "user-a".to_string(),
                kind: "personal".to_string(),
                name: "Existing".to_string(),
                membership_id: "membership-existing".to_string(),
                role: "owner".to_string(),
                membership_created_at: "2026-07-01T01:00:00Z".to_string(),
                membership_updated_at: "2026-07-16T01:00:00Z".to_string(),
                created_at: "2026-07-01T00:00:00Z".to_string(),
                updated_at: "2026-07-16T00:00:00Z".to_string(),
            }],
        },
    )
    .await
    .unwrap();

    let error = runtime
        .configure_cloudsync_token_with_projection(
            "managed-database-id".to_string(),
            "token".to_string(),
            "user-a".to_string(),
            Some(anlg_db_app::CloudsyncWorkspaceProjection {
                account_user_id: "user-b".to_string(),
                personal_workspace_id: "user-b".to_string(),
                workspaces: vec![anlg_db_app::CloudsyncWorkspaceProjectionEntry {
                    id: "user-b".to_string(),
                    owner_user_id: "user-b".to_string(),
                    kind: "personal".to_string(),
                    name: "Replacement".to_string(),
                    membership_id: "membership-replacement".to_string(),
                    role: "owner".to_string(),
                    membership_created_at: "2026-07-01T01:00:00Z".to_string(),
                    membership_updated_at: "2026-07-16T01:00:00Z".to_string(),
                    created_at: "2026-07-01T00:00:00Z".to_string(),
                    updated_at: "2026-07-16T00:00:00Z".to_string(),
                }],
            }),
            unreachable_witness("user-a"),
        )
        .await
        .unwrap_err();

    let workspaces: Vec<(String, String)> =
        sqlx::query_as("SELECT id, name FROM workspaces ORDER BY id")
            .fetch_all(runtime.pool())
            .await
            .unwrap();
    assert!(matches!(
        error,
        crate::Error::CloudsyncWorkspace(
            anlg_db_app::CloudsyncWorkspaceError::InvalidWorkspaceProjection
        )
    ));
    assert_eq!(
        workspaces,
        vec![("user-a".to_string(), "Existing".to_string())]
    );
}

#[tokio::test]
async fn token_configuration_rejects_foreign_workspace_rows() {
    let (_dir, runtime) = setup_enabled_cloudsync_runtime().await;
    sqlx::query("INSERT INTO sessions (id, workspace_id) VALUES ('session', 'other-user')")
        .execute(runtime.pool())
        .await
        .unwrap();

    assert!(
        runtime
            .bind_cloudsync_account("user-a".to_string())
            .await
            .unwrap()
    );

    let result = runtime
        .configure_cloudsync_token(
            "managed-database-id".to_string(),
            "token".to_string(),
            "user-a".to_string(),
            unreachable_witness("user-a"),
        )
        .await
        .unwrap();

    assert_eq!(result, CloudsyncTokenConfigurationResult::AccountMismatch);
    assert_eq!(
        runtime.cloudsync_status().await.unwrap()["configured"],
        false
    );
}

#[tokio::test]
async fn token_configuration_rejects_an_invalid_workspace_binding() {
    let (_dir, runtime) = setup_enabled_cloudsync_runtime().await;
    assert!(
        runtime
            .bind_cloudsync_account("user-a".to_string())
            .await
            .unwrap()
    );
    sqlx::query(
        "UPDATE app_settings
         SET value_json = 'not-json'
         WHERE id = 'cloudsync_workspace_binding'",
    )
    .execute(runtime.pool())
    .await
    .unwrap();

    let result = runtime
        .configure_cloudsync_token(
            "managed-database-id".to_string(),
            "token".to_string(),
            "user-a".to_string(),
            unreachable_witness("user-a"),
        )
        .await
        .unwrap();

    assert_eq!(result, CloudsyncTokenConfigurationResult::AccountMismatch);
    assert_eq!(
        runtime.cloudsync_status().await.unwrap()["configured"],
        false
    );
}

#[tokio::test]
async fn same_account_binding_is_idempotent() {
    let (_dir, runtime) = setup_runtime().await;
    assert!(
        runtime
            .bind_cloudsync_account("user-a".to_string())
            .await
            .unwrap()
    );

    assert!(
        runtime
            .bind_cloudsync_account("user-a".to_string())
            .await
            .unwrap()
    );

    assert_eq!(
        runtime.cloudsync_status().await.unwrap()["configured"],
        false
    );
}

#[tokio::test]
async fn account_switch_is_rejected_and_leaves_cloudsync_suspended() {
    let (_dir, runtime) = setup_runtime().await;
    assert!(
        runtime
            .bind_cloudsync_account("user-a".to_string())
            .await
            .unwrap()
    );

    let bound = runtime
        .bind_cloudsync_account("user-b".to_string())
        .await
        .unwrap();

    assert!(!bound);
    assert_eq!(
        runtime.cloudsync_status().await.unwrap()["configured"],
        false
    );
}

#[tokio::test]
async fn invalid_account_binding_remains_an_error() {
    let (_dir, runtime) = setup_runtime().await;
    assert!(
        runtime
            .bind_cloudsync_account("user-a".to_string())
            .await
            .unwrap()
    );

    assert!(
        runtime
            .bind_cloudsync_account(" ".to_string())
            .await
            .is_err()
    );
    assert_eq!(
        runtime.cloudsync_status().await.unwrap()["configured"],
        false
    );
}
