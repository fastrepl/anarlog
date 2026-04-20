#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Session {
    pub id: String,
    pub title: String,
    pub raw_md: String,
    pub folder_id: String,
    pub event_json: String,
    pub created_at: String,
    pub updated_at: String,
}

pub struct UpsertSession<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub raw_md: &'a str,
    pub folder_id: &'a str,
    pub event_json: &'a str,
}

/// Fields optionally supplied when creating a session. Any omitted field
/// falls back to the caller's default; see `hypr_api::sessions::create_session`.
#[derive(Debug, Clone, Default)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase", default)
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CreateSessionInput {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub title: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub raw_md: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub folder_id: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub event_json: Option<String>,
}

/// Partial update for a session. Each `Some` field overwrites the existing
/// value; `None` fields are left untouched.
#[derive(Debug, Clone, Default)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase", default)
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct UpdateSessionPatch {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub title: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub raw_md: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub folder_id: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub event_json: Option<String>,
}
