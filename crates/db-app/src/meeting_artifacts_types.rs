pub struct MeetingArtifactRow {
    pub id: String,
    pub task_id: String,
    pub transcript_md: String,
    pub note_body: String,
    pub user_id: String,
    pub visibility: String,
    pub created_at: String,
    pub updated_at: String,
}

pub struct MeetingSummaryRow {
    pub id: String,
    pub task_id: String,
    pub template_id: String,
    pub content: String,
    pub position: i32,
    pub title: String,
    pub user_id: String,
    pub visibility: String,
    pub created_at: String,
    pub updated_at: String,
}
