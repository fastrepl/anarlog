use hypr_api_auth::AuthContext;
use rmcp::{
    ErrorData as McpError,
    model::*,
    schemars::{self, JsonSchema},
};
use serde::Deserialize;
use stripe_billing::billing_portal_session::CreateBillingPortalSession;

use crate::state::AppState;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct CreateBillingPortalSessionParams {
    #[schemars(
        description = "URL to redirect the user back to after they finish in the billing portal. Can be omitted."
    )]
    pub return_url: Option<String>,
}

pub(crate) async fn create_billing_portal_session(
    state: &AppState,
    auth: &AuthContext,
    params: CreateBillingPortalSessionParams,
) -> Result<CallToolResult, McpError> {
    let customer_id = state
        .get_stripe_customer_id(auth)
        .await
        .map_err(|e| McpError::internal_error(e, None))?
        .ok_or_else(|| {
            McpError::invalid_request("No Stripe customer found for this account", None)
        })?;

    let mut req = CreateBillingPortalSession::new().customer(&customer_id);
    if let Some(url) = params.return_url {
        req = req.return_url(url);
    }
    let session = req
        .send(&state.stripe)
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::json!({ "url": session.url }).to_string(),
    )]))
}
