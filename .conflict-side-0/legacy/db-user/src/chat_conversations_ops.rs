use super::{ChatConversation, UserDatabase};

impl UserDatabase {
    pub async fn create_conversation(
        &self,
        conversation: ChatConversation,
    ) -> Result<ChatConversation, crate::Error> {
        let conn = self.conn()?;

        let mut rows = conn
            .query(
                "INSERT INTO chat_conversations (
                    id, session_id, user_id, name, created_at, 
updated_at
                ) VALUES (?, ?, ?, ?, ?, ?)
                RETURNING *",
                vec![
                    conversation.id,
                    conversation.session_id,
                    conversation.user_id,
                    conversation.name.unwrap_or_default(),
                    conversation.created_at.to_rfc3339(),
                    conversation.updated_at.to_rfc3339(),
                ],
            )
            .await?;

        let row = rows.next().await?.unwrap();
        let conversation: ChatConversation = libsql::de::from_row(&row)?;
        Ok(conversation)
    }

    pub async fn list_conversations(
        &self,
        session_id: impl Into<String>,
    ) -> Result<Vec<ChatConversation>, crate::Error> {
        let conn = self.conn()?;

        let mut rows = conn
            .query(
                "SELECT * FROM chat_conversations 
                WHERE session_id = ? 
                ORDER BY updated_at DESC",
                vec![session_id.into()],
            )
            .await?;

        let mut conversations = Vec::new();
        while let Some(row) = rows.next().await? {
            let conversation: ChatConversation = libsql::de::from_row(&row)?;
            conversations.push(conversation);
        }
        Ok(conversations)
    }

    pub async fn get_conversation(
        &self,
        id: impl Into<String>,
    ) -> Result<Option<ChatConversation>, crate::Error> {
        let conn = self.conn()?;

        let mut rows = conn
            .query(
                "SELECT * FROM chat_conversations WHERE id = ?",
                vec![id.into()],
            )
            .await?;

        if let Some(row) = rows.next().await? {
            let conversation: ChatConversation = libsql::de::from_row(&row)?;
            Ok(Some(conversation))
        } else {
            Ok(None)
        }
    }
}
