use std::collections::HashSet;

use axum::{
    Extension, Json, Router,
    extract::State,
    http::{HeaderValue, header},
    routing::post,
};
use chrono::{SecondsFormat, TimeDelta, Utc};
use hypr_api_auth::AuthContext;
use serde::{Deserialize, Serialize};
use utoipa::OpenApi;

use crate::error::{Result, SyncError};
use crate::state::AppState;

const WORKSPACE_PROJECTION_SELECT: &str = "id,user_id,role,created_at,updated_at,workspace:workspaces!inner(id,owner_user_id,kind,name,created_at,updated_at)";
const WORKSPACE_PROJECTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const MAX_TOKEN_WORKSPACES: usize = 128;
const MAX_TOKEN_ATTRIBUTES_BYTES: usize = 8 * 1024;

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CloudsyncCredentials {
    database_id: String,
    token: String,
    expires_at: String,
    workspace_id: String,
    account_user_id: String,
    personal_workspace_id: String,
    workspaces: Vec<CloudsyncWorkspace>,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CloudsyncWorkspace {
    id: String,
    owner_user_id: String,
    kind: String,
    name: String,
    membership_id: String,
    role: String,
    membership_created_at: String,
    membership_updated_at: String,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateTokenRequest<'a> {
    name: &'static str,
    user_id: &'a str,
    expires_at: &'a str,
    attributes: &'a str,
}

#[derive(Deserialize)]
struct CreateTokenEnvelope {
    data: CreateTokenResponse,
}

#[derive(Deserialize)]
struct CreateTokenResponse {
    token: String,
}

#[derive(Deserialize)]
struct WorkspaceMembershipRow {
    id: String,
    user_id: String,
    role: String,
    created_at: String,
    updated_at: String,
    workspace: WorkspaceRow,
}

#[derive(Deserialize)]
struct WorkspaceRow {
    id: String,
    owner_user_id: String,
    kind: String,
    name: String,
    created_at: String,
    updated_at: String,
}

#[derive(OpenApi)]
#[openapi(
    paths(create_credentials),
    components(schemas(CloudsyncCredentials, CloudsyncWorkspace))
)]
pub struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/token", post(create_credentials))
        .with_state(state)
}

#[utoipa::path(
    post,
    path = "/token",
    tag = "sync",
    responses(
        (status = 200, description = "Short-lived CloudSync credentials", body = CloudsyncCredentials),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Anarlog Pro subscription required"),
        (status = 502, description = "Credential issuer unavailable")
    )
)]
async fn create_credentials(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> Result<(
    [(header::HeaderName, HeaderValue); 1],
    Json<CloudsyncCredentials>,
)> {
    if !auth.claims.is_pro() {
        return Err(SyncError::ProPlanRequired);
    }

    let workspace_rows = fetch_workspace_projection(&state, &auth).await?;
    let (personal_workspace_id, workspaces) =
        validate_workspace_projection(workspace_rows, &auth.claims.sub)?;
    let token_attributes = encode_workspace_token_attributes(&workspaces)?;

    let ttl = i64::try_from(state.config.token_ttl_seconds)
        .map_err(|_| SyncError::Internal("CloudSync token TTL is too large".to_string()))?;
    let ttl = TimeDelta::try_seconds(ttl)
        .ok_or_else(|| SyncError::Internal("CloudSync token TTL is too large".to_string()))?;
    let expires_at = Utc::now()
        .checked_add_signed(ttl)
        .ok_or_else(|| SyncError::Internal("CloudSync token expiry is invalid".to_string()))?
        .to_rfc3339_opts(SecondsFormat::Secs, true);
    let response = state
        .client
        .post(format!("{}/v2/tokens", state.config.project_url))
        .bearer_auth(&state.config.token_issuer_api_key)
        .json(&CreateTokenRequest {
            name: "anarlog-cloudsync",
            user_id: &auth.claims.sub,
            expires_at: &expires_at,
            attributes: &token_attributes,
        })
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "SQLite Cloud token request failed");
            SyncError::Upstream
        })?;
    if !response.status().is_success() {
        tracing::warn!(status = %response.status(), "SQLite Cloud token request was rejected");
        return Err(SyncError::Upstream);
    }
    let response: CreateTokenEnvelope = response.json().await.map_err(|error| {
        tracing::warn!(%error, "SQLite Cloud token response was invalid");
        SyncError::Upstream
    })?;
    if response.data.token.trim().is_empty() {
        return Err(SyncError::Upstream);
    }

    Ok((
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(CloudsyncCredentials {
            database_id: state.config.database_id,
            token: response.data.token,
            expires_at,
            workspace_id: personal_workspace_id.clone(),
            account_user_id: auth.claims.sub,
            personal_workspace_id,
            workspaces,
        }),
    ))
}

async fn fetch_workspace_projection(
    state: &AppState,
    auth: &AuthContext,
) -> Result<Vec<WorkspaceMembershipRow>> {
    let user_filter = format!("eq.{}", auth.claims.sub);
    let response = state
        .client
        .get(format!(
            "{}/rest/v1/workspace_memberships",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_anon_key)
        .bearer_auth(&auth.token)
        .timeout(WORKSPACE_PROJECTION_TIMEOUT)
        .query(&[
            ("select", WORKSPACE_PROJECTION_SELECT),
            ("user_id", user_filter.as_str()),
            ("deleted_at", "is.null"),
            ("workspace.deleted_at", "is.null"),
        ])
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase workspace projection request failed");
            SyncError::Upstream
        })?;
    if !response.status().is_success() {
        tracing::warn!(
            status = %response.status(),
            "Supabase workspace projection request was rejected"
        );
        return Err(SyncError::Upstream);
    }

    response.json().await.map_err(|error| {
        tracing::warn!(%error, "Supabase workspace projection response was invalid");
        SyncError::Upstream
    })
}

fn validate_workspace_projection(
    mut rows: Vec<WorkspaceMembershipRow>,
    account_user_id: &str,
) -> Result<(String, Vec<CloudsyncWorkspace>)> {
    let mut membership_ids = HashSet::with_capacity(rows.len());
    let mut workspace_ids = HashSet::with_capacity(rows.len());

    for row in &rows {
        if row.user_id != account_user_id {
            return invalid_workspace_projection("membership user does not match account");
        }
        if row.id.trim().is_empty()
            || row.workspace.id.trim().is_empty()
            || row.workspace.owner_user_id.trim().is_empty()
        {
            return invalid_workspace_projection("workspace projection contains a blank identity");
        }
        if !matches!(row.role.as_str(), "owner" | "admin" | "member") {
            return invalid_workspace_projection("workspace membership has an invalid role");
        }
        if !matches!(row.workspace.kind.as_str(), "personal" | "shared") {
            return invalid_workspace_projection("workspace has an invalid kind");
        }
        if chrono::DateTime::parse_from_rfc3339(&row.workspace.created_at).is_err()
            || chrono::DateTime::parse_from_rfc3339(&row.workspace.updated_at).is_err()
        {
            return invalid_workspace_projection("workspace has an invalid timestamp");
        }
        if chrono::DateTime::parse_from_rfc3339(&row.created_at).is_err()
            || chrono::DateTime::parse_from_rfc3339(&row.updated_at).is_err()
        {
            return invalid_workspace_projection("workspace membership has an invalid timestamp");
        }
        if !membership_ids.insert(&row.id) || !workspace_ids.insert(&row.workspace.id) {
            return invalid_workspace_projection("workspace projection contains duplicate rows");
        }
    }

    let personal_workspaces = rows
        .iter()
        .filter(|row| row.workspace.kind == "personal")
        .collect::<Vec<_>>();
    if personal_workspaces.len() != 1 {
        return invalid_workspace_projection(
            "account must have exactly one personal owner workspace",
        );
    }

    let personal_workspace = personal_workspaces[0];
    if personal_workspace.role != "owner"
        || personal_workspace.workspace.id != account_user_id
        || personal_workspace.workspace.owner_user_id != account_user_id
    {
        return invalid_workspace_projection("personal workspace identity does not match account");
    }

    let personal_workspace_id = personal_workspace.workspace.id.clone();
    rows.sort_by(|left, right| {
        let left_is_personal = left.workspace.id == personal_workspace_id;
        let right_is_personal = right.workspace.id == personal_workspace_id;
        right_is_personal
            .cmp(&left_is_personal)
            .then_with(|| left.workspace.created_at.cmp(&right.workspace.created_at))
            .then_with(|| left.workspace.id.cmp(&right.workspace.id))
    });

    Ok((
        personal_workspace_id,
        rows.into_iter()
            .map(|row| CloudsyncWorkspace {
                id: row.workspace.id,
                owner_user_id: row.workspace.owner_user_id,
                kind: row.workspace.kind,
                name: row.workspace.name,
                membership_id: row.id,
                role: row.role,
                membership_created_at: row.created_at,
                membership_updated_at: row.updated_at,
                created_at: row.workspace.created_at,
                updated_at: row.workspace.updated_at,
            })
            .collect(),
    ))
}

fn invalid_workspace_projection<T>(reason: &'static str) -> Result<T> {
    tracing::warn!(reason, "Supabase workspace projection failed validation");
    Err(SyncError::Upstream)
}

fn encode_workspace_token_attributes(workspaces: &[CloudsyncWorkspace]) -> Result<String> {
    if workspaces.len() > MAX_TOKEN_WORKSPACES {
        tracing::warn!(
            workspace_count = workspaces.len(),
            "CloudSync workspace projection exceeds token limit"
        );
        return Err(SyncError::Upstream);
    }

    let attributes = serde_json::to_string(&serde_json::json!({
        "workspace_ids": workspaces
            .iter()
            .map(|workspace| workspace.id.as_str())
            .collect::<Vec<_>>(),
    }))
    .map_err(|error| {
        SyncError::Internal(format!("CloudSync token attributes are invalid: {error}"))
    })?;
    if attributes.len() > MAX_TOKEN_ATTRIBUTES_BYTES {
        tracing::warn!(
            attribute_bytes = attributes.len(),
            "CloudSync workspace projection exceeds token size limit"
        );
        return Err(SyncError::Upstream);
    }

    Ok(attributes)
}

#[cfg(test)]
mod tests {
    use axum::{Extension, body::Body, body::to_bytes, http::Request, http::StatusCode};
    use hypr_api_auth::{AuthContext, Claims};
    use serde_json::{Value, json};
    use tower::ServiceExt;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_partial_json, header, method, path, query_param},
    };

    use super::*;
    use crate::SyncConfig;

    fn test_router(server: &MockServer, api_key: &str, entitlements: &[&str]) -> Router {
        router(AppState::new(SyncConfig {
            project_url: server.uri(),
            token_issuer_api_key: api_key.to_string(),
            database_id: "database-id".to_string(),
            token_ttl_seconds: 60,
            supabase_url: server.uri(),
            supabase_anon_key: "anon-key".to_string(),
        }))
        .layer(Extension(AuthContext {
            token: "supabase-token".to_string(),
            claims: Claims {
                sub: "user-123".to_string(),
                email: None,
                entitlements: entitlements
                    .iter()
                    .map(|entitlement| (*entitlement).to_string())
                    .collect(),
                subscription_status: None,
                trial_end: None,
                has_payment_method: None,
            },
        }))
    }

    fn personal_workspace(id: &str) -> Value {
        json!({
            "id": id,
            "user_id": "user-123",
            "role": "owner",
            "created_at": "2026-07-16T08:01:00Z",
            "updated_at": "2026-07-16T08:02:00Z",
            "workspace": {
                "id": id,
                "owner_user_id": id,
                "kind": "personal",
                "name": "Personal",
                "created_at": "2026-07-16T08:00:00Z",
                "updated_at": "2026-07-16T08:00:00Z"
            }
        })
    }

    async fn mock_workspace_projection(server: &MockServer, body: Value) {
        Mock::given(method("GET"))
            .and(path("/rest/v1/workspace_memberships"))
            .and(header("apikey", "anon-key"))
            .and(header("authorization", "Bearer supabase-token"))
            .and(query_param("select", WORKSPACE_PROJECTION_SELECT))
            .and(query_param("user_id", "eq.user-123"))
            .and(query_param("deleted_at", "is.null"))
            .and(query_param("workspace.deleted_at", "is.null"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(server)
            .await;
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn mints_token_for_verified_supabase_subject() {
        let server = MockServer::start().await;
        mock_workspace_projection(
            &server,
            json!([
                {
                    "id": "membership-team",
                    "user_id": "user-123",
                    "role": "member",
                    "created_at": "2026-07-16T09:01:00Z",
                    "updated_at": "2026-07-16T10:01:00Z",
                    "workspace": {
                        "id": "workspace-team",
                        "owner_user_id": "user-456",
                        "kind": "shared",
                        "name": "Acme",
                        "created_at": "2026-07-16T09:00:00Z",
                        "updated_at": "2026-07-16T10:00:00Z"
                    }
                },
                personal_workspace("user-123")
            ]),
        )
        .await;
        Mock::given(method("POST"))
            .and(path("/v2/tokens"))
            .and(header("authorization", "Bearer issuer-key"))
            .and(body_partial_json(json!({
                "name": "anarlog-cloudsync",
                "userId": "user-123"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": { "token": "sqlite-token" }
            })))
            .mount(&server)
            .await;

        let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
            .oneshot(Request::post("/token").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body = response_json(response).await;
        assert_eq!(body["databaseId"], "database-id");
        assert_eq!(body["token"], "sqlite-token");
        assert_eq!(body["workspaceId"], "user-123");
        assert_eq!(body["accountUserId"], "user-123");
        assert_eq!(body["personalWorkspaceId"], "user-123");
        assert_eq!(body["workspaces"][0]["id"], "user-123");
        assert_eq!(body["workspaces"][0]["membershipId"], "user-123");
        assert_eq!(body["workspaces"][0]["role"], "owner");
        assert_eq!(body["workspaces"][1]["id"], "workspace-team");
        assert_eq!(body["workspaces"][1]["ownerUserId"], "user-456");
        assert_eq!(body["workspaces"][1]["kind"], "shared");
        assert_eq!(body["workspaces"][1]["name"], "Acme");
        assert_eq!(
            body["workspaces"][1]["membershipCreatedAt"],
            "2026-07-16T09:01:00Z"
        );
        assert_eq!(
            body["workspaces"][1]["membershipUpdatedAt"],
            "2026-07-16T10:01:00Z"
        );
        assert_eq!(body["workspaces"][1]["createdAt"], "2026-07-16T09:00:00Z");
        assert_eq!(body["workspaces"][1]["updatedAt"], "2026-07-16T10:00:00Z");
        assert!(body["expiresAt"].as_str().unwrap().ends_with('Z'));

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests[0].url.path(), "/rest/v1/workspace_memberships");
        assert_eq!(requests[1].url.path(), "/v2/tokens");
        let token_request: Value = serde_json::from_slice(&requests[1].body).unwrap();
        assert_eq!(token_request.as_object().unwrap().len(), 4);
        assert_eq!(token_request["userId"], "user-123");
        let attributes: Value =
            serde_json::from_str(token_request["attributes"].as_str().unwrap()).unwrap();
        assert_eq!(
            attributes,
            json!({ "workspace_ids": ["user-123", "workspace-team"] })
        );
        assert!(token_request.get("workspaceId").is_none());
        assert!(token_request.get("workspaceIds").is_none());
    }

    #[test]
    fn bounds_workspace_token_attributes() {
        let workspace = |id: String| CloudsyncWorkspace {
            id,
            owner_user_id: "user-123".to_string(),
            kind: "shared".to_string(),
            name: "Shared".to_string(),
            membership_id: "membership".to_string(),
            role: "member".to_string(),
            membership_created_at: "2026-07-16T08:00:00Z".to_string(),
            membership_updated_at: "2026-07-16T08:00:00Z".to_string(),
            created_at: "2026-07-16T08:00:00Z".to_string(),
            updated_at: "2026-07-16T08:00:00Z".to_string(),
        };

        let too_many = (0..=MAX_TOKEN_WORKSPACES)
            .map(|index| workspace(format!("workspace-{index}")))
            .collect::<Vec<_>>();
        assert!(matches!(
            encode_workspace_token_attributes(&too_many),
            Err(SyncError::Upstream)
        ));

        let oversized = [workspace("x".repeat(MAX_TOKEN_ATTRIBUTES_BYTES))];
        assert!(matches!(
            encode_workspace_token_attributes(&oversized),
            Err(SyncError::Upstream)
        ));
    }

    #[tokio::test]
    async fn rejects_users_without_pro_entitlement() {
        let cases: &[&[&str]] = &[&[], &["hyprnote_lite"]];

        for entitlements in cases {
            let server = MockServer::start().await;
            let response = test_router(&server, "issuer-key", entitlements)
                .oneshot(Request::post("/token").body(Body::empty()).unwrap())
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::FORBIDDEN);
            let body = response_json(response).await;
            assert_eq!(body["error"]["code"], "subscription_required");
            assert!(server.received_requests().await.unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn refuses_token_when_workspace_projection_is_invalid() {
        let invalid_projections = [
            json!([]),
            json!([personal_workspace("different-user")]),
            json!([
                personal_workspace("user-123"),
                {
                    "id": "other-owner-membership",
                    "user_id": "user-123",
                    "role": "owner",
                    "created_at": "2026-07-16T09:00:00Z",
                    "updated_at": "2026-07-16T09:00:00Z",
                    "workspace": {
                        "id": "other-personal",
                        "owner_user_id": "other-personal",
                        "kind": "personal",
                        "name": "Other",
                        "created_at": "2026-07-16T09:00:00Z",
                        "updated_at": "2026-07-16T09:00:00Z"
                    }
                }
            ]),
            json!([{
                "id": "user-123",
                "user_id": "user-123",
                "role": "admin",
                "created_at": "2026-07-16T08:00:00Z",
                "updated_at": "2026-07-16T08:00:00Z",
                "workspace": {
                    "id": "user-123",
                    "owner_user_id": "user-123",
                    "kind": "personal",
                    "name": "Personal",
                    "created_at": "2026-07-16T08:00:00Z",
                    "updated_at": "2026-07-16T08:00:00Z"
                }
            }]),
            json!([
                personal_workspace("user-123"),
                {
                    "id": "",
                    "user_id": "user-123",
                    "role": "member",
                    "created_at": "2026-07-16T09:00:00Z",
                    "updated_at": "2026-07-16T09:00:00Z",
                    "workspace": {
                        "id": "workspace-team",
                        "owner_user_id": "user-456",
                        "kind": "shared",
                        "name": "Acme",
                        "created_at": "2026-07-16T09:00:00Z",
                        "updated_at": "2026-07-16T09:00:00Z"
                    }
                }
            ]),
            json!([
                personal_workspace("user-123"),
                {
                    "id": "membership-team",
                    "user_id": "user-123",
                    "role": "editor",
                    "created_at": "2026-07-16T09:00:00Z",
                    "updated_at": "2026-07-16T09:00:00Z",
                    "workspace": {
                        "id": "workspace-team",
                        "owner_user_id": "user-456",
                        "kind": "shared",
                        "name": "Acme",
                        "created_at": "2026-07-16T09:00:00Z",
                        "updated_at": "2026-07-16T09:00:00Z"
                    }
                }
            ]),
            json!([
                personal_workspace("user-123"),
                {
                    "id": "membership-team",
                    "user_id": "user-123",
                    "role": "member",
                    "created_at": "2026-07-16T09:00:00Z",
                    "updated_at": "2026-07-16T09:00:00Z",
                    "workspace": {
                        "id": "workspace-team",
                        "owner_user_id": "user-456",
                        "kind": "team",
                        "name": "Acme",
                        "created_at": "2026-07-16T09:00:00Z",
                        "updated_at": "2026-07-16T09:00:00Z"
                    }
                }
            ]),
            json!([
                personal_workspace("user-123"),
                {
                    "id": "membership-team",
                    "user_id": "user-123",
                    "role": "member",
                    "created_at": "2026-07-16T09:00:00Z",
                    "updated_at": "2026-07-16T09:00:00Z",
                    "workspace": {
                        "id": "workspace-team",
                        "owner_user_id": "user-456",
                        "kind": "shared",
                        "name": "Acme",
                        "created_at": "not-a-timestamp",
                        "updated_at": "2026-07-16T09:00:00Z"
                    }
                }
            ]),
            json!([
                personal_workspace("user-123"),
                {
                    "id": "membership-team",
                    "user_id": "user-123",
                    "role": "member",
                    "created_at": "not-a-timestamp",
                    "updated_at": "2026-07-16T09:00:00Z",
                    "workspace": {
                        "id": "workspace-team",
                        "owner_user_id": "user-456",
                        "kind": "shared",
                        "name": "Acme",
                        "created_at": "2026-07-16T09:00:00Z",
                        "updated_at": "2026-07-16T09:00:00Z"
                    }
                }
            ]),
        ];

        for projection in invalid_projections {
            let server = MockServer::start().await;
            mock_workspace_projection(&server, projection).await;

            let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
                .oneshot(Request::post("/token").body(Body::empty()).unwrap())
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
            let requests = server.received_requests().await.unwrap();
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].url.path(), "/rest/v1/workspace_memberships");
        }
    }

    #[tokio::test]
    async fn workspace_projection_failure_is_redacted() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/v1/workspace_memberships"))
            .respond_with(ResponseTemplate::new(403).set_body_string("secret supabase detail"))
            .mount(&server)
            .await;

        let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
            .oneshot(Request::post("/token").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let body = response_json(response).await.to_string();
        assert!(!body.contains("anon-key"));
        assert!(!body.contains("supabase-token"));
        assert!(!body.contains("supabase detail"));
    }

    #[tokio::test]
    async fn sqlite_cloud_failure_is_redacted() {
        let server = MockServer::start().await;
        mock_workspace_projection(&server, json!([personal_workspace("user-123")])).await;
        Mock::given(method("POST"))
            .and(path("/v2/tokens"))
            .respond_with(ResponseTemplate::new(403).set_body_string("secret upstream detail"))
            .mount(&server)
            .await;

        let response = test_router(&server, "issuer-secret", &["hyprnote_pro"])
            .oneshot(Request::post("/token").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let body = response_json(response).await.to_string();
        assert!(!body.contains("issuer-secret"));
        assert!(!body.contains("upstream detail"));
    }
}
