use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;

use crate::state::AppState;

pub(crate) const OAUTH_SCOPES: &[&str] = &["openid", "email"];
pub(crate) const MCP_RESOURCE: &str = "https://api.anarlog.so/mcp";
const OPENAI_APPS_CHALLENGE: &str = "Rueo1ui7LUWfrc8duM53jXRiYcaxExDbCNrsLp2VdvU";

#[derive(Clone)]
pub(crate) struct OAuthResourceConfig {
    resource: String,
    authorization_server: String,
    resource_metadata: String,
}

#[derive(Serialize)]
struct ProtectedResourceMetadata {
    resource: String,
    authorization_servers: [String; 1],
    scopes_supported: &'static [&'static str],
    bearer_methods_supported: [&'static str; 1],
    resource_documentation: &'static str,
}

impl OAuthResourceConfig {
    pub(crate) fn new(resource: &str, supabase_url: &str) -> Result<Self, String> {
        let resource_url = reqwest::Url::parse(resource)
            .map_err(|_| "MCP OAuth resource must be an absolute URL".to_string())?;
        if resource_url.scheme() != "https"
            || resource_url.query().is_some()
            || resource_url.fragment().is_some()
        {
            return Err(
                "MCP OAuth resource must be an HTTPS URL without query or fragment".to_string(),
            );
        }
        let origin = resource_url.origin().ascii_serialization();
        let resource_path = resource_url.path().trim_start_matches('/');
        let resource_metadata = if resource_path.is_empty() {
            format!("{origin}/.well-known/oauth-protected-resource")
        } else {
            format!("{origin}/.well-known/oauth-protected-resource/{resource_path}")
        };

        Ok(Self {
            resource: resource_url.to_string().trim_end_matches('/').to_string(),
            authorization_server: format!("{}/auth/v1", supabase_url.trim_end_matches('/')),
            resource_metadata,
        })
    }

    pub(crate) fn resource(&self) -> &str {
        &self.resource
    }

    pub(crate) fn authorization_server(&self) -> &str {
        &self.authorization_server
    }

    pub(crate) fn challenge(&self, error: Option<(&str, &str)>) -> String {
        let mut challenge = format!(
            "Bearer resource_metadata=\"{}\", scope=\"{}\"",
            self.resource_metadata,
            OAUTH_SCOPES.join(" ")
        );
        if let Some((error, description)) = error {
            challenge.push_str(&format!(
                ", error=\"{error}\", error_description=\"{description}\""
            ));
        }
        challenge
    }
}

pub fn metadata_router(state: AppState) -> Router {
    Router::new()
        .route(
            "/.well-known/oauth-protected-resource",
            get(protected_resource_metadata),
        )
        .route(
            "/.well-known/oauth-protected-resource/mcp",
            get(protected_resource_metadata),
        )
        .route(
            "/.well-known/openai-apps-challenge",
            get(|| async { OPENAI_APPS_CHALLENGE }),
        )
        .with_state(state)
}

async fn protected_resource_metadata(
    State(state): State<AppState>,
) -> Json<ProtectedResourceMetadata> {
    let oauth = state.oauth();
    Json(ProtectedResourceMetadata {
        resource: oauth.resource().to_string(),
        authorization_servers: [oauth.authorization_server().to_string()],
        scopes_supported: OAUTH_SCOPES,
        bearer_methods_supported: ["header"],
        resource_documentation: "https://docs.anarlog.so/reference/api-cloud#remote-mcp",
    })
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
    use serde_json::Value;
    use tower::ServiceExt;

    use super::*;
    use crate::{CloudApiConfig, state::AppState};

    #[tokio::test]
    async fn publishes_protected_resource_metadata_at_root_and_path_locations() {
        let state = AppState::new(
            CloudApiConfig::new("https://auth.example.com", "service-role-key").unwrap(),
        );
        let app = metadata_router(state);

        for path in [
            "/.well-known/oauth-protected-resource",
            "/.well-known/oauth-protected-resource/mcp",
        ] {
            let response = app
                .clone()
                .oneshot(Request::get(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert!(response.status().is_success());
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let metadata: Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(metadata["resource"], "https://api.anarlog.so/mcp");
            assert_eq!(
                metadata["authorization_servers"][0],
                "https://auth.example.com/auth/v1"
            );
            assert_eq!(
                metadata["scopes_supported"],
                serde_json::json!(["openid", "email"])
            );
        }
    }

    #[tokio::test]
    async fn publishes_openai_domain_verification_challenge() {
        let state = AppState::new(
            CloudApiConfig::new("https://auth.example.com", "service-role-key").unwrap(),
        );
        let response = metadata_router(state)
            .oneshot(
                Request::get("/.well-known/openai-apps-challenge")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(response.status().is_success());
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(body, OPENAI_APPS_CHALLENGE);
    }

    #[test]
    fn challenge_points_to_path_specific_metadata() {
        let oauth =
            OAuthResourceConfig::new("https://api.anarlog.so/mcp", "https://auth.example.com")
                .unwrap();

        assert_eq!(
            oauth.challenge(Some(("invalid_token", "Access token is invalid"))),
            "Bearer resource_metadata=\"https://api.anarlog.so/.well-known/oauth-protected-resource/mcp\", scope=\"openid email\", error=\"invalid_token\", error_description=\"Access token is invalid\""
        );
    }
}
