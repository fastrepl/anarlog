use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::search_contacts,
        crate::routes::contact_context,
        crate::routes::log_note,
    ),
    components(schemas(
        crate::routes::CrmSearchContactsRequest,
        crate::routes::CrmSearchContactsResponse,
        crate::routes::CrmContactContextRequest,
        crate::routes::CrmContactContextResponse,
        crate::routes::CrmLogNoteRequest,
        crate::routes::CrmLogNoteResponse,
        crate::contacts::CrmContact,
        crate::contacts::CrmDeal,
        crate::contacts::CrmNote,
        crate::contacts::CrmLoggedNote,
    )),
    tags((name = "crm", description = "CRM contact lookup, context, and note logging"))
)]
struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
