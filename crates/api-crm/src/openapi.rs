use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(crate::routes::search_contacts),
    components(schemas(
        crate::routes::CrmSearchContactsRequest,
        crate::routes::CrmSearchContactsResponse,
        crate::contacts::CrmContact,
    )),
    tags((name = "crm", description = "CRM contact lookup"))
)]
struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
