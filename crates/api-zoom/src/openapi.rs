use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(crate::routes::import_meetings),
    components(schemas(
        crate::routes::ZoomImportMeetingsRequest,
        crate::routes::ZoomImportMeetingsResponse,
        crate::routes::ZoomImportTextFile,
    )),
    tags((name = "zoom", description = "Zoom meeting import"))
)]
struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
