#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptOverrideRow {
    pub task_type: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}

pub struct UpsertPromptOverride<'a> {
    pub task_type: &'a str,
    pub content: &'a str,
}
