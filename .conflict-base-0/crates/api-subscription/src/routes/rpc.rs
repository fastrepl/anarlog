use axum::{Extension, Json, extract::State};
use serde::Serialize;
use utoipa::ToSchema;

use crate::state::AppState;

use hypr_api_auth::AuthContext;

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
) -> Json<CanStartTrialResponse> {
    let result: std::result::Result<bool, _> = state
        .supabase
        .rpc("can_start_trial", &auth.token, None)
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
