use serde::{Deserialize, Serialize};

use crate::SessionIngestEnvelope;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeliveryItem {
    pub cursor: u64,
    pub job_id: String,
    pub revision: u64,
    pub finalized: bool,
    pub content_hash: String,
    pub acknowledged: bool,
    pub created_at: String,
    pub envelope: SessionIngestEnvelope,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeliveryPage {
    pub items: Vec<DeliveryItem>,
    pub next_cursor: u64,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcknowledgeRequest {
    pub consumer_id: String,
    pub revision: u64,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcknowledgeResponse {
    pub acknowledged: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionRead {
    pub job_id: String,
    pub revision: u64,
    pub finalized: bool,
    pub content_hash: String,
    pub envelope: SessionIngestEnvelope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordingDownload {
    pub url: String,
    pub expires_at: String,
}
