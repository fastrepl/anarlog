#[cfg_attr(feature = "cli", derive(serde::Serialize))]
pub struct UserRow {
    pub id: String,
    pub name: String,
    pub created_at: String,
}
