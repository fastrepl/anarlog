use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{BotState, CaptureEvent, CaptureProviderKind, MeetingReference};

// Exact capture protocol wire shapes shared by the enterprise control plane
// (server) and capture workers (clients). Serialization only: retries, HTTP
// clients, storage, and domain behavior stay with each consumer.

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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

impl From<&CaptureJobLease> for CaptureJobLeaseIdentity {
    fn from(lease: &CaptureJobLease) -> Self {
        Self {
            worker_id: lease.worker_id.clone(),
            lease_id: lease.lease_id.clone(),
            epoch: lease.epoch,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureJobCheckpoint {
    pub job: CaptureJob,
    pub state: BotState,
    pub next_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaimCaptureJobRequest {
    pub worker_id: String,
    pub lease_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenewCaptureJobLeaseRequest {
    pub lease: CaptureJobLeaseIdentity,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppendCaptureEventRequest {
    pub lease: CaptureJobLeaseIdentity,
    pub event: CaptureEvent,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MeetingPlatform;

    fn lease() -> CaptureJobLease {
        CaptureJobLease {
            worker_id: "worker-a".to_string(),
            lease_id: "lease-a".to_string(),
            epoch: 3,
            expires_at: "2026-08-18T00:05:00Z".parse().unwrap(),
        }
    }

    #[test]
    fn lease_round_trips_the_exact_wire_shape() {
        let json = serde_json::json!({
            "workerId": "worker-a",
            "leaseId": "lease-a",
            "epoch": 3,
            "expiresAt": "2026-08-18T00:05:00Z",
        });

        let parsed: CaptureJobLease = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(parsed, lease());
        assert_eq!(serde_json::to_value(&parsed).unwrap(), json);
    }

    #[test]
    fn lease_identity_derives_from_a_lease() {
        let identity = CaptureJobLeaseIdentity::from(&lease());
        assert_eq!(
            serde_json::to_value(&identity).unwrap(),
            serde_json::json!({
                "workerId": "worker-a",
                "leaseId": "lease-a",
                "epoch": 3,
            })
        );
    }

    #[test]
    fn claim_and_renew_requests_round_trip() {
        let claim = ClaimCaptureJobRequest {
            worker_id: "worker-a".to_string(),
            lease_id: "lease-a".to_string(),
        };
        let claim_json = serde_json::json!({
            "workerId": "worker-a",
            "leaseId": "lease-a",
        });
        assert_eq!(serde_json::to_value(&claim).unwrap(), claim_json);
        assert_eq!(
            serde_json::from_value::<ClaimCaptureJobRequest>(claim_json).unwrap(),
            claim
        );

        let renew = RenewCaptureJobLeaseRequest {
            lease: CaptureJobLeaseIdentity::from(&lease()),
        };
        let renew_json = serde_json::to_value(&renew).unwrap();
        assert_eq!(renew_json["lease"]["epoch"], 3);
        assert_eq!(
            serde_json::from_value::<RenewCaptureJobLeaseRequest>(renew_json).unwrap(),
            renew
        );
    }

    #[test]
    fn checkpoint_round_trips_with_the_full_job() {
        let checkpoint = CaptureJobCheckpoint {
            job: CaptureJob {
                workspace_id: "workspace-a".to_string(),
                job_id: "job-a".to_string(),
                bot_id: "bot-a".to_string(),
                owner_user_id: "owner-a".to_string(),
                requesting_actor_id: "actor-a".to_string(),
                session_id: "session-a".to_string(),
                session_title: "Weekly sync".to_string(),
                provider: CaptureProviderKind::Anarlog,
                meeting: MeetingReference {
                    platform: MeetingPlatform::GoogleMeet,
                    url: "https://meet.google.com/abc-defg-hij".to_string(),
                    external_id: None,
                    calendar_event_id: None,
                },
                created_at: "2026-08-18T00:00:00Z".parse().unwrap(),
            },
            state: BotState::Queued,
            next_sequence: 7,
        };

        let json = serde_json::to_value(&checkpoint).unwrap();
        assert_eq!(json["job"]["workspaceId"], "workspace-a");
        assert_eq!(json["nextSequence"], 7);
        assert_eq!(
            serde_json::from_value::<CaptureJobCheckpoint>(json).unwrap(),
            checkpoint
        );
    }
}
