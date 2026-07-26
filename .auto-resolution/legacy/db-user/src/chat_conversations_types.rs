use crate::user_common_derives;

user_common_derives! {
    pub struct ChatConversation {
        pub id: String,
        pub session_id: String,
        pub user_id: String,
        pub name: Option<String>,
        pub created_at: chrono::DateTime<chrono::Utc>,
        pub updated_at: chrono::DateTime<chrono::Utc>,
    }
}
