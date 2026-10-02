use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use crate::contacts::{ContactKind, PinnedContactEntry, run_write};
use crate::storage::transaction_utils::js_iso8601_timestamp;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ToggleContactPinRequest {
    pub kind: ContactKind,
    pub contact_id: String,
}

pub async fn toggle_contact_pin(
    pool: &SqlitePool,
    request: ToggleContactPinRequest,
) -> Result<(), String> {
    let now = js_iso8601_timestamp();
    run_write(pool, |conn| {
        Box::pin(async move {
            anlg_db_app::toggle_contact_pin(conn, request.kind.table(), &request.contact_id, &now)
                .await
                .map_err(|error| error.to_string())?;
            Ok(())
        })
    })
    .await
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ReorderPinnedContactsRequest {
    pub contacts: Vec<PinnedContactEntry>,
}

pub async fn reorder_pinned_contacts(
    pool: &SqlitePool,
    request: ReorderPinnedContactsRequest,
) -> Result<(), String> {
    if request.contacts.is_empty() {
        return Ok(());
    }
    let now = js_iso8601_timestamp();
    run_write(pool, |conn| {
        Box::pin(async move {
            for (index, contact) in request.contacts.iter().enumerate() {
                anlg_db_app::set_pinned_order(
                    conn,
                    contact.kind.table(),
                    index as i64,
                    &contact.id,
                    &now,
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

    #[tokio::test]
    async fn pinning_uses_max_order_across_both_tables_and_unpin_clears_it() {
        let db = test_db().await;
        sqlx::query(
            "INSERT INTO humans (id, owner_user_id, name, pinned, pin_order)
             VALUES ('human-1', 'u', 'A', 1, 3), ('human-2', 'u', 'B', 0, NULL);
             INSERT INTO organizations (id, owner_user_id, name, pinned, pin_order)
             VALUES ('org-1', 'u', 'O', 1, 7), ('org-2', 'u', 'P', 0, NULL)",
        )
        .execute(db.pool())
        .await
        .unwrap();

        // Pin an org: pin_order = max(3, 7) + 1 = 8.
        toggle_contact_pin(
            db.pool(),
            ToggleContactPinRequest {
                kind: ContactKind::Organization,
                contact_id: "org-2".to_string(),
            },
        )
        .await
        .unwrap();
        let (pinned, pin_order): (bool, Option<i64>) =
            sqlx::query_as("SELECT pinned, pin_order FROM organizations WHERE id = 'org-2'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert!(pinned);
        assert_eq!(pin_order, Some(8));

        // Unpin a human: pinned = 0, pin_order = NULL.
        toggle_contact_pin(
            db.pool(),
            ToggleContactPinRequest {
                kind: ContactKind::Human,
                contact_id: "human-1".to_string(),
            },
        )
        .await
        .unwrap();
        let (pinned, pin_order): (bool, Option<i64>) =
            sqlx::query_as("SELECT pinned, pin_order FROM humans WHERE id = 'human-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert!(!pinned);
        assert_eq!(pin_order, None);
    }

    #[tokio::test]
    async fn reorder_writes_indices_and_skips_unpinned_or_deleted_rows() {
        let db = test_db().await;
        sqlx::query(
            "INSERT INTO humans (id, owner_user_id, name, pinned, pin_order, deleted_at)
             VALUES ('human-1', 'u', 'A', 1, 0, NULL), ('human-2', 'u', 'B', 0, NULL, NULL),
                    ('human-3', 'u', 'C', 1, 0, '2026-01-01T00:00:00.000Z');
             INSERT INTO organizations (id, owner_user_id, name, pinned, pin_order)
             VALUES ('org-1', 'u', 'O', 1, 0)",
        )
        .execute(db.pool())
        .await
        .unwrap();

        reorder_pinned_contacts(
            db.pool(),
            ReorderPinnedContactsRequest {
                contacts: vec![
                    PinnedContactEntry {
                        kind: ContactKind::Organization,
                        id: "org-1".to_string(),
                    },
                    PinnedContactEntry {
                        kind: ContactKind::Human,
                        id: "human-1".to_string(),
                    },
                    PinnedContactEntry {
                        kind: ContactKind::Human,
                        id: "human-2".to_string(),
                    },
                    PinnedContactEntry {
                        kind: ContactKind::Human,
                        id: "human-3".to_string(),
                    },
                ],
            },
        )
        .await
        .unwrap();

        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT id, pin_order FROM humans WHERE id IN ('human-1', 'human-2', 'human-3') ORDER BY id",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(
            rows,
            vec![
                ("human-1".to_string(), 1),
                ("human-2".to_string(), 0),
                ("human-3".to_string(), 0),
            ]
        );
        let pin_order: i64 =
            sqlx::query_scalar("SELECT pin_order FROM organizations WHERE id = 'org-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(pin_order, 0);
    }
}
