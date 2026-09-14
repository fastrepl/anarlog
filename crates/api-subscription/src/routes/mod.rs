pub(crate) mod account;
pub(crate) mod billing;
pub(crate) mod rpc;
pub(crate) mod scim;

use axum::{
    Router,
    routing::{delete, get, post},
};

use crate::config::SubscriptionConfig;
use crate::state::AppState;

pub use crate::trial::{Interval, StartTrialReason, StartTrialResponse};
pub use account::DeleteAccountResponse;
pub use rpc::{CanStartTrialReason, CanStartTrialResponse};

pub fn router(config: SubscriptionConfig) -> Router {
    billing_router(config.clone()).merge(account_router(config))
}

pub fn billing_router(config: SubscriptionConfig) -> Router {
    let state = AppState::new(config);

    Router::new()
        .route("/can-start-trial", get(rpc::can_start_trial))
        .route("/start-trial", post(billing::start_trial))
        .with_state(state)
}

pub fn account_router(config: SubscriptionConfig) -> Router {
    Router::new()
        .route("/delete-account", delete(account::delete_account))
        .with_state(AppState::new(config))
}

pub fn scim_router(config: SubscriptionConfig) -> Router {
    Router::new()
        .merge(scim::router())
        .with_state(AppState::new(config))
}
