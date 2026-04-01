pub struct ThreadRow {
    pub id: String,
    pub user_id: String,
    pub meeting_id: Option<String>,
    pub title: String,
    pub visibility: String,
    pub created_at: String,
}

pub struct MessageRow {
    pub id: String,
    pub user_id: String,
    pub thread_id: String,
    pub role: String,
    pub parts: String,
    pub visibility: String,
    pub created_at: String,
}
