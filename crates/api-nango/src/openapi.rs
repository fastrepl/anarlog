use utoipa::OpenApi;

use crate::routes::{
    ConnectionItem, CreateSessionRequest, DeleteConnectionRequest, DeleteConnectionResponse,
    ListConnectionsResponse, SessionResponse, WebhookResponse,
};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::connect::create_session,
        crate::routes::disconnect::delete_connection,
        crate::routes::status::list_connections,
        crate::routes::webhook::nango_webhook,
    ),
    components(
        schemas(
            CreateSessionRequest,
            DeleteConnectionRequest,
            DeleteConnectionResponse,
            SessionResponse,
            ConnectionItem,
            ListConnectionsResponse,
            WebhookResponse,
        )
    ),
    tags(
        (name = "nango", description = "Integration management via Nango")
    )
)]
struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
