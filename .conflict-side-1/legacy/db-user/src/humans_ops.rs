use legacy_db_core::SqlTable;

use super::{Human, ListHumanFilter, UserDatabase};

impl UserDatabase {
    pub async fn get_human(&self, id: impl Into<String>) -> Result<Option<Human>, crate::Error> {
        let conn = self.conn()?;

        let sql = format!("SELECT * FROM {} WHERE id = ?", Human::sql_table());
        let mut rows = conn.query(&sql, vec![id.into()]).await?;

        let row = rows.next().await?;
        Ok(row.map(|row| libsql::de::from_row(&row)).transpose()?)
    }

    pub async fn delete_human(&self, id: impl Into<String>) -> Result<(), crate::Error> {
        let conn = self.conn()?;

        let sql = format!("DELETE FROM {} WHERE id = ?", Human::sql_table());
        conn.query(&sql, vec![id.into()]).await?;
        Ok(())
    }

    pub async fn upsert_human(&self, human: Human) -> Result<Human, crate::Error> {
        let conn = self.conn()?;

        let sql = format!(
            "INSERT INTO {} (
                id,
                organization_id,
                is_user,
                full_name,
                email,
                job_title,
                linkedin_username
            ) VALUES (?, ?, ?, ?, ?, ?, ?) 
            ON CONFLICT (id) DO UPDATE SET
                organization_id = excluded.organization_id,
                is_user = excluded.is_user,
                full_name = excluded.full_name,
                email = excluded.email,
                job_title = excluded.job_title,
                linkedin_username = excluded.linkedin_username
            RETURNING *",
            Human::sql_table()
        );

        let params = (
            human.id,
            human.organization_id,
            human.is_user,
            human.full_name,
            human.email,
            human.job_title,
            human.linkedin_username,
        );

        let mut rows = conn.query(&sql, params).await?;
        let row = rows.next().await?.unwrap();
        let human: Human = libsql::de::from_row(&row)?;
        Ok(human)
    }

    pub async fn list_humans(
        &self,
        filter: Option<ListHumanFilter>,
    ) -> Result<Vec<Human>, crate::Error> {
        let conn = self.conn()?;

        let mut rows = match &filter {
            None => {
                let sql = format!("SELECT * FROM {}", Human::sql_table());
                conn.query(&sql, ()).await?
            }
            Some(ListHumanFilter::Search((max, q))) => {
                let sql = format!(
                    "SELECT * FROM {} WHERE full_name LIKE ? LIMIT ?",
                    Human::sql_table()
                );
                conn.query(&sql, vec![format!("%{}%", q), max.to_string()])
                    .await?
            }
        };

        let mut humans = Vec::new();
        while let Some(row) = rows.next().await? {
            let human: Human = libsql::de::from_row(&row)?;
            humans.push(human);
        }
        Ok(humans)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Human, tests::setup_db};

    #[tokio::test]
    async fn test_humans() {
        let db = setup_db().await;

        let humans = db.list_humans(None).await.unwrap();
        assert!(humans.len() == 0);

        let human = Human {
            full_name: Some("test".to_string()),
            ..Human::default()
        };

        let human = db.upsert_human(human).await.unwrap();
        assert_eq!(human.full_name, Some("test".to_string()));

        let humans = db.list_humans(None).await.unwrap();
        assert!(humans.len() == 1);
    }
}
