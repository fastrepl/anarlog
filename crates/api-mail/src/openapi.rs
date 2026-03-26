use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::google::routes::list_labels,
        crate::google::routes::list_messages,
        crate::google::routes::get_message,
        crate::google::routes::get_attachment,
        crate::google::routes::get_profile,
        crate::google::routes::list_threads,
        crate::google::routes::get_thread,
        crate::google::routes::list_history,
    ),
    components(schemas(
        crate::google::routes::GoogleListLabelsRequest,
        crate::google::routes::GoogleListMessagesRequest,
        crate::google::routes::GoogleGetMessageRequest,
        crate::google::routes::GoogleGetAttachmentRequest,
        crate::google::routes::GoogleGetProfileRequest,
        crate::google::routes::GoogleListThreadsRequest,
        crate::google::routes::GoogleGetThreadRequest,
        crate::google::routes::GoogleListHistoryRequest,
    )),
    tags(
        (name = "mail", description = "Mail management")
    )
)]
struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    let mut doc = ApiDoc::openapi();
    doc.merge(hypr_google_mail::openapi::openapi());
    doc
}
