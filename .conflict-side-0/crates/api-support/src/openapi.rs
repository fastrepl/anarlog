use utoipa::OpenApi;

use crate::routes::{FeedbackRequest, FeedbackResponse};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::feedback::submit,
        crate::routes::chatwoot::create_contact,
        crate::routes::chatwoot::create_conversation,
        crate::routes::chatwoot::list_conversations,
        crate::routes::chatwoot::send_message,
        crate::routes::chatwoot::get_messages,
    ),
    components(
        schemas(
            FeedbackRequest,
            FeedbackResponse,
            crate::routes::feedback::FeedbackType,
            crate::routes::feedback::DeviceInfo,
            crate::routes::chatwoot::CreateContactRequest,
            crate::routes::chatwoot::CreateContactResponse,
            crate::routes::chatwoot::CreateConversationRequest,
            crate::routes::chatwoot::CreateConversationResponse,
            crate::routes::chatwoot::ListConversationsQuery,
            crate::routes::chatwoot::ConversationSummary,
            crate::routes::chatwoot::SendMessageRequest,
            crate::routes::chatwoot::MessageResponse,
        )
    ),
    tags(
        (name = "support", description = "User feedback and support"),
        (name = "chatwoot", description = "Chatwoot conversation persistence")
    )
)]
struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
