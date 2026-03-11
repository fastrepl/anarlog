use utoipa::OpenApi;

use crate::model::CharTask;

#[utoipa::path(
    post,
    path = "/llm/chat/completions",
    operation_id = "llm_chat_completions",
    params(
        ("x-char-task" = Option<CharTask>, Header, description = "Task type for model selection"),
    ),
    responses(
        (status = 200, description = "Chat completion response (streaming or non-streaming)"),
        (status = 401, description = "Unauthorized"),
        (status = 429, description = "Rate limit exceeded"),
        (status = 502, description = "Upstream provider failed"),
        (status = 504, description = "Request timeout"),
    ),
    tag = "llm",
)]
async fn _chat_completions_spec() {}

#[derive(OpenApi)]
#[openapi(
    paths(_chat_completions_spec),
    components(schemas(CharTask)),
    tags((name = "llm", description = "LLM chat completions proxy"))
)]
pub struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
