use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use hypr_analytics::{AnalyticsPayload, PropertiesPayload, ToAnalyticsPayload};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use hypr_api_error::error_response;

#[derive(Debug, Deserialize, IntoParams)]
pub struct StartTrialQuery {
    #[serde(default = "default_interval")]
    #[param(example = "monthly")]
    pub interval: Interval,
}

fn default_interval() -> Interval {
    Interval::Monthly
}

#[derive(Debug, Deserialize, Clone, Copy, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Interval {
    Monthly,
    Yearly,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StartTrialReason {
    Started,
    NotEligible,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StartTrialResponse {
    #[schema(example = true)]
    pub started: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<StartTrialReason>,
}

pub(crate) enum TrialOutcome {
    NotEligible,
    StripeError(String),
    CustomerError(String),
    RpcError(String),
    Started(Interval),
}

impl ToAnalyticsPayload for TrialOutcome {
    fn to_analytics_payload(&self) -> AnalyticsPayload {
        match self {
            Self::NotEligible => AnalyticsPayload::builder("trial_skipped")
                .with("reason", "not_eligible")
                .build(),
            Self::StripeError(_) => AnalyticsPayload::builder("trial_failed")
                .with("reason", "stripe_error")
                .build(),
            Self::CustomerError(_) => AnalyticsPayload::builder("trial_failed")
                .with("reason", "customer_error")
                .build(),
            Self::RpcError(_) => AnalyticsPayload::builder("trial_failed")
                .with("reason", "rpc_error")
                .build(),
            Self::Started(interval) => {
                let plan = match interval {
                    Interval::Monthly => "pro_monthly",
                    Interval::Yearly => "pro_yearly",
                };
                AnalyticsPayload::builder("trial_started")
                    .with("plan", plan)
                    .build()
            }
        }
    }

    fn to_analytics_properties(&self) -> Option<PropertiesPayload> {
        match self {
            Self::Started(_) => {
                let trial_end_date = (Utc::now() + chrono::Duration::days(14)).to_rfc3339();
                Some(
                    PropertiesPayload::builder()
                        .set("plan", "pro")
                        .set("trial_end_date", trial_end_date)
                        .build(),
                )
            }
            _ => None,
        }
    }
}

impl IntoResponse for TrialOutcome {
    fn into_response(self) -> Response {
        match self {
            Self::NotEligible => Json(StartTrialResponse {
                started: false,
                reason: Some(StartTrialReason::NotEligible),
            })
            .into_response(),
            Self::StripeError(msg) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed_to_create_subscription",
                &msg,
            ),
            Self::CustomerError(msg) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed_to_create_customer",
                &msg,
            ),
            Self::RpcError(msg) => {
                error_response(StatusCode::INTERNAL_SERVER_ERROR, "rpc_error", &msg)
            }
            Self::Started(_) => Json(StartTrialResponse {
                started: true,
                reason: Some(StartTrialReason::Started),
            })
            .into_response(),
        }
    }
}
