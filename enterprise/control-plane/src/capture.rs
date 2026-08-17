use anlg_meeting_capture::{BotState, CaptureEvent, CaptureProviderKind, MeetingReference};
use anlg_session_ingest::SessionIngestEnvelope;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureJob {
    pub workspace_id: String,
    pub job_id: String,
    pub bot_id: String,
    pub owner_user_id: String,
    pub requesting_actor_id: String,
    pub session_id: String,
    pub session_title: String,
    pub provider: CaptureProviderKind,
    pub meeting: MeetingReference,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureJobStatus {
    pub job_id: String,
    pub created: bool,
    pub state: BotState,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureJobCheckpoint {
    pub job: CaptureJob,
    pub state: BotState,
    pub next_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureJobLease {
    pub worker_id: String,
    pub lease_id: String,
    pub epoch: u64,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureJobLeaseIdentity {
    pub worker_id: String,
    pub lease_id: String,
    pub epoch: u64,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaimCaptureJobRequest {
    pub worker_id: String,
    pub lease_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenewCaptureJobLeaseRequest {
    pub lease: CaptureJobLeaseIdentity,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppendCaptureEventRequest {
    pub lease: CaptureJobLeaseIdentity,
    pub event: CaptureEvent,
}
