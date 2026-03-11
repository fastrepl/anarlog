use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::google::routes::list_calendars,
        crate::google::routes::list_events,
        crate::outlook::routes::list_calendars,
        crate::outlook::routes::list_events,
    ),
    components(schemas(
        crate::google::routes::GoogleListEventsRequest,
        crate::outlook::routes::OutlookListEventsRequest,
    )),
    tags(
        (name = "calendar", description = "Calendar management")
    )
)]
struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    let mut doc = ApiDoc::openapi();
    doc.merge(hypr_google_calendar::openapi::openapi());
    doc.merge(hypr_outlook_calendar::openapi::openapi());
    doc
}
