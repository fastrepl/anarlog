use crate::user_common_derives;

user_common_derives! {
    pub struct ChatGroup {
        pub id: String,
        pub user_id: String,
        pub name: Option<String>,
        pub created_at: chrono::DateTime<chrono::Utc>,
        pub session_id: String,
    }
}
