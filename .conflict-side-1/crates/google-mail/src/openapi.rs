use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::Attachment,
    crate::GetMessageRequest,
    crate::GetThreadRequest,
    crate::History,
    crate::HistoryLabelAdded,
    crate::HistoryLabelRemoved,
    crate::HistoryMessageAdded,
    crate::HistoryMessageDeleted,
    crate::HistoryType,
    crate::Label,
    crate::LabelColor,
    crate::LabelListVisibility,
    crate::LabelType,
    crate::ListHistoryRequest,
    crate::ListHistoryResponse,
    crate::ListLabelsResponse,
    crate::ListMessagesRequest,
    crate::ListMessagesResponse,
    crate::ListThreadsRequest,
    crate::ListThreadsResponse,
    crate::Message,
    crate::MessageFormat,
    crate::MessageListVisibility,
    crate::MessagePart,
    crate::MessagePartBody,
    crate::MessagePartHeader,
    crate::MessageRef,
    crate::Profile,
    crate::Thread,
    crate::ThreadRef,
)))]
struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
