use hypr_api_auth::AuthContext;
use rmcp::{
    ErrorData as McpError,
    model::*,
    schemars::{self, JsonSchema},
};
use serde::Deserialize;
use stripe_billing::subscription::{ListSubscription, ListSubscriptionStatus};

use crate::state::AppState;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ListSubscriptionsParams {
    #[schemars(
        description = "Filter by subscription status. Values: 'active', 'canceled', 'ended', 'past_due', 'trialing', 'unpaid', or 'all'. Omit to return only active subscriptions."
    )]
    pub status: Option<String>,
}

pub(crate) async fn list_subscriptions(
    state: &AppState,
    auth: &AuthContext,
    params: ListSubscriptionsParams,
) -> Result<CallToolResult, McpError> {
    let customer_id = state
        .get_stripe_customer_id(auth)
        .await
        .map_err(|e| McpError::internal_error(e, None))?
        .ok_or_else(|| {
            McpError::invalid_request("No Stripe customer found for this account", None)
        })?;

    let mut req = ListSubscription::new().customer(&customer_id);
    if let Some(status) = params.status {
        let parsed: ListSubscriptionStatus = status
            .parse()
            .map_err(|_| McpError::invalid_params(format!("Invalid status: {status}"), None))?;
        req = req.status(parsed);
    }

    let list = req
        .send(&state.stripe)
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    let subscriptions: Vec<serde_json::Value> = list
        .data
        .iter()
        .map(|sub| {
            serde_json::json!({
                "id": sub.id.as_str(),
                "status": format!("{:?}", sub.status),
                "start_date": sub.start_date,
                "cancel_at_period_end": sub.cancel_at_period_end,
                "cancel_at": sub.cancel_at,
                "canceled_at": sub.canceled_at,
                "trial_start": sub.trial_start,
                "trial_end": sub.trial_end,
            })
        })
        .collect();

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string(&subscriptions)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?,
    )]))
}
