use super::{ChatMessageV2, UserDatabase};

impl UserDatabase {
    pub async fn create_message_v2(
        &self,
        message: ChatMessageV2,
    ) -> Result<ChatMessageV2, crate::Error> {
        let conn = self.conn()?;

        let mut rows = conn
            .query(
                "INSERT INTO chat_messages_v2 (
                    id, conversation_id, role, parts, metadata, 
created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?)
                RETURNING *",
                vec![
                    message.id,
                    message.conversation_id,
                    message.role.to_string(),
                    message.parts,
                    message.metadata.unwrap_or_default(),
                    message.created_at.to_rfc3339(),
                    message.updated_at.to_rfc3339(),
                ],
            )
            .await?;

        let row = rows.next().await?.unwrap();
        let message: ChatMessageV2 = libsql::de::from_row(&row)?;
        Ok(message)
    }

    pub async fn list_messages_v2(
        &self,
        conversation_id: impl Into<String>,
    ) -> Result<Vec<ChatMessageV2>, crate::Error> {
        let conn = self.conn()?;

        let mut rows = conn
            .query(
                "SELECT * FROM chat_messages_v2 
                WHERE conversation_id = ? 
                ORDER BY created_at ASC",
                vec![conversation_id.into()],
            )
            .await?;

        let mut messages = Vec::new();
        while let Some(row) = rows.next().await? {
            let message: ChatMessageV2 = libsql::de::from_row(&row)?;
            messages.push(message);
        }
        Ok(messages)
    }

    pub async fn update_message_v2_parts(
        &self,
        id: impl Into<String>,
        parts: impl Into<String>,
    ) -> Result<(), crate::Error> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE chat_messages_v2 
            SET parts = ?, updated_at = ? 
            WHERE id = ?",
            vec![parts.into(), chrono::Utc::now().to_rfc3339(), id.into()],
        )
        .await?;

        Ok(())
    }
}
