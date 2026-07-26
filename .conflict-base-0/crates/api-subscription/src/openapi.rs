use utoipa::OpenApi;

use crate::routes::{
    CanStartTrialReason, CanStartTrialResponse, DeleteAccountResponse, Interval, StartTrialReason,
    StartTrialResponse,
};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::rpc::can_start_trial,
        crate::routes::billing::start_trial,
        crate::routes::account::delete_account,
    ),
    components(
        schemas(
            CanStartTrialResponse,
            CanStartTrialReason,
            StartTrialResponse,
            StartTrialReason,
            Interval,
            DeleteAccountResponse,
        )
    ),
    tags(
        (name = "subscription", description = "Subscription and trial management")
    )
)]
struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
