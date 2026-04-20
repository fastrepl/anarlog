#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Task {
    pub id: String,
    pub source_type: String,
    pub source_id: String,
    pub source_order: i64,
    pub status: String,
    pub text_preview: String,
    pub body_json: String,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub due_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub struct UpsertTask<'a> {
    pub id: &'a str,
    pub source_type: &'a str,
    pub source_id: &'a str,
    pub source_order: i64,
    pub status: &'a str,
    pub text_preview: &'a str,
    pub body_json: &'a str,
    pub due_date: Option<&'a str>,
}

pub struct MoveTaskToSource<'a> {
    pub task_ids: &'a [&'a str],
    pub source_type: &'a str,
    pub source_id: &'a str,
    pub starting_source_order: i64,
}

/// Identifies the owning surface for a set of tasks (e.g. a session or a
/// daily note). Mirrors the `source_type` / `source_id` columns.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct TaskSource {
    pub source_type: String,
    pub source_id: String,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CreateTaskInput {
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub id: Option<String>,
    pub source_type: String,
    pub source_id: String,
    pub source_order: i64,
    pub status: String,
    pub text_preview: String,
    pub body_json: String,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub due_date: Option<String>,
}

/// Partial update for a task. Each `Some` field overwrites the existing
/// value; `None` fields are left untouched.
#[derive(Debug, Clone, Default)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase", default)
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct UpdateTaskPatch {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub source_type: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub source_id: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub source_order: Option<i64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub status: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub text_preview: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub body_json: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub due_date: Option<String>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct MoveTasksToSourceInput {
    pub task_ids: Vec<String>,
    pub source_type: String,
    pub source_id: String,
    pub starting_source_order: i64,
}
