use legacy_db_core::SqlTable;

use super::{Human, ListOrganizationFilter, Organization, UserDatabase};

impl UserDatabase {
    pub async fn upsert_organization(
        &self,
        organization: Organization,
    ) -> Result<Organization, crate::Error> {
        let conn = self.conn()?;

        let sql = format!(
            "INSERT INTO {} (
                id,
                name,
                description
            )  VALUES (?, ?, ?) 
            ON CONFLICT (id) DO UPDATE SET 
                name = excluded.name,
                description = excluded.description
            RETURNING *",
            Organization::sql_table()
        );
        let params = (organization.id, organization.name, organization.description);

        let mut rows = conn.query(&sql, params).await?;
        let row = rows.next().await?.unwrap();
        let organization: Organization = libsql::de::from_row(&row)?;
        Ok(organization)
    }

    pub async fn delete_organization(&self, id: impl Into<String>) -> Result<(), crate::Error> {
        let conn = self.conn()?;

        let sql = format!("DELETE FROM {} WHERE id = ?", Organization::sql_table());
        conn.query(&sql, vec![id.into()]).await?;
        Ok(())
    }

    pub async fn list_organizations(
        &self,
        filter: Option<ListOrganizationFilter>,
    ) -> Result<Vec<Organization>, crate::Error> {
        let conn = self.conn()?;

        let mut rows = match &filter {
            None => {
                let sql = format!("SELECT * FROM {}", Organization::sql_table());
                conn.query(&sql, ()).await?
            }
            Some(ListOrganizationFilter::Search((max, q))) => {
                let sql = format!(
                    "SELECT * FROM {} WHERE name LIKE ? LIMIT ?",
                    Organization::sql_table()
                );
                let params = (format!("%{}%", q), max.to_string());
                conn.query(&sql, params).await?
            }
        };

        let mut items = Vec::new();
        while let Some(row) = rows.next().await? {
            let item: Organization = libsql::de::from_row(&row)?;
            items.push(item);
        }
        Ok(items)
    }

    pub async fn list_organization_members(
        &self,
        organization_id: impl Into<String>,
    ) -> Result<Vec<Human>, crate::Error> {
        let conn = self.conn()?;

        let sql = format!(
            "SELECT * FROM {} WHERE organization_id = ?",
            Human::sql_table()
        );
        let mut rows = conn.query(&sql, vec![organization_id.into()]).await?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().await? {
            let item: Human = libsql::de::from_row(&row)?;
            items.push(item);
        }
        Ok(items)
    }

    pub async fn get_organization(
        &self,
        id: impl Into<String>,
    ) -> Result<Option<Organization>, crate::Error> {
        let conn = self.conn()?;

        let sql = format!("SELECT * FROM {} WHERE id = ?", Organization::sql_table());
        let mut rows = conn.query(&sql, vec![id.into()]).await?;

        match rows.next().await? {
            None => Ok(None),
            Some(row) => {
                let item: Organization = libsql::de::from_row(&row)?;
                Ok(Some(item))
            }
        }
    }

    pub async fn get_organization_by_user_id(
        &self,
        id: impl Into<String>,
    ) -> Result<Option<Organization>, crate::Error> {
        let conn = self.conn()?;
        let id = id.into();

        let sql = format!(
            "SELECT o.* FROM {} o
                INNER JOIN {} u ON u.organization_id = o.id
                WHERE u.id = ?",
            Organization::sql_table(),
            Human::sql_table()
        );
        let mut rows = conn.query(&sql, vec![id]).await?;

        match rows.next().await? {
            None => Ok(None),
            Some(row) => {
                let org: Organization = libsql::de::from_row(&row)?;
                Ok(Some(org))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::setup_db;

    #[tokio::test]
    async fn test_list_organizations() {
        let db = setup_db().await;

        let organizations = db.list_organizations(None).await.unwrap();
        assert!(organizations.len() == 0);
    }
}
