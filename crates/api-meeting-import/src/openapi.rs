use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::fathom_import_meetings,
        crate::routes::webex_import_meetings,
        crate::routes::google_meet_import_meetings,
        crate::routes::teams_import_meetings,
    ),
    components(schemas(
        crate::routes::ImportMeetingsRequest,
        crate::routes::ImportMeetingsResponse,
        crate::routes::ImportTextFile,
    )),
    tags(
        (name = "fathom", description = "Fathom meeting import"),
        (name = "webex", description = "Webex meeting import"),
        (name = "google-meet", description = "Google Meet meeting import"),
        (name = "microsoft-teams", description = "Microsoft Teams meeting import"),
    )
)]
struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
