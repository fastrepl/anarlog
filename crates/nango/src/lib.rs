mod client;
mod connect_session;
mod connection;
mod error;
mod http;
mod integration;
pub mod proxy;
mod sync;
mod trigger;
pub mod webhook;

pub use client::NangoIntegration;
pub use client::*;
pub use connect_session::*;
pub use connection::*;
pub use error::*;
pub use http::NangoHttpClient;
pub use http::OwnedNangoHttpClient;
pub use integration::*;
pub use proxy::NangoProxy;
pub use proxy::OwnedNangoProxy;
pub use sync::*;
pub use trigger::*;
pub use webhook::*;

macro_rules! common_derives {
    ($item:item) => {
        #[derive(
            Debug,
            Eq,
            PartialEq,
            Clone,
            serde::Serialize,
            serde::Deserialize,
            specta::Type,
            schemars::JsonSchema,
        )]
        $item
    };
}

pub(crate) use common_derives;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_create_connect_session() {
        let api_key =
            std::env::var("NANGO_API_KEY").expect("NANGO_API_KEY is required for this live test");
        let nango_client = NangoClientBuilder::default()
            .api_base("https://api.nango.dev")
            .api_key(api_key)
            .build()
            .unwrap();

        let _ = nango_client
            .create_connect_session(CreateConnectSessionRequest {
                end_user: EndUser {
                    id: "id".to_string(),
                    display_name: None,
                    email: None,
                    tags: None,
                },
                organization: None,
                allowed_integrations: None,
                integrations_config_defaults: None,
            })
            .await
            .unwrap();
    }

    #[test]
    fn test_proxy() {
        let nango_client = NangoClientBuilder::default()
            .api_base("https://api.nango.dev")
            .api_key("api_key")
            .build()
            .unwrap();

        let _ = nango_client
            .integration("google-calendar")
            .connection("connection")
            .get("/users")
            .unwrap();
    }

    #[test]
    fn test_build_missing_api_key() {
        let result = NangoClientBuilder::default()
            .api_base("https://api.nango.dev")
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_build_defaults_api_base() {
        let nango_client = NangoClientBuilder::default()
            .api_key("key")
            .build()
            .unwrap();
        assert_eq!(nango_client.api_base.as_str(), "https://api.nango.dev/");
    }

    #[test]
    fn connect_session_serializes_oauth_scope_overrides() {
        let mut defaults = std::collections::HashMap::new();
        defaults.insert(
            "google-calendar".to_string(),
            IntegrationConfigDefault {
                user_scopes: None,
                connection_config: Some(ConnectionConfigOverride {
                    oauth_scopes_override: Some(
                        "https://www.googleapis.com/auth/calendar.readonly".to_string(),
                    ),
                }),
            },
        );

        let json = serde_json::to_value(&CreateConnectSessionRequest {
            end_user: EndUser {
                id: "user-1".to_string(),
                display_name: None,
                email: None,
                tags: None,
            },
            organization: None,
            allowed_integrations: Some(vec!["google-calendar".to_string()]),
            integrations_config_defaults: Some(defaults),
        })
        .unwrap();

        assert_eq!(
            json["integrations_config_defaults"]["google-calendar"]["connection_config"]["oauth_scopes_override"],
            "https://www.googleapis.com/auth/calendar.readonly"
        );
    }

    #[test]
    fn reconnect_session_serializes_oauth_scope_overrides() {
        let mut defaults = std::collections::HashMap::new();
        defaults.insert(
            "outlook".to_string(),
            IntegrationConfigDefault {
                user_scopes: None,
                connection_config: Some(ConnectionConfigOverride {
                    oauth_scopes_override: Some(
                        "offline_access User.Read Calendars.Read".to_string(),
                    ),
                }),
            },
        );

        let json = serde_json::to_value(&ReconnectSessionRequest {
            connection_id: "conn-1".to_string(),
            integration_id: "outlook".to_string(),
            integrations_config_defaults: Some(defaults),
        })
        .unwrap();

        assert_eq!(
            json["integrations_config_defaults"]["outlook"]["connection_config"]["oauth_scopes_override"],
            "offline_access User.Read Calendars.Read"
        );
    }
}
