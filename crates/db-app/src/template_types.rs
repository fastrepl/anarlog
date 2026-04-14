#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct TemplateRow {
    pub id: String,
    pub title: String,
    pub description: String,
    pub pinned: bool,
    pub pin_order: Option<i64>,
    pub category: Option<String>,
    pub targets_json: Option<String>,
    pub sections_json: String,
    pub created_at: String,
    pub updated_at: String,
}

pub struct UpsertTemplate<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub description: &'a str,
    pub pinned: bool,
    pub pin_order: Option<i64>,
    pub category: Option<&'a str>,
    pub targets_json: Option<&'a str>,
    pub sections_json: &'a str,
}
