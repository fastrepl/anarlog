use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use hypr_analytics::{AnalyticsPayload, PropertiesPayload, ToAnalyticsPayload};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use utoipa::{IntoParams, ToSchema};

use hypr_api_error::error_response;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrialPolicy {
    pro_trial_days: u32,
}

static TRIAL_POLICY: LazyLock<TrialPolicy> = LazyLock::new(|| {
    serde_json::from_str(include_str!(
        "../../../packages/pricing/src/trial-policy.json"
    ))
    .expect("shared trial policy must be valid JSON")
});

pub(crate) fn pro_trial_days() -> u32 {
    TRIAL_POLICY.pro_trial_days
}

fn trial_end_date(now: chrono::DateTime<Utc>, stripe_trial_end: Option<i64>) -> String {
    stripe_trial_end
        .and_then(|timestamp| chrono::DateTime::from_timestamp(timestamp, 0))
        .unwrap_or_else(|| now + chrono::Duration::days(i64::from(pro_trial_days())))
        .to_rfc3339()
}

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
    Started(Interval, Option<i64>),
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
            Self::Started(interval, _) => {
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
            Self::Started(_, stripe_trial_end) => {
                let trial_end_date = trial_end_date(Utc::now(), *stripe_trial_end);
                Some(
                    PropertiesPayload::builder()
                        .set("plan", "trial")
                        .set("trial_end_date", trial_end_date)
                        .build(),
                )
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use hypr_analytics::ToAnalyticsPayload;
    use serde_json::json;

    use super::{Interval, TrialOutcome, pro_trial_days, trial_end_date};

    #[test]
    fn analytics_uses_the_product_trial_duration() {
        let now = Utc.with_ymd_and_hms(2026, 7, 17, 0, 0, 0).unwrap();

        assert_eq!(trial_end_date(now, None), "2026-08-07T00:00:00+00:00");
        assert_eq!(pro_trial_days(), 21);
    }

    #[test]
    fn analytics_uses_stripes_actual_trial_end_when_available() {
        let stripe_trial_end = Utc
            .with_ymd_and_hms(2026, 8, 9, 12, 30, 0)
            .unwrap()
            .timestamp();
        let outcome = TrialOutcome::Started(Interval::Monthly, Some(stripe_trial_end));
        let event = outcome.to_analytics_payload();
        let properties = outcome.to_analytics_properties().unwrap();

        assert_eq!(event.event, "trial_started");
        assert_eq!(event.props.get("plan"), Some(&json!("pro_monthly")));
        assert_eq!(properties.set.get("plan"), Some(&json!("trial")));
        assert_eq!(
            properties.set.get("trial_end_date"),
            Some(&json!("2026-08-09T12:30:00+00:00"))
        );
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
            Self::Started(_, _) => Json(StartTrialResponse {
                started: true,
                reason: Some(StartTrialReason::Started),
            })
            .into_response(),
        }
    }
}
