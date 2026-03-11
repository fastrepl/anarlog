uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum BridgeError {
    #[error("Invalid TipTap JSON: {reason}")]
    InvalidJson { reason: String },
    #[error("Document conversion failed: {reason}")]
    InvalidDocument { reason: String },
    #[error("JSON serialization failed: {reason}")]
    SerializationFailed { reason: String },
}

#[uniffi::export]
pub fn tiptap_json_to_markdown(json: String) -> Result<String, BridgeError> {
    let value: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| BridgeError::InvalidJson {
            reason: e.to_string(),
        })?;

    hypr_tiptap::tiptap_json_to_md(&value).map_err(|reason| BridgeError::InvalidDocument { reason })
}

#[uniffi::export]
pub fn markdown_to_tiptap_json(md: String) -> Result<String, BridgeError> {
    let value = hypr_tiptap::md_to_tiptap_json(&md)
        .map_err(|reason| BridgeError::InvalidDocument { reason })?;

    serde_json::to_string_pretty(&value).map_err(|e| BridgeError::SerializationFailed {
        reason: e.to_string(),
    })
}

#[uniffi::export]
pub fn markdown_to_tiptap_json_compact(md: String) -> Result<String, BridgeError> {
    let value = hypr_tiptap::md_to_tiptap_json(&md)
        .map_err(|reason| BridgeError::InvalidDocument { reason })?;

    serde_json::to_string(&value).map_err(|e| BridgeError::SerializationFailed {
        reason: e.to_string(),
    })
}
