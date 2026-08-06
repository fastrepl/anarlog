use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::messenger::list_slack_channels,
        crate::routes::messenger::send_slack_message,
    ),
    components(schemas(
        crate::routes::messenger::SlackSendRequest,
        crate::routes::messenger::SlackChannel,
        crate::routes::messenger::SlackChannelsResponse,
        crate::routes::messenger::SendMessageResponse,
    )),
    tags(
        (name = "messenger", description = "Messaging integrations")
    )
)]
struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
