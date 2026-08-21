use std::{collections::BTreeMap, env, net::SocketAddr, time::Duration};

use anarlog_enterprise_zoom_rtms_worker::{ZoomRtmsCredentials, ZoomWebhookVerifier};
use chrono::Utc;
use serde::Deserialize;

use crate::license::{LICENSE_ENV, LICENSE_KEY_ENV, License, LicenseError};

pub const DATABASE_URL_ENV: &str = "ANARLOG_ENTERPRISE_DATABASE_URL";
pub const WORKSPACE_TOKENS_ENV: &str = "ANARLOG_ENTERPRISE_WORKSPACE_TOKENS";
pub const BIND_ADDRESS_ENV: &str = "ANARLOG_ENTERPRISE_BIND_ADDRESS";
pub const DATABASE_MAX_CONNECTIONS_ENV: &str = "ANARLOG_ENTERPRISE_DATABASE_MAX_CONNECTIONS";
pub const ZOOM_CLIENT_ID_ENV: &str = "ANARLOG_ENTERPRISE_ZOOM_CLIENT_ID";
pub const ZOOM_CLIENT_SECRET_ENV: &str = "ANARLOG_ENTERPRISE_ZOOM_CLIENT_SECRET";
pub const ZOOM_WEBHOOK_SECRET_ENV: &str = "ANARLOG_ENTERPRISE_ZOOM_WEBHOOK_SECRET";
pub const ZOOM_ACCOUNT_WORKSPACES_ENV: &str = "ANARLOG_ENTERPRISE_ZOOM_ACCOUNT_WORKSPACES";

const ZOOM_WEBHOOK_MAX_AGE: Duration = Duration::from_secs(5 * 60);

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub bind_address: SocketAddr,
    pub database_max_connections: u32,
    pub database_acquire_timeout: Duration,
    pub workspace_tokens: BTreeMap<String, String>,
    pub zoom: Option<ZoomConfig>,
    pub license: Option<License>,
}

#[derive(Clone)]
pub struct ZoomConfig {
    credentials: ZoomRtmsCredentials,
    verifier: ZoomWebhookVerifier,
    account_workspaces: BTreeMap<String, String>,
}

impl ZoomConfig {
    pub fn credentials(&self) -> &ZoomRtmsCredentials {
        &self.credentials
    }

    pub fn verifier(&self) -> &ZoomWebhookVerifier {
        &self.verifier
    }

    pub fn workspace_for_account(&self, account_id: &str) -> Option<&str> {
        self.account_workspaces.get(account_id).map(String::as_str)
    }
}

#[derive(Default)]
pub struct ZoomConfigValues {
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub webhook_secret: Option<String>,
    pub account_workspaces: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_values_with_zoom(
            required_env(DATABASE_URL_ENV)?,
            env::var(BIND_ADDRESS_ENV).ok(),
            env::var(DATABASE_MAX_CONNECTIONS_ENV).ok(),
            required_env(WORKSPACE_TOKENS_ENV)?,
            ZoomConfigValues {
                client_id: env::var(ZOOM_CLIENT_ID_ENV).ok(),
                client_secret: env::var(ZOOM_CLIENT_SECRET_ENV).ok(),
                webhook_secret: env::var(ZOOM_WEBHOOK_SECRET_ENV).ok(),
                account_workspaces: env::var(ZOOM_ACCOUNT_WORKSPACES_ENV).ok(),
            },
        )
    }

    pub fn from_values(
        database_url: String,
        bind_address: Option<String>,
        database_max_connections: Option<String>,
        workspace_tokens: String,
    ) -> Result<Self, ConfigError> {
        Self::from_values_with_zoom(
            database_url,
            bind_address,
            database_max_connections,
            workspace_tokens,
            ZoomConfigValues::default(),
        )
    }

    pub fn from_values_with_zoom(
        database_url: String,
        bind_address: Option<String>,
        database_max_connections: Option<String>,
        workspace_tokens: String,
        zoom: ZoomConfigValues,
    ) -> Result<Self, ConfigError> {
        if !database_url.starts_with("postgres://") && !database_url.starts_with("postgresql://") {
            return Err(ConfigError::InvalidDatabaseUrl);
        }

        let bind_address = bind_address
            .unwrap_or_else(|| "0.0.0.0:8080".to_owned())
            .parse()
            .map_err(|_| ConfigError::InvalidBindAddress)?;
        let database_max_connections = database_max_connections
            .unwrap_or_else(|| "10".to_owned())
            .parse::<u32>()
            .map_err(|_| ConfigError::InvalidDatabaseMaxConnections)?;
        if !(1..=100).contains(&database_max_connections) {
            return Err(ConfigError::InvalidDatabaseMaxConnections);
        }

        let workspace_tokens = serde_json::from_str::<WorkspaceTokens>(&workspace_tokens)
            .map_err(|_| ConfigError::InvalidWorkspaceTokens)?
            .0;
        validate_workspace_tokens(&workspace_tokens)?;
        let zoom = parse_zoom_config(zoom, &workspace_tokens)?;
        let license = parse_license(LicenseConfigValues::from_env(), &workspace_tokens)?;

        Ok(Self {
            database_url,
            bind_address,
            database_max_connections,
            database_acquire_timeout: Duration::from_secs(10),
            workspace_tokens,
            zoom,
            license,
        })
    }
}

#[derive(Deserialize)]
#[serde(transparent)]
struct WorkspaceTokens(BTreeMap<String, String>);

#[derive(Deserialize)]
#[serde(transparent)]
struct ZoomAccountWorkspaces(BTreeMap<String, String>);

fn required_env(name: &'static str) -> Result<String, ConfigError> {
    env::var(name).map_err(|_| ConfigError::Missing(name))
}

fn validate_workspace_tokens(
    workspace_tokens: &BTreeMap<String, String>,
) -> Result<(), ConfigError> {
    if workspace_tokens.is_empty() {
        return Err(ConfigError::EmptyWorkspaceTokens);
    }
    for (workspace_id, token) in workspace_tokens {
        if workspace_id.len() > 128
            || workspace_id.is_empty()
            || !workspace_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
        {
            return Err(ConfigError::InvalidWorkspaceId(workspace_id.clone()));
        }
        if token.len() < 32 || token.len() > 512 {
            return Err(ConfigError::InvalidWorkspaceToken(workspace_id.clone()));
        }
    }
    Ok(())
}

fn parse_zoom_config(
    values: ZoomConfigValues,
    workspace_tokens: &BTreeMap<String, String>,
) -> Result<Option<ZoomConfig>, ConfigError> {
    let present = [
        values.client_id.is_some(),
        values.client_secret.is_some(),
        values.webhook_secret.is_some(),
        values.account_workspaces.is_some(),
    ];
    if present.iter().all(|present| !present) {
        return Ok(None);
    }
    if present.iter().any(|present| !present) {
        return Err(ConfigError::IncompleteZoomConfiguration);
    }

    let credentials = ZoomRtmsCredentials::new(
        values
            .client_id
            .expect("complete configuration has a client ID"),
        values
            .client_secret
            .expect("complete configuration has a client secret"),
    )
    .map_err(|_| ConfigError::InvalidZoomCredentials)?;
    let verifier = ZoomWebhookVerifier::new(
        values
            .webhook_secret
            .expect("complete configuration has a webhook secret"),
        ZOOM_WEBHOOK_MAX_AGE,
    )
    .map_err(|_| ConfigError::InvalidZoomWebhookSecret)?;
    let account_workspaces = serde_json::from_str::<ZoomAccountWorkspaces>(
        &values
            .account_workspaces
            .expect("complete configuration has an account registry"),
    )
    .map_err(|_| ConfigError::InvalidZoomAccountWorkspaces)?
    .0;
    if account_workspaces.is_empty() {
        return Err(ConfigError::EmptyZoomAccountWorkspaces);
    }
    for (account_id, workspace_id) in &account_workspaces {
        if account_id.is_empty()
            || account_id.len() > 256
            || account_id.chars().any(char::is_control)
        {
            return Err(ConfigError::InvalidZoomAccountId);
        }
        if !workspace_tokens.contains_key(workspace_id) {
            return Err(ConfigError::UnknownZoomWorkspace(workspace_id.clone()));
        }
    }

    Ok(Some(ZoomConfig {
        credentials,
        verifier,
        account_workspaces,
    }))
}

#[derive(Default)]
pub struct LicenseConfigValues {
    pub token: Option<String>,
    pub key: Option<String>,
}

impl LicenseConfigValues {
    fn from_env() -> Self {
        Self {
            token: env::var(LICENSE_ENV).ok(),
            key: env::var(LICENSE_KEY_ENV).ok(),
        }
    }
}

fn parse_license(
    values: LicenseConfigValues,
    workspace_tokens: &BTreeMap<String, String>,
) -> Result<Option<License>, ConfigError> {
    match (values.token, values.key) {
        (None, None) => Ok(None),
        (Some(_), None) | (None, Some(_)) => Err(ConfigError::IncompleteLicenseConfiguration),
        (Some(token), Some(key)) => {
            let license = License::parse(&token, &key, Utc::now())?;
            if license
                .claims
                .workspace_ids
                .iter()
                .any(|workspace_id| !workspace_tokens.contains_key(workspace_id))
            {
                return Err(ConfigError::UnknownLicenseWorkspace);
            }
            Ok(Some(license))
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("missing required configuration: {0}")]
    Missing(&'static str),
    #[error("{DATABASE_URL_ENV} must be a Postgres URL")]
    InvalidDatabaseUrl,
    #[error("{BIND_ADDRESS_ENV} must be a socket address")]
    InvalidBindAddress,
    #[error("{DATABASE_MAX_CONNECTIONS_ENV} must be an integer between 1 and 100")]
    InvalidDatabaseMaxConnections,
    #[error("{WORKSPACE_TOKENS_ENV} must be a JSON object of workspace IDs to bearer tokens")]
    InvalidWorkspaceTokens,
    #[error("{WORKSPACE_TOKENS_ENV} must contain at least one workspace")]
    EmptyWorkspaceTokens,
    #[error("invalid workspace ID in {WORKSPACE_TOKENS_ENV}: {0}")]
    InvalidWorkspaceId(String),
    #[error("bearer token for workspace {0} must contain between 32 and 512 bytes")]
    InvalidWorkspaceToken(String),
    #[error(
        "Zoom integration requires {ZOOM_CLIENT_ID_ENV}, {ZOOM_CLIENT_SECRET_ENV}, {ZOOM_WEBHOOK_SECRET_ENV}, and {ZOOM_ACCOUNT_WORKSPACES_ENV} together"
    )]
    IncompleteZoomConfiguration,
    #[error("Zoom client credentials are invalid")]
    InvalidZoomCredentials,
    #[error("Zoom webhook credentials are invalid")]
    InvalidZoomWebhookSecret,
    #[error(
        "{ZOOM_ACCOUNT_WORKSPACES_ENV} must be a JSON object of Zoom account IDs to workspace IDs"
    )]
    InvalidZoomAccountWorkspaces,
    #[error("{ZOOM_ACCOUNT_WORKSPACES_ENV} must contain at least one Zoom account")]
    EmptyZoomAccountWorkspaces,
    #[error("{ZOOM_ACCOUNT_WORKSPACES_ENV} contains an invalid Zoom account ID")]
    InvalidZoomAccountId,
    #[error("Zoom account registry references an unconfigured workspace: {0}")]
    UnknownZoomWorkspace(String),
    #[error("offline license validation requires {LICENSE_ENV} and {LICENSE_KEY_ENV} together")]
    IncompleteLicenseConfiguration,
    #[error("offline license is invalid")]
    InvalidLicense,
    #[error("offline license references an unconfigured workspace")]
    UnknownLicenseWorkspace,
}

impl From<LicenseError> for ConfigError {
    fn from(error: LicenseError) -> Self {
        match error {
            LicenseError::InvalidKey => Self::IncompleteLicenseConfiguration,
            LicenseError::InvalidToken
            | LicenseError::InvalidSignature
            | LicenseError::NotYetValid
            | LicenseError::Expired => Self::InvalidLicense,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn accepts_minimal_configuration() {
        let config = Config::from_values(
            "postgres://postgres:postgres@localhost/anarlog".into(),
            None,
            None,
            format!(r#"{{"workspace-a":"{TOKEN}"}}"#),
        )
        .unwrap();

        assert_eq!(config.bind_address, "0.0.0.0:8080".parse().unwrap());
        assert_eq!(config.database_max_connections, 10);
        assert_eq!(config.workspace_tokens["workspace-a"], TOKEN);
        assert!(config.zoom.is_none());
        assert!(config.license.is_none());
    }

    #[test]
    fn fails_closed_without_workspace_credentials() {
        let error = match Config::from_values(
            "postgres://localhost/anarlog".into(),
            None,
            None,
            "{}".into(),
        ) {
            Ok(_) => panic!("empty credentials must fail"),
            Err(error) => error,
        };

        assert_eq!(error, ConfigError::EmptyWorkspaceTokens);
    }

    #[test]
    fn rejects_short_tokens_without_echoing_them() {
        let error = match Config::from_values(
            "postgres://localhost/anarlog".into(),
            None,
            None,
            r#"{"workspace-a":"short-secret"}"#.into(),
        ) {
            Ok(_) => panic!("short tokens must fail"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            ConfigError::InvalidWorkspaceToken("workspace-a".into())
        );
        assert!(!error.to_string().contains("short-secret"));
    }

    #[test]
    fn enables_zoom_only_with_a_complete_workspace_scoped_registry() {
        let config = Config::from_values_with_zoom(
            "postgres://localhost/anarlog".into(),
            None,
            None,
            format!(r#"{{"workspace-a":"{TOKEN}"}}"#),
            ZoomConfigValues {
                client_id: Some("zoom-client".into()),
                client_secret: Some("zoom-client-secret".into()),
                webhook_secret: Some("zoom-webhook-secret".into()),
                account_workspaces: Some(r#"{"account-a":"workspace-a"}"#.into()),
            },
        )
        .unwrap();

        let zoom = config.zoom.unwrap();
        assert_eq!(zoom.workspace_for_account("account-a"), Some("workspace-a"));
        assert_eq!(zoom.workspace_for_account("account-b"), None);
        assert!(!format!("{:?}", zoom.credentials()).contains("zoom-client-secret"));
        assert!(!format!("{:?}", zoom.verifier()).contains("zoom-webhook-secret"));
    }

    #[test]
    fn rejects_partial_zoom_configuration_without_weakening_core_startup() {
        let error = Config::from_values_with_zoom(
            "postgres://localhost/anarlog".into(),
            None,
            None,
            format!(r#"{{"workspace-a":"{TOKEN}"}}"#),
            ZoomConfigValues {
                client_id: Some("zoom-client".into()),
                ..ZoomConfigValues::default()
            },
        )
        .err()
        .unwrap();

        assert_eq!(error, ConfigError::IncompleteZoomConfiguration);
    }

    #[test]
    fn rejects_zoom_accounts_mapped_to_an_unconfigured_workspace() {
        let error = Config::from_values_with_zoom(
            "postgres://localhost/anarlog".into(),
            None,
            None,
            format!(r#"{{"workspace-a":"{TOKEN}"}}"#),
            ZoomConfigValues {
                client_id: Some("zoom-client".into()),
                client_secret: Some("zoom-client-secret".into()),
                webhook_secret: Some("zoom-webhook-secret".into()),
                account_workspaces: Some(r#"{"account-a":"workspace-b"}"#.into()),
            },
        )
        .err()
        .unwrap();

        assert_eq!(
            error,
            ConfigError::UnknownZoomWorkspace("workspace-b".into())
        );
    }
}
