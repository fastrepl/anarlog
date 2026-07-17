use axum::{
    Extension,
    extract::{Query, State},
    response::{IntoResponse, Response},
};
use hypr_analytics::{AnalyticsClient, DeviceFingerprint, ToAnalyticsPayload};
use hypr_api_auth::AuthContext;

use crate::state::AppState;
use crate::stripe::{create_trial_subscription, get_or_create_customer};
use crate::trial::{Interval, StartTrialQuery, StartTrialResponse, TrialOutcome};

#[utoipa::path(
    post,
    path = "/start-trial",
    params(StartTrialQuery),
    responses(
        (status = 200, description = "Trial started successfully", body = StartTrialResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "subscription",
)]
#[tracing::instrument(
    name = "subscription.start_trial",
    skip(state, query, auth, device_fingerprint),
    fields(hyprnote.subsystem = "subscription")
)]
pub async fn start_trial(
    State(state): State<AppState>,
    Query(query): Query<StartTrialQuery>,
    Extension(auth): Extension<AuthContext>,
    device_fingerprint: Option<Extension<DeviceFingerprint>>,
) -> Response {
    let user_id = &auth.claims.sub;
    let device_fingerprint =
        device_fingerprint.map(|Extension(DeviceFingerprint(fingerprint))| fingerprint);

    let source = if device_fingerprint.is_some() {
        "desktop"
    } else {
        "web"
    };
    let distinct_id = device_fingerprint.as_deref().unwrap_or(user_id);

    let can_start: bool = match state
        .supabase
        .rpc("can_start_trial", &auth.token, None)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            return emit_and_respond(
                state.config.analytics.as_deref(),
                distinct_id,
                user_id,
                source,
                TrialOutcome::RpcError(e.to_string()),
            )
            .await;
        }
    };

    let outcome =
        if !can_start {
            TrialOutcome::NotEligible
        } else {
            let customer_id =
                match get_or_create_customer(&state.supabase, &state.stripe, &auth.token, user_id)
                    .await
                {
                    Ok(Some(id)) => id,
                    Ok(None) => {
                        return emit_and_respond(
                            state.config.analytics.as_deref(),
                            distinct_id,
                            user_id,
                            source,
                            TrialOutcome::CustomerError("customer not found".to_string()),
                        )
                        .await;
                    }
                    Err(e) => {
                        return emit_and_respond(
                            state.config.analytics.as_deref(),
                            distinct_id,
                            user_id,
                            source,
                            TrialOutcome::CustomerError(e.to_string()),
                        )
                        .await;
                    }
                };

            let price_id = match query.interval {
                Interval::Monthly => &state.config.stripe.stripe_monthly_price_id,
                Interval::Yearly => &state.config.stripe.stripe_yearly_price_id,
            };

            match create_trial_subscription(&state.stripe, &customer_id, price_id, user_id).await {
                Ok(()) => TrialOutcome::Started(query.interval),
                Err(e) => TrialOutcome::StripeError(e.to_string()),
            }
        };

    emit_and_respond(
        state.config.analytics.as_deref(),
        distinct_id,
        user_id,
        source,
        outcome,
    )
    .await
}

async fn emit_and_respond<O>(
    analytics: Option<&AnalyticsClient>,
    distinct_id: &str,
    user_id: &str,
    source: &str,
    outcome: O,
) -> Response
where
    O: IntoResponse + ToAnalyticsPayload,
{
    if let Some(analytics) = analytics {
        let mut payload = outcome.to_analytics_payload();
        payload.props.insert("source".to_string(), source.into());
        if distinct_id != user_id {
            payload.props.insert("user_id".to_string(), user_id.into());
        }

        if let Err(e) = analytics.event(distinct_id, payload).await {
            tracing::warn!("analytics event error: {e}");
        }
        if let Some(props) = outcome.to_analytics_properties()
            && let Err(e) = analytics.set_properties(user_id, props).await
        {
            tracing::warn!("analytics set_properties error: {e}");
        }
    }
    outcome.into_response()
}
