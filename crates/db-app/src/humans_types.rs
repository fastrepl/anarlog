#[cfg_attr(feature = "cli", derive(serde::Serialize))]
pub struct HumanRow {
    pub id: String,
    pub created_at: String,
    pub name: String,
    pub email: String,
    pub org_id: String,
    pub job_title: String,
    pub linkedin_username: String,
    pub memo: String,
    pub pinned: bool,
    pub pin_order: i32,
    pub user_id: String,
    pub linked_user_id: Option<String>,
}
