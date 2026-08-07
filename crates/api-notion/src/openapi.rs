use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::notion::search_pages,
        crate::routes::notion::append_update,
    ),
    components(schemas(
        crate::routes::notion::NotionSearchPagesRequest,
        crate::routes::notion::NotionPage,
        crate::routes::notion::NotionPagesResponse,
        crate::routes::notion::NotionAppendUpdateRequest,
        crate::routes::notion::NotionAppendUpdateResponse,
    )),
    tags(
        (name = "notion", description = "Notion integration")
    )
)]
struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
