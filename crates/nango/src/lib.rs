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

    #[test]
    fn builder_requires_api_key_and_defaults_api_base() {
        let result = NangoClientBuilder::default()
            .api_base("https://api.nango.dev")
            .build();

        assert!(result.is_err());

        let nango_client = NangoClientBuilder::default()
            .api_key("key")
            .build()
            .unwrap();
        assert_eq!(nango_client.api_base.as_str(), "https://api.nango.dev/");
    }

    fn scope_override_defaults(
        integration_id: &str,
        oauth_scopes_override: &str,
    ) -> std::collections::HashMap<String, IntegrationConfigDefault> {
        let mut defaults = std::collections::HashMap::new();
        defaults.insert(
            integration_id.to_string(),
            IntegrationConfigDefault {
                user_scopes: None,
                connection_config: Some(ConnectionConfigOverride {
                    oauth_scopes_override: Some(oauth_scopes_override.to_string()),
                }),
            },
        );
        defaults
    }

    #[test]
    fn session_requests_serialize_oauth_scope_overrides() {
        let connect_json = serde_json::to_value(&CreateConnectSessionRequest {
            end_user: EndUser {
                id: "user-1".to_string(),
                display_name: None,
                email: None,
                tags: None,
            },
            organization: None,
            allowed_integrations: Some(vec!["google-calendar".to_string()]),
            integrations_config_defaults: Some(scope_override_defaults(
                "google-calendar",
                "https://www.googleapis.com/auth/calendar.readonly",
            )),
        })
        .unwrap();

        assert_eq!(
            connect_json["integrations_config_defaults"]["google-calendar"]["connection_config"]["oauth_scopes_override"],
            "https://www.googleapis.com/auth/calendar.readonly"
        );

        let reconnect_json = serde_json::to_value(&ReconnectSessionRequest {
            connection_id: "conn-1".to_string(),
            integration_id: "outlook".to_string(),
            integrations_config_defaults: Some(scope_override_defaults(
                "outlook",
                "offline_access User.Read Calendars.Read",
            )),
        })
        .unwrap();

        assert_eq!(
            reconnect_json["integrations_config_defaults"]["outlook"]["connection_config"]["oauth_scopes_override"],
            "offline_access User.Read Calendars.Read"
        );
    }
}
