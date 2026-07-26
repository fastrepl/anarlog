use owhisper_interface::openapi::{CommonListenParams, StreamListenParams};
use utoipa::OpenApi;

#[derive(utoipa::ToSchema)]
#[schema(format = "binary")]
struct BatchAudioBody(#[allow(dead_code)] Vec<u8>);

#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
enum BatchListenSuccessResponse {
    Sync(owhisper_interface::batch::Response),
    Callback(crate::routes::batch::async_callback::ListenCallbackResponse),
}

#[utoipa::path(
    post,
    path = "/stt/listen",
    operation_id = "stt_listen_batch",
    params(
        CommonListenParams,
        ("callback" = Option<String>, Query, description = "When set, enables async callback mode. Body should be JSON with a `url` field instead of raw audio"),
    ),
    request_body(
        description = "Raw audio bytes (sync mode) or JSON `{ \"url\": \"<file_id>\" }` (callback mode)",
        content(
            (inline(BatchAudioBody) = "application/octet-stream"),
            (crate::routes::batch::async_callback::ListenCallbackRequest = "application/json"),
        )
    ),
    responses(
        (status = 200, description = "Transcription result (sync) or ListenCallbackResponse (callback)", body = BatchListenSuccessResponse, content_type = "application/json"),
        (status = 400, description = "Bad request (empty body, invalid provider, etc.)"),
        (status = 401, description = "Unauthorized"),
        (status = 502, description = "All upstream providers failed"),
    ),
    tag = "stt",
)]
async fn _batch_spec() {}

#[utoipa::path(
    get,
    path = "/stt/listen",
    operation_id = "stt_listen_stream",
    params(CommonListenParams, StreamListenParams),
    responses(
        (status = 101, description = "WebSocket upgrade. Server sends StreamResponse JSON messages."),
        (status = 400, description = "Bad request (invalid provider, missing params, etc.)"),
        (status = 502, description = "Upstream provider connection failed"),
    ),
    tag = "stt",
)]
async fn _stream_spec() {}

#[derive(OpenApi)]
#[openapi(
    paths(
        _batch_spec,
        _stream_spec,
        crate::routes::status::handler,
    ),
    components(schemas(
        crate::routes::batch::async_callback::ListenCallbackRequest,
        crate::routes::batch::async_callback::ListenCallbackResponse,
        crate::routes::status::SttStatusResponse,
    )),
    tags((name = "stt", description = "Speech-to-text transcription proxy"))
)]
pub struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    let mut doc = ApiDoc::openapi();
    doc.merge(owhisper_interface::openapi());
    doc
}
