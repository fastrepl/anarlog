use super::{Config, UserDatabase};

impl UserDatabase {
    pub async fn get_config(
        &self,
        user_id: impl Into<String>,
    ) -> Result<Option<Config>, crate::Error> {
        let conn = self.conn()?;

        let mut rows = conn
            .query(
                "SELECT * FROM configs WHERE user_id = ?",
                vec![user_id.into()],
            )
            .await?;

        match rows.next().await? {
            None => Ok(None),
            Some(row) => {
                let config = Config::from_row(&row)?;
                Ok(Some(config))
            }
        }
    }

    pub async fn set_config(&self, config: Config) -> Result<(), crate::Error> {
        let conn = self.conn()?;

        conn.execute(
            "INSERT OR REPLACE INTO configs (
                    id,
                    user_id,
                    general,
                    notification,
                    ai
                ) VALUES (?, ?, ?, ?, ?)",
            vec![
                config.id.clone(),
                config.user_id.clone(),
                serde_json::to_string(&config.general)?,
                serde_json::to_string(&config.notification)?,
                serde_json::to_string(&config.ai)?,
            ],
        )
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{Config, ConfigAI, ConfigGeneral, ConfigNotification, Human, tests::setup_db};

    #[tokio::test]
    async fn test_config() {
        let db = setup_db().await;

        let human = db
            .upsert_human(Human {
                full_name: Some("yujonglee".to_string()),
                ..Human::default()
            })
            .await
            .unwrap();

        db.set_config(Config {
            id: uuid::Uuid::new_v4().to_string(),
            user_id: human.id.clone(),
            general: ConfigGeneral::default(),
            notification: ConfigNotification {
                before: false,
                ..ConfigNotification::default()
            },
            ai: ConfigAI::default(),
        })
        .await
        .unwrap();

        let config = db.get_config(human.id).await.unwrap().unwrap();
        assert_eq!(config.notification.before, false);
    }
}
