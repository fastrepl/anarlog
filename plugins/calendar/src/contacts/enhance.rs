use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use crate::contacts::run_write;
use crate::storage::transaction_utils::js_iso8601_timestamp;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ApplyContactEnhancementRequest {
    pub human_id: String,
    pub owner_user_id: String,
    pub create_if_missing: bool,
    pub name: Option<String>,
    pub email: Option<String>,
    pub company_name: Option<String>,
    pub job_title: Option<String>,
    pub phone: Option<String>,
    pub linkedin_username: Option<String>,
}

pub async fn apply_contact_enhancement(
    pool: &SqlitePool,
    request: ApplyContactEnhancementRequest,
) -> Result<(), String> {
    if !request.create_if_missing
        && request.name.is_none()
        && request.email.is_none()
        && request.job_title.is_none()
        && request.phone.is_none()
        && request.linkedin_username.is_none()
        && request
            .company_name
            .as_deref()
            .unwrap_or_default()
            .is_empty()
    {
        return Ok(());
    }

    let now = js_iso8601_timestamp();
    run_write(pool, |conn| {
        Box::pin(async move {
            if request.create_if_missing {
                anlg_db_app::upsert_revived_human(
                    conn,
                    &request.human_id,
                    &request.owner_user_id,
                    request.name.as_deref().unwrap_or_default(),
                    request.email.as_deref().unwrap_or_default(),
                    &now,
                )
                .await
                .map_err(|error| error.to_string())?;
            }

            let company_name = request.company_name.as_deref().unwrap_or_default();
            if !company_name.is_empty() {
                anlg_db_app::insert_organization_by_name_if_missing(
                    conn,
                    &uuid::Uuid::new_v4().to_string(),
                    &request.owner_user_id,
                    company_name,
                    &now,
                )
                .await
                .map_err(|error| error.to_string())?;
            }

            let mut assignments: Vec<(&'static str, String)> = Vec::new();
            if let Some(name) = &request.name {
                assignments.push(("name = ?", name.clone()));
            }
            if let Some(email) = &request.email {
                assignments.push(("email = ?", email.clone()));
            }
            if let Some(job_title) = &request.job_title {
                assignments.push(("job_title = ?", job_title.clone()));
            }
            if let Some(phone) = &request.phone {
                assignments.push(("phone = ?", phone.clone()));
            }
            if let Some(linkedin_username) = &request.linkedin_username {
                assignments.push(("linkedin_username = ?", linkedin_username.clone()));
            }
            if !company_name.is_empty() {
                assignments.push((
                    "organization_id = CASE
          WHEN organization_id = '' THEN COALESCE((
            SELECT id
            FROM organizations
            WHERE lower(name) = lower(?) AND deleted_at IS NULL
            ORDER BY created_at, id
            LIMIT 1
          ), organization_id)
          ELSE organization_id
        END",
                    company_name.to_string(),
                ));
            }

            if !assignments.is_empty() {
                anlg_db_app::update_contact_fields(
                    conn,
                    "humans",
                    &assignments,
                    &now,
                    &request.human_id,
                )
                .await
                .map_err(|error| error.to_string())?;
            }
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

    fn base_request() -> ApplyContactEnhancementRequest {
        ApplyContactEnhancementRequest {
            human_id: "human-1".to_string(),
            owner_user_id: "user-1".to_string(),
            create_if_missing: false,
            name: None,
            email: None,
            company_name: None,
            job_title: None,
            phone: None,
            linkedin_username: None,
        }
    }

    #[tokio::test]
    async fn creates_the_organization_once_case_insensitively_and_fills_empty_org() {
        let db = test_db().await;
        sqlx::query(
            "INSERT INTO humans (id, owner_user_id, name) VALUES ('human-1', 'user-1', 'Ada')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let mut request = base_request();
        request.company_name = Some("Example".to_string());
        request.name = Some("Alice Kim".to_string());
        apply_contact_enhancement(db.pool(), request).await.unwrap();

        let org_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM organizations WHERE lower(name) = 'example'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(org_count, 1);

        let (name, org_id): (String, String) =
            sqlx::query_as("SELECT name, organization_id FROM humans WHERE id = 'human-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(name, "Alice Kim");
        assert!(!org_id.is_empty());

        // Second call does not create a duplicate org nor overwrite the assignment.
        sqlx::query("INSERT INTO humans (id, owner_user_id, name, organization_id) VALUES ('human-2', 'user-1', 'Bob', 'org-set')")
            .execute(db.pool())
            .await
            .unwrap();
        let mut request = base_request();
        request.human_id = "human-2".to_string();
        request.company_name = Some("EXAMPLE".to_string());
        apply_contact_enhancement(db.pool(), request).await.unwrap();

        let org_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM organizations WHERE lower(name) = 'example'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(org_count, 1);
        let org_id: String =
            sqlx::query_scalar("SELECT organization_id FROM humans WHERE id = 'human-2'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(org_id, "org-set");
    }

    #[tokio::test]
    async fn create_if_missing_revives_a_soft_deleted_human() {
        let db = test_db().await;
        sqlx::query(
            "INSERT INTO humans (id, owner_user_id, name, email, deleted_at)
             VALUES ('human-1', 'user-1', 'Old', 'old@example.com', '2026-01-01T00:00:00.000Z')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let mut request = base_request();
        request.create_if_missing = true;
        request.name = Some("New".to_string());
        apply_contact_enhancement(db.pool(), request).await.unwrap();

        let (deleted, name): (Option<String>, String) =
            sqlx::query_as("SELECT deleted_at, name FROM humans WHERE id = 'human-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert!(deleted.is_none());
        assert_eq!(name, "New");
    }

    #[tokio::test]
    async fn no_changes_returns_ok_without_writing() {
        let db = test_db().await;
        apply_contact_enhancement(db.pool(), base_request())
            .await
            .unwrap();

        let mut request = base_request();
        request.company_name = Some(String::new());
        apply_contact_enhancement(db.pool(), request).await.unwrap();
    }

    #[tokio::test]
    async fn a_mid_transaction_failure_rolls_back_the_org_insert() {
        let db = test_db().await;
        sqlx::query(
            "INSERT INTO humans (id, owner_user_id, name) VALUES ('human-1', 'user-1', 'Ada')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "CREATE TRIGGER block_human_updates BEFORE UPDATE ON humans
             BEGIN SELECT RAISE(ABORT, 'boom'); END",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let mut request = base_request();
        request.company_name = Some("Example".to_string());
        apply_contact_enhancement(db.pool(), request)
            .await
            .unwrap_err();

        let org_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM organizations WHERE name = 'Example'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(org_count, 0);
    }
}
