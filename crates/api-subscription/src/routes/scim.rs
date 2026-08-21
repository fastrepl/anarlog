use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ServiceProviderConfig", get(service_provider_config))
        .route("/Users", post(create_or_activate_user))
        .route("/Users/{user_id}", patch(patch_user).put(replace_user))
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let token = value.strip_prefix("Bearer ")?;
    if token.len() < 32 || token.len() > 512 || token.chars().any(char::is_control) {
        return None;
    }
    Some(token.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScimUserRequest {
    #[serde(default)]
    user_name: Option<String>,
    #[serde(default)]
    active: Option<bool>,
    #[serde(default)]
    emails: Vec<ScimEmail>,
}

#[derive(Debug, Deserialize)]
struct ScimEmail {
    #[serde(default)]
    value: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ScimPatchRequest {
    #[serde(default, alias = "Operations")]
    operations: Vec<ScimPatchOp>,
}

#[derive(Debug, Deserialize)]
struct ScimPatchOp {
    #[serde(default)]
    op: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    value: Value,
}

#[derive(Debug, Deserialize)]
struct ScimApplyRow {
    user_id: String,
    #[allow(dead_code)]
    workspace_id: String,
    active: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScimUserResource {
    schemas: [&'static str; 1],
    id: String,
    user_name: String,
    active: bool,
}

fn scim_error(status: StatusCode, detail: &str) -> Response {
    (
        status,
        Json(json!({
            "schemas": ["urn:ietf:params:scim:api:messages:2.0:Error"],
            "status": status.as_u16().to_string(),
            "detail": detail,
        })),
    )
        .into_response()
}

fn email_from(request: &ScimUserRequest) -> Option<String> {
    request
        .emails
        .iter()
        .find_map(|email| email.value.clone())
        .or_else(|| request.user_name.clone())
        .map(|value| value.trim().to_string())
        .filter(|value| value.contains('@') && value.len() <= 320)
}

fn active_from_patch(request: &ScimPatchRequest) -> Option<bool> {
    request.operations.iter().find_map(|operation| {
        if !operation.op.eq_ignore_ascii_case("replace") {
            return None;
        }
        let path = operation.path.as_deref().unwrap_or("active");
        if path != "active" && !path.ends_with(":active") {
            return None;
        }
        operation.value.as_bool()
    })
}

fn resource(email: &str, row: ScimApplyRow) -> ScimUserResource {
    ScimUserResource {
        schemas: ["urn:ietf:params:scim:schemas:core:2.0:User"],
        id: row.user_id,
        user_name: email.to_string(),
        active: row.active,
    }
}

fn map_apply_error(error: crate::error::SubscriptionError) -> Response {
    let message = error.to_string();
    if message.contains("42501") || message.contains("invalid scim token") {
        scim_error(StatusCode::UNAUTHORIZED, "invalid bearer token")
    } else if message.contains("P0002") || message.contains("scim user not found") {
        scim_error(StatusCode::NOT_FOUND, "user not found")
    } else {
        tracing::error!(error = %error, "scim_apply_user_failed");
        scim_error(StatusCode::BAD_GATEWAY, "directory update failed")
    }
}

async fn apply_email(
    state: &AppState,
    token: &str,
    email: &str,
    active: bool,
) -> Result<ScimApplyRow, Response> {
    let rows: Vec<ScimApplyRow> = state
        .supabase
        .admin_rpc(
            "scim_apply_user",
            &json!({
                "p_token": token,
                "p_email": email,
                "p_active": active,
            }),
        )
        .await
        .map_err(map_apply_error)?;
    rows.into_iter()
        .next()
        .ok_or_else(|| scim_error(StatusCode::BAD_GATEWAY, "directory update failed"))
}

async fn apply_user_id(
    state: &AppState,
    token: &str,
    user_id: &str,
    active: bool,
) -> Result<ScimApplyRow, Response> {
    let rows: Vec<ScimApplyRow> = state
        .supabase
        .admin_rpc(
            "scim_apply_user_id",
            &json!({
                "p_token": token,
                "p_user_id": user_id,
                "p_active": active,
            }),
        )
        .await
        .map_err(map_apply_error)?;
    rows.into_iter()
        .next()
        .ok_or_else(|| scim_error(StatusCode::BAD_GATEWAY, "directory update failed"))
}

async fn service_provider_config() -> Json<Value> {
    Json(json!({
        "schemas": ["urn:ietf:params:scim:schemas:core:2.0:ServiceProviderConfig"],
        "patch": { "supported": true },
        "bulk": { "supported": false, "maxOperations": 0, "maxPayloadSize": 0 },
        "filter": { "supported": false, "maxResults": 0 },
        "changePassword": { "supported": false },
        "sort": { "supported": false },
        "etag": { "supported": false },
        "authenticationSchemes": [{
            "type": "oauthbearertoken",
            "name": "OAuth Bearer Token",
            "description": "Authentication scheme using the OAuth Bearer Token Standard",
            "specUri": "https://www.rfc-editor.org/rfc/rfc6750.html",
            "primary": true
        }]
    }))
}

async fn create_or_activate_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ScimUserRequest>,
) -> Response {
    let Some(token) = bearer_token(&headers) else {
        return scim_error(StatusCode::UNAUTHORIZED, "invalid bearer token");
    };
    let Some(email) = email_from(&request) else {
        return scim_error(
            StatusCode::BAD_REQUEST,
            "userName or emails.value is required",
        );
    };
    match apply_email(&state, &token, &email, request.active.unwrap_or(true)).await {
        Ok(row) => (StatusCode::CREATED, Json(resource(&email, row))).into_response(),
        Err(response) => response,
    }
}

async fn patch_user(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ScimPatchRequest>,
) -> Response {
    let Some(token) = bearer_token(&headers) else {
        return scim_error(StatusCode::UNAUTHORIZED, "invalid bearer token");
    };
    let Some(active) = active_from_patch(&request) else {
        return scim_error(
            StatusCode::BAD_REQUEST,
            "active replace operation is required",
        );
    };
    let result = if user_id.contains('@') {
        apply_email(&state, &token, &user_id, active).await
    } else {
        apply_user_id(&state, &token, &user_id, active).await
    };
    match result {
        Ok(row) => Json(resource(&user_id, row)).into_response(),
        Err(response) => response,
    }
}

async fn replace_user(
    State(state): State<AppState>,
    Path(_user_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ScimUserRequest>,
) -> Response {
    let Some(token) = bearer_token(&headers) else {
        return scim_error(StatusCode::UNAUTHORIZED, "invalid bearer token");
    };
    let Some(email) = email_from(&request) else {
        return scim_error(
            StatusCode::BAD_REQUEST,
            "userName or emails.value is required",
        );
    };
    match apply_email(&state, &token, &email, request.active.unwrap_or(true)).await {
        Ok(row) => Json(resource(&email, row)).into_response(),
        Err(response) => response,
    }
}
