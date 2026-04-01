#[cfg_attr(feature = "cli", derive(serde::Serialize))]
pub struct FolderRow {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub created_at: String,
}
