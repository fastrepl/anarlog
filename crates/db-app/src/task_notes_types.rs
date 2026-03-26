pub struct TaskNoteRow {
    pub id: String,
    pub task_id: String,
    pub author_type: String,
    pub author_id: String,
    pub body: String,
    pub user_id: String,
    pub visibility: String,
    pub created_at: String,
    pub deleted_at: Option<String>,
}
