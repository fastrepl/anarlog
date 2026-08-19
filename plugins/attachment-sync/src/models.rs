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

// Only legal delete outcomes are constructible: skipping, deleting without a
// local guard file, or deleting with a sealed guard identified by guard_id.
#[derive(Debug, Clone, serde::Serialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum DeleteGuardOutcome {
    Skip,
    DeleteDirectly,
    #[serde(rename_all = "camelCase")]
    DeleteWithGuard {
        guard_id: String,
    },
}

#[derive(Debug, Clone, serde::Serialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PreparedDeleteGuard {
    pub attachment_ref: String,
    pub version_ref: String,
    pub outcome: DeleteGuardOutcome,
}

#[cfg(test)]
mod delete_guard_outcome_tests {
    use super::*;

    #[test]
    fn outcomes_serialize_as_tagged_variants() {
        let base = |outcome| PreparedDeleteGuard {
            attachment_ref: "attachment".to_string(),
            version_ref: "version".to_string(),
            outcome,
        };

        assert_eq!(
            serde_json::to_value(base(DeleteGuardOutcome::Skip)).unwrap(),
            serde_json::json!({
                "attachmentRef": "attachment",
                "versionRef": "version",
                "outcome": { "kind": "skip" },
            })
        );
        assert_eq!(
            serde_json::to_value(base(DeleteGuardOutcome::DeleteDirectly)).unwrap(),
            serde_json::json!({
                "attachmentRef": "attachment",
                "versionRef": "version",
                "outcome": { "kind": "deleteDirectly" },
            })
        );
        assert_eq!(
            serde_json::to_value(base(DeleteGuardOutcome::DeleteWithGuard {
                guard_id: "guard-1".to_string(),
            }))
            .unwrap(),
            serde_json::json!({
                "attachmentRef": "attachment",
                "versionRef": "version",
                "outcome": { "kind": "deleteWithGuard", "guardId": "guard-1" },
            })
        );
    }
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
