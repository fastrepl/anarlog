mod auth;
mod error;
mod mcp;
mod routes;
mod state;

pub use auth::require_cloud_api_key;
pub use error::CloudApiError;
pub use routes::{connector_router, management_router};
pub use state::{AppState, CloudApiConfig};

pub fn openapi() -> utoipa::openapi::OpenApi {
    use utoipa::OpenApi;

    #[derive(OpenApi)]
    #[openapi(
        paths(
            routes::get_settings,
            routes::update_settings,
            routes::list_keys,
            routes::create_key,
            routes::revoke_key,
            routes::publish_snapshot,
            routes::delete_snapshot,
            routes::list_meetings,
            routes::get_meeting,
            routes::get_transcript,
            routes::get_history,
            routes::export_meeting,
        ),
        components(schemas(
            routes::CloudApiSettings,
            routes::UpdateSettingsBody,
            routes::ApiKeyInfo,
            routes::CreatedApiKey,
            routes::CreateApiKeyBody,
            routes::SnapshotReceipt,
            error::ErrorEnvelope,
            hypr_agent_access::MeetingListItem,
            hypr_agent_access::MeetingPage,
            hypr_agent_access::Pagination,
            hypr_agent_access::TranscriptPage,
            hypr_agent_access::Document,
            hypr_agent_access::Participant,
            hypr_agent_access::ActionItem,
            hypr_agent_access::Meeting,
            hypr_agent_access::Transcript,
            hypr_agent_access::MeetingExport,
        )),
        tags(
            (name = "cloud-api", description = "Opt-in hosted access to Anarlog meeting data")
        )
    )]
    struct CloudApiDoc;

    CloudApiDoc::openapi()
}
