use legacy_db_core::SqlTable;

use super::{ExtensionMapping, UserDatabase};

impl UserDatabase {
    pub async fn upsert_extension_mapping(
        &self,
        mapping: ExtensionMapping,
    ) -> Result<ExtensionMapping, crate::Error> {
        let conn = self.conn()?;

        let sql = format!(
            "INSERT OR REPLACE INTO {} (
                id,
                user_id,
                extension_id,
                config,
                position
            ) VALUES (?, ?, ?, ?, ?) RETURNING *",
            ExtensionMapping::sql_table()
        );

        let params = vec![
            libsql::Value::Text(mapping.id),
            libsql::Value::Text(mapping.user_id),
            libsql::Value::Text(mapping.extension_id),
            libsql::Value::Text(serde_json::to_string(&mapping.config)?),
            libsql::Value::Text(serde_json::to_string(&mapping.widgets)?),
        ];

        let mut rows = conn.query(&sql, params).await?;
        let row = rows.next().await?.unwrap();
        let item = ExtensionMapping::from_row(&row)?;
        Ok(item)
    }

    pub async fn get_extension_mapping(
        &self,
        user_id: impl Into<String>,
        extension_id: impl Into<String>,
    ) -> Result<Option<ExtensionMapping>, crate::Error> {
        let conn = self.conn()?;

        let sql = format!(
            "SELECT * FROM {} WHERE user_id = ? AND extension_id = ?",
            ExtensionMapping::sql_table()
        );

        let mut rows = conn
            .query(&sql, vec![user_id.into(), extension_id.into()])
            .await?;

        match rows.next().await? {
            None => Ok(None),
            Some(row) => {
                let item = ExtensionMapping::from_row(&row)?;
                Ok(Some(item))
            }
        }
    }

    pub async fn list_extension_mappings(
        &self,
        user_id: impl Into<String>,
    ) -> Result<Vec<ExtensionMapping>, crate::Error> {
        let conn = self.conn()?;

        let sql = format!(
            "SELECT * FROM {} WHERE user_id = ?",
            ExtensionMapping::sql_table()
        );

        let mut rows = conn.query(&sql, vec![user_id.into()]).await?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().await? {
            let item = ExtensionMapping::from_row(&row)?;
            items.push(item);
        }
        Ok(items)
    }
}
