#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyNoteRow {
    pub id: String,
    pub date: String,
    pub content: String,
    pub user_id: String,
    pub created_at: String,
    pub updated_at: String,
}
