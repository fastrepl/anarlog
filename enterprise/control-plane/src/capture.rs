use anlg_meeting_capture::{BotState, CaptureProviderKind, MeetingReference};
// The exact capture wire contract is owned by the shared MIT-layer module and
// used unchanged by the control plane and every capture worker.
pub use anlg_meeting_capture::wire::{
    AppendCaptureEventRequest, CaptureJob, CaptureJobCheckpoint, CaptureJobLease,
    CaptureJobLeaseIdentity, ClaimCaptureJobRequest, RenewCaptureJobLeaseRequest,
};
use anlg_session_ingest::SessionIngestEnvelope;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureJobStatus {
    pub job_id: String,
    pub created: bool,
    pub state: BotState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CaptureDispatch {
    pub workspace_id: String,
    pub job_id: String,
    pub provider: CaptureProviderKind,
    pub dispatch_id: String,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectionPublication {
    pub job_id: String,
    pub revision: u64,
    pub finalized: bool,
    pub content_hash: String,
    pub envelope: SessionIngestEnvelope,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCaptureJobRequest {
    pub bot_id: String,
    pub owner_user_id: String,
    pub requesting_actor_id: String,
    pub session_id: String,
    pub session_title: String,
    pub provider: CaptureProviderKind,
    pub meeting: MeetingReference,
    pub created_at: DateTime<Utc>,
}
