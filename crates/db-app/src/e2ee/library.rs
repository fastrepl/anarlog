use serde_json::Value;
use sqlx::{Executor, Sqlite, Transaction};

pub(super) struct LibraryIdentity {
    pub local_workspace_id: String,
    remote_workspace_id: String,
    connected: bool,
    pub active: bool,
}

pub(super) async fn queue_other_connections(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: &str,
    table: &str,
    row_id: &str,
    edited_at_ms: i64,
) -> sqlx::Result<()> {
    let identity = LibraryIdentity::load(&mut **transaction, workspace_id).await?;
    let local_row_id = identity.local_row_id(table, row_id);
    sqlx::query(
        "INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id, dirtied_at_ms)
         SELECT account_user_id, ?,
           CASE WHEN ? = 'humans' AND ? = library_workspace_id THEN account_user_id ELSE ? END, ?
         FROM local_library_connections
         WHERE library_workspace_id = ? AND account_user_id != ?
         ON CONFLICT(workspace_id, table_name, row_id) DO UPDATE SET
           generation = e2ee_dirty_rows.generation + 1,
           dirtied_at_ms = MAX(e2ee_dirty_rows.dirtied_at_ms, excluded.dirtied_at_ms)",
    )
    .bind(table)
    .bind(table)
    .bind(local_row_id)
    .bind(local_row_id)
    .bind(edited_at_ms)
    .bind(&identity.local_workspace_id)
    .bind(workspace_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

impl LibraryIdentity {
    pub async fn remote_value<'a>(
        &self,
        executor: impl Executor<'a, Database = Sqlite>,
        table: &str,
        row_id: &str,
        field: &str,
        value: Value,
    ) -> sqlx::Result<Value> {
        if table == "session_attachments"
            && matches!(field, "cloud_object_key" | "cloud_sync_enabled")
        {
            let state: Option<(String, i64)> = sqlx::query_as(
                "SELECT cloud_object_key, cloud_sync_enabled FROM local_library_attachment_state
                 WHERE account_user_id = ? AND attachment_id = ?",
            )
            .bind(&self.remote_workspace_id)
            .bind(row_id)
            .fetch_optional(executor)
            .await?;
            if let Some((key, enabled)) = state {
                return Ok(if field == "cloud_object_key" {
                    Value::String(key)
                } else {
                    Value::from(enabled)
                });
            }
            if self.connected {
                return Ok(if field == "cloud_object_key" {
                    Value::String(String::new())
                } else {
                    Value::from(0)
                });
            }
        }
        Ok(self.to_remote(table, field, value))
    }

    pub async fn load<'a>(
        executor: impl Executor<'a, Database = Sqlite>,
        remote_workspace_id: &str,
    ) -> sqlx::Result<Self> {
        let local_workspace_id = sqlx::query_as::<_, (String, bool)>(
            "SELECT library_workspace_id, active FROM local_library_connections
             WHERE account_user_id = ?",
        )
        .bind(remote_workspace_id)
        .fetch_optional(executor)
        .await?;
        let connected = local_workspace_id.is_some();
        Ok(Self {
            connected,
            active: local_workspace_id
                .as_ref()
                .is_none_or(|(_, active)| *active),
            local_workspace_id: local_workspace_id
                .map(|(id, _)| id)
                .unwrap_or_else(|| remote_workspace_id.to_string()),
            remote_workspace_id: remote_workspace_id.to_string(),
        })
    }

    pub fn local_row_id<'a>(&'a self, table: &str, row_id: &'a str) -> &'a str {
        if table == "humans" && row_id == self.remote_workspace_id {
            &self.local_workspace_id
        } else {
            row_id
        }
    }

    pub fn to_remote(&self, table: &str, field: &str, value: Value) -> Value {
        self.map_value(
            table,
            field,
            value,
            &self.local_workspace_id,
            &self.remote_workspace_id,
        )
    }

    pub fn to_local(&self, table: &str, field: &str, value: Value) -> Value {
        self.map_value(
            table,
            field,
            value,
            &self.remote_workspace_id,
            &self.local_workspace_id,
        )
    }

    fn map_value(&self, table: &str, field: &str, value: Value, from: &str, to: &str) -> Value {
        let identity = matches!(field, "owner_user_id" | "created_by" | "updated_by")
            || (table == "session_participants" && field == "human_id")
            || (table == "action_items" && field == "assignee_human_id");
        if identity && value.as_str() == Some(from) {
            Value::String(to.to_string())
        } else {
            value
        }
    }
}
