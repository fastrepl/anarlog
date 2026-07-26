use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::github::routes::list_repos,
        crate::github::routes::list_tickets,
        crate::linear::routes::list_teams,
        crate::linear::routes::list_tickets,
    ),
    components(schemas(
        crate::github::routes::GitHubListReposRequest,
        crate::github::routes::GitHubListTicketsRequest,
        crate::linear::routes::LinearListTeamsRequest,
        crate::linear::routes::LinearListTicketsRequest,
    )),
    tags(
        (name = "ticket", description = "Ticket management")
    )
)]
struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
