#[cfg_attr(feature = "cli", derive(serde::Serialize))]
pub struct OrganizationRow {
    pub id: String,
    pub created_at: String,
    pub name: String,
    pub pinned: bool,
    pub pin_order: i32,
    pub user_id: String,
}
