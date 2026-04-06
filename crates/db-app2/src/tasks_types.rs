#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRow {
    pub id: String,
    pub daily_note_id: String,
    pub parent_task_id: Option<String>,
    pub sort_key: String,
    pub kind: String,
    pub title: String,
    pub status: String,
    pub body_json: String,
    pub source_type: String,
    pub source_id: String,
    pub due_date: Option<String>,
    pub metadata_json: String,
    pub user_id: String,
    pub created_at: String,
    pub updated_at: String,
}
