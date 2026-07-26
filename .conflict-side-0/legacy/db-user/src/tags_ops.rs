use super::{Tag, UserDatabase};

impl UserDatabase {
    pub async fn upsert_tag(&self, tag: Tag) -> Result<Tag, crate::Error> {
        let conn = self.conn()?;

        let mut rows = conn
            .query(
                "INSERT OR REPLACE INTO tags (
                    id,
                    name
                ) VALUES (?, ?)
                RETURNING *",
                (tag.id, tag.name),
            )
            .await?;

        let row = rows.next().await.unwrap().unwrap();
        let tag: Tag = libsql::de::from_row(&row).unwrap();
        Ok(tag)
    }

    pub async fn delete_tag(&self, tag_id: impl Into<String>) -> Result<(), crate::Error> {
        let conn = self.conn()?;
        let tag_id = tag_id.into();

        // delete tags from sessions first
        conn.execute(
            "DELETE FROM tags_sessions WHERE tag_id = ?",
            vec![tag_id.clone()],
        )
        .await?;

        conn.query("DELETE FROM tags WHERE id = ?", vec![tag_id])
            .await?;
        Ok(())
    }

    pub async fn assign_tag_to_session(
        &self,
        tag_id: impl Into<String>,
        session_id: impl Into<String>,
    ) -> Result<(), crate::Error> {
        let conn = self.conn()?;

        conn.execute(
            "INSERT INTO tags_sessions (tag_id, session_id) VALUES (?, ?)",
            vec![tag_id.into(), session_id.into()],
        )
        .await?;
        Ok(())
    }

    pub async fn unassign_tag_from_session(
        &self,
        tag_id: impl Into<String>,
        session_id: impl Into<String>,
    ) -> Result<(), crate::Error> {
        let conn = self.conn()?;

        conn.execute(
            "DELETE FROM tags_sessions WHERE tag_id = ? AND session_id = ?",
            vec![tag_id.into(), session_id.into()],
        )
        .await?;
        Ok(())
    }

    pub async fn list_all_tags(&self) -> Result<Vec<Tag>, crate::Error> {
        let conn = self.conn()?;

        let mut rows = conn.query("SELECT * FROM tags", ()).await?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().await.unwrap() {
            let item: Tag = libsql::de::from_row(&row).unwrap();
            items.push(item);
        }
        Ok(items)
    }

    pub async fn list_session_tags(
        &self,
        session_id: impl Into<String>,
    ) -> Result<Vec<Tag>, crate::Error> {
        let conn = self.conn()?;

        let mut rows = conn
            .query(
                "SELECT t.* FROM tags t 
                 JOIN tags_sessions ts ON t.id = ts.tag_id 
                 WHERE ts.session_id = ?",
                vec![session_id.into()],
            )
            .await?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().await.unwrap() {
            let item: Tag = libsql::de::from_row(&row).unwrap();
            items.push(item);
        }
        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Human, Session, Tag, tests::setup_db};

    #[tokio::test]
    async fn test_tags() {
        let db = setup_db().await;

        let user = db
            .upsert_human(Human {
                full_name: Some("John Doe".to_string()),
                ..Human::default()
            })
            .await
            .unwrap();

        let _ = db
            .upsert_session(Session {
                id: uuid::Uuid::new_v4().to_string(),
                user_id: user.id.clone(),
                created_at: chrono::Utc::now(),
                visited_at: chrono::Utc::now(),
                calendar_event_id: None,
                title: "Test Session".to_string(),
                raw_memo_html: "".to_string(),
                enhanced_memo_html: None,
                conversations: vec![],
                words: vec![],
                record_start: None,
                record_end: None,
                pre_meeting_memo_html: None,
            })
            .await
            .unwrap();

        assert_eq!(db.list_all_tags().await.unwrap().len(), 0);

        let _ = db
            .upsert_tag(Tag {
                id: uuid::Uuid::new_v4().to_string(),
                name: "Test Tag".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(db.list_all_tags().await.unwrap().len(), 1);
    }
}
