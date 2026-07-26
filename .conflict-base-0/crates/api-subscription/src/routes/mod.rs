pub(crate) mod account;
pub(crate) mod billing;
pub(crate) mod rpc;

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
    let state = AppState::new(config);

    Router::new()
        .route("/can-start-trial", get(rpc::can_start_trial))
        .route("/start-trial", post(billing::start_trial))
        .route("/delete-account", delete(account::delete_account))
        .with_state(state)
}
