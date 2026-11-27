use utoipa::OpenApi;

use crate::handler::ModelsResponse;
use crate::model::{CharTask, SelectableModel};

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

#[utoipa::path(
    get,
    path = "/llm/models",
    operation_id = "llm_list_models",
    responses(
        (status = 200, description = "Available hosted LLM models", body = ModelsResponse),
        (status = 401, description = "Unauthorized"),
    ),
    tag = "llm",
)]
async fn _models_spec() {}

#[derive(OpenApi)]
#[openapi(
    paths(_chat_completions_spec, _models_spec),
    components(schemas(CharTask, ModelsResponse, SelectableModel)),
    tags((name = "llm", description = "LLM chat completions proxy"))
)]
pub struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
