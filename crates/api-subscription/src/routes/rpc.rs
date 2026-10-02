use anlg_analytics::DeviceFingerprint;
use axum::{Extension, Json, extract::State};
use serde::Serialize;
use serde_json::json;
use utoipa::ToSchema;

use crate::state::AppState;

use anlg_api_auth::AuthContext;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CanStartTrialReason {
    Eligible,
    NotEligible,
    Error,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CanStartTrialResponse {
    #[schema(example = true)]
    pub can_start_trial: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<CanStartTrialReason>,
}

#[utoipa::path(
    get,
    path = "/can-start-trial",
    responses(
        (status = 200, description = "Check successful", body = CanStartTrialResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "subscription",
)]
pub async fn can_start_trial(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    device_fingerprint: Option<Extension<DeviceFingerprint>>,
) -> Json<CanStartTrialResponse> {
    let device_fingerprint =
        device_fingerprint.map(|Extension(DeviceFingerprint(fingerprint))| fingerprint);

    let result: std::result::Result<bool, _> = state
        .supabase
        .rpc(
            "can_start_trial",
            &auth.token,
            Some(json!({
                "p_device_fingerprint": device_fingerprint,
            })),
        )
        .await;

    match result {
        Ok(true) => Json(CanStartTrialResponse {
            can_start_trial: true,
            reason: Some(CanStartTrialReason::Eligible),
        }),
        Ok(false) => Json(CanStartTrialResponse {
            can_start_trial: false,
            reason: Some(CanStartTrialReason::NotEligible),
        }),
        Err(e) => {
            tracing::error!(error = %e, "can_start_trial_rpc_failed");
            Json(CanStartTrialResponse {
                can_start_trial: false,
                reason: Some(CanStartTrialReason::Error),
            })
        }
    }
}
