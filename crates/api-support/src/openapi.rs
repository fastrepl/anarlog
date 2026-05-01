use utoipa::OpenApi;

use crate::routes::{FeedbackRequest, FeedbackResponse};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::feedback::submit,
    ),
    components(
        schemas(
            FeedbackRequest,
            FeedbackResponse,
            crate::routes::feedback::FeedbackType,
            crate::routes::feedback::DeviceInfo,
        )
    ),
    tags(
        (name = "support", description = "User feedback and support")
    )
)]
struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
