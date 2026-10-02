use anlg_db_app::AvatarChange;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use crate::contacts::run_write;
use crate::storage::transaction_utils::js_iso8601_timestamp;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CreateHumanRequest {
    pub id: String,
    pub owner_user_id: String,
    pub name: String,
    pub email: String,
}

pub async fn create_human(pool: &SqlitePool, request: CreateHumanRequest) -> Result<(), String> {
    let now = js_iso8601_timestamp();
    run_write(pool, |conn| {
        Box::pin(async move {
            anlg_db_app::create_human(
                conn,
                &request.id,
                &request.owner_user_id,
                &request.name,
                &request.email,
                &now,
            )
            .await
            .map_err(|error| error.to_string())?;
            Ok(())
        })
    })
    .await
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CreateOrganizationRequest {
    pub id: String,
    pub owner_user_id: String,
    pub name: String,
}

pub async fn create_organization(
    pool: &SqlitePool,
    request: CreateOrganizationRequest,
) -> Result<(), String> {
    let now = js_iso8601_timestamp();
    run_write(pool, |conn| {
        Box::pin(async move {
            anlg_db_app::create_organization(
                conn,
                &request.id,
                &request.owner_user_id,
                &request.name,
                &now,
            )
            .await
            .map_err(|error| error.to_string())?;
            Ok(())
        })
    })
    .await
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SavePersonalContactRequest {
    pub human_id: String,
    pub name: String,
    pub email: String,
    pub phone: String,
    pub job_title: String,
    pub linkedin_username: String,
    pub memo: String,
    pub organization_id: String,
    pub avatar_data_url: Option<String>,
    pub remove_avatar: bool,
}

pub async fn save_personal_contact(
    pool: &SqlitePool,
    request: SavePersonalContactRequest,
) -> Result<(), String> {
    let now = js_iso8601_timestamp();
    let avatar = match (&request.avatar_data_url, request.remove_avatar) {
        (Some(value), _) => AvatarChange::Set(value.clone()),
        (None, true) => AvatarChange::Remove,
        (None, false) => AvatarChange::Keep,
    };
    run_write(pool, |conn| {
        Box::pin(async move {
            anlg_db_app::upsert_personal_contact(
                conn,
                &request.human_id,
                &request.name,
                &request.email,
                &request.phone,
                &request.job_title,
                &request.linkedin_username,
                &request.memo,
                &request.organization_id,
                &avatar,
                &now,
            )
            .await
            .map_err(|error| error.to_string())?;
            Ok(())
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_db_core::Db;

    async fn test_db() -> Db {
        let db = Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        db
    }

    #[tokio::test]
    async fn create_human_resolves_owner_and_workspace_from_active_library() {
        let db = test_db().await;
        sqlx::query(
            "INSERT INTO local_library_connections (account_user_id, library_workspace_id, active)
             VALUES ('acct-1', 'library-ws', 1)",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "UPDATE app_settings SET value_json = ? WHERE id = 'cloudsync_workspace_binding'",
        )
        .bind(r#"{"workspace_id":"bound-ws"}"#)
        .execute(db.pool())
        .await
        .unwrap();

        create_human(
            db.pool(),
            CreateHumanRequest {
                id: "human-1".to_string(),
                owner_user_id: "user-1".to_string(),
                name: "Ada".to_string(),
                email: "ada@example.com".to_string(),
            },
        )
        .await
        .unwrap();

        let (owner, workspace): (String, String) =
            sqlx::query_as("SELECT owner_user_id, workspace_id FROM humans WHERE id = 'human-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(owner, "library-ws");
        assert_eq!(workspace, "bound-ws");
    }

    #[tokio::test]
    async fn create_organization_falls_back_to_workspace_binding_then_default() {
        let db = test_db().await;
        sqlx::query(
            "UPDATE app_settings SET value_json = ? WHERE id = 'cloudsync_workspace_binding'",
        )
        .bind(r#"{"workspace_id":"bound-ws"}"#)
        .execute(db.pool())
        .await
        .unwrap();

        create_organization(
            db.pool(),
            CreateOrganizationRequest {
                id: "org-1".to_string(),
                owner_user_id: anlg_db_app::DEFAULT_USER_ID.to_string(),
                name: "Acme".to_string(),
            },
        )
        .await
        .unwrap();

        let (owner, workspace): (String, String) = sqlx::query_as(
            "SELECT owner_user_id, workspace_id FROM organizations WHERE id = 'org-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(owner, "bound-ws");
        assert_eq!(workspace, "bound-ws");
    }

    #[tokio::test]
    async fn create_human_uses_the_explicit_owner_with_binding_workspace() {
        let db = test_db().await;
        sqlx::query(
            "UPDATE app_settings SET value_json = ? WHERE id = 'cloudsync_workspace_binding'",
        )
        .bind(r#"{"workspace_id":"bound-ws"}"#)
        .execute(db.pool())
        .await
        .unwrap();

        create_human(
            db.pool(),
            CreateHumanRequest {
                id: "human-1".to_string(),
                owner_user_id: "user-1".to_string(),
                name: "Ada".to_string(),
                email: String::new(),
            },
        )
        .await
        .unwrap();

        let (owner, workspace): (String, String) =
            sqlx::query_as("SELECT owner_user_id, workspace_id FROM humans WHERE id = 'human-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(owner, "user-1");
        assert_eq!(workspace, "bound-ws");
    }

    fn personal_request(avatar: Option<String>, remove_avatar: bool) -> SavePersonalContactRequest {
        SavePersonalContactRequest {
            human_id: "self-1".to_string(),
            name: "Ada".to_string(),
            email: "ada@example.com".to_string(),
            phone: "123".to_string(),
            job_title: "Engineer".to_string(),
            linkedin_username: "ada".to_string(),
            memo: "notes".to_string(),
            organization_id: "org-1".to_string(),
            avatar_data_url: avatar,
            remove_avatar,
        }
    }

    #[tokio::test]
    async fn save_personal_contact_inserts_and_updates_without_losing_avatar() {
        let db = test_db().await;
        save_personal_contact(
            db.pool(),
            personal_request(Some("data:image/png;base64,x".into()), false),
        )
        .await
        .unwrap();

        let metadata: String =
            sqlx::query_scalar("SELECT metadata_json FROM humans WHERE id = 'self-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&metadata).unwrap(),
            serde_json::json!({"avatarDataUrl": "data:image/png;base64,x"})
        );

        // Update without touching the avatar; update also revives deleted rows.
        sqlx::query(
            "UPDATE humans SET deleted_at = '2026-01-01T00:00:00.000Z',
             metadata_json = json_set(metadata_json, '$.other', 'keep') WHERE id = 'self-1'",
        )
        .execute(db.pool())
        .await
        .unwrap();
        save_personal_contact(db.pool(), personal_request(None, false))
            .await
            .unwrap();

        let (deleted, metadata): (Option<String>, String) =
            sqlx::query_as("SELECT deleted_at, metadata_json FROM humans WHERE id = 'self-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert!(deleted.is_none());
        let metadata: serde_json::Value = serde_json::from_str(&metadata).unwrap();
        assert_eq!(metadata["other"], "keep");
        assert_eq!(metadata["avatarDataUrl"], "data:image/png;base64,x");

        save_personal_contact(db.pool(), personal_request(None, true))
            .await
            .unwrap();
        let metadata: String =
            sqlx::query_scalar("SELECT metadata_json FROM humans WHERE id = 'self-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let metadata: serde_json::Value = serde_json::from_str(&metadata).unwrap();
        assert!(metadata.get("avatarDataUrl").is_none());
        assert_eq!(metadata["other"], "keep");
    }

    #[tokio::test]
    async fn save_personal_contact_repairs_invalid_metadata_json() {
        let db = test_db().await;
        sqlx::query(
            "INSERT INTO humans (id, owner_user_id, name, metadata_json)
             VALUES ('self-1', 'self-1', 'Ada', 'not-json')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        save_personal_contact(db.pool(), personal_request(None, false))
            .await
            .unwrap();

        let metadata: String =
            sqlx::query_scalar("SELECT metadata_json FROM humans WHERE id = 'self-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&metadata).unwrap(),
            serde_json::json!({})
        );
    }
}
