use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::notion::search_pages,
        crate::routes::notion::append_update,
        crate::routes::notion::import_meetings,
    ),
    components(schemas(
        crate::routes::notion::NotionSearchPagesRequest,
        crate::routes::notion::NotionPage,
        crate::routes::notion::NotionPagesResponse,
        crate::routes::notion::NotionAppendUpdateRequest,
        crate::routes::notion::NotionAppendUpdateResponse,
        crate::routes::notion::NotionImportMeetingsRequest,
        crate::routes::notion::NotionImportMeetingsResponse,
        crate::routes::notion::NotionImportTextFile,
    )),
    tags(
        (name = "notion", description = "Notion integration")
    )
)]
struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
