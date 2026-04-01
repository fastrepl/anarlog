use utoipa::OpenApi;

use crate::routes::{
    ConnectionItem, CreateSessionRequest, DeleteConnectionRequest, DeleteConnectionResponse,
    ListConnectionsResponse, SessionMode, SessionResponse, WebhookResponse, WhoAmIItem,
    WhoAmIResponse,
};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::connect::create_session,
        crate::routes::disconnect::delete_connection,
        crate::routes::status::list_connections,
        crate::routes::webhook::nango_webhook,
        crate::routes::whoami::whoami,
    ),
    components(
        schemas(
            CreateSessionRequest,
            SessionMode,
            DeleteConnectionRequest,
            DeleteConnectionResponse,
            SessionResponse,
            ConnectionItem,
            ListConnectionsResponse,
            WebhookResponse,
            WhoAmIItem,
            WhoAmIResponse,
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
