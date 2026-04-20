#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct DailyNote {
    pub date: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}

pub struct UpsertDailyNote<'a> {
    pub date: &'a str,
    pub content: &'a str,
}

/// Partial upsert for a daily note; omit `content` to read-modify-write the
/// existing row without touching its body.
#[derive(Debug, Clone, Default)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase", default)
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct UpsertDailyNotePatch {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub content: Option<String>,
}
