use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::bot::send_bot,
        crate::routes::bot::remove_bot,
        crate::routes::bot::start_demo,
        crate::routes::bot::demo_status,
    ),
    components(
        schemas(
            crate::routes::bot::SendBotRequest,
            crate::routes::bot::SendBotResponse,
            crate::routes::bot::StartDemoRequest,
            crate::routes::bot::StartDemoResponse,
            crate::routes::bot::DemoStatusResponse,
        )
    ),
    tags(
        (name = "bot", description = "Meeting bot management")
    )
)]
struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
