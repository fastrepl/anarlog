#[derive(Debug, Clone, serde::Serialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UploadDescriptor {
    pub attachment_ref: String,
    pub version_ref: String,
    pub ciphertext_size_bytes: u64,
    pub format_version: i16,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PreparedUpload {
    pub cache_id: String,
    pub ciphertext_sha256: String,
    pub ciphertext_size_bytes: u64,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PreparedSharedUpload {
    pub cache_id: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PreparedDeleteGuard {
    pub should_delete: bool,
    pub guard_id: String,
}

#[derive(Debug, Clone, serde::Deserialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SharedUploadVersion {
    pub sha256: String,
    pub size_bytes: u64,
    pub filename: String,
    pub content_type: String,
    pub cloud_object_key: String,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RestoredAttachment {
    pub attachment_id: String,
    pub session_id: String,
    pub relative_path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SharedAttachmentCacheResult {
    pub cache_id: String,
    pub local_path: String,
    pub size_bytes: u64,
    pub sha256: String,
}
