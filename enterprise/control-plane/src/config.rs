use std::{collections::BTreeMap, env, net::SocketAddr, time::Duration};

use serde::Deserialize;

pub const DATABASE_URL_ENV: &str = "ANARLOG_ENTERPRISE_DATABASE_URL";
pub const WORKSPACE_TOKENS_ENV: &str = "ANARLOG_ENTERPRISE_WORKSPACE_TOKENS";
pub const BIND_ADDRESS_ENV: &str = "ANARLOG_ENTERPRISE_BIND_ADDRESS";
pub const DATABASE_MAX_CONNECTIONS_ENV: &str = "ANARLOG_ENTERPRISE_DATABASE_MAX_CONNECTIONS";

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub bind_address: SocketAddr,
    pub database_max_connections: u32,
    pub database_acquire_timeout: Duration,
    pub workspace_tokens: BTreeMap<String, String>,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_values(
            required_env(DATABASE_URL_ENV)?,
            env::var(BIND_ADDRESS_ENV).ok(),
            env::var(DATABASE_MAX_CONNECTIONS_ENV).ok(),
            required_env(WORKSPACE_TOKENS_ENV)?,
        )
    }

    pub fn from_values(
        database_url: String,
        bind_address: Option<String>,
        database_max_connections: Option<String>,
        workspace_tokens: String,
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

        Ok(Self {
            database_url,
            bind_address,
            database_max_connections,
            database_acquire_timeout: Duration::from_secs(10),
            workspace_tokens,
        })
    }
}

#[derive(Deserialize)]
#[serde(transparent)]
struct WorkspaceTokens(BTreeMap<String, String>);

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
}
