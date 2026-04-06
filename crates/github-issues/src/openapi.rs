use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::types::Issue,
    crate::types::IssuePullRequest,
    crate::types::User,
    crate::types::Label,
    crate::types::Milestone,
    crate::types::Repository,
)))]
struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
