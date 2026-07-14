use serde::Deserialize;

const DEFAULT_TOKEN_TTL_SECONDS: u64 = 15 * 60;
const MIN_TOKEN_TTL_SECONDS: u64 = 60;
const MAX_TOKEN_TTL_SECONDS: u64 = 60 * 60;

#[derive(Clone, Deserialize)]
pub struct SyncEnv {
    #[serde(default)]
    pub sqlitecloud_project_url: Option<String>,
    #[serde(default)]
    pub sqlitecloud_token_issuer_api_key: Option<String>,
    #[serde(default)]
    pub anarlog_cloudsync_database_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_u64")]
    pub anarlog_cloudsync_token_ttl_seconds: Option<u64>,
}

fn deserialize_optional_u64<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)?
        .map(|value| value.parse().map_err(serde::de::Error::custom))
        .transpose()
}

#[derive(Clone)]
pub struct SyncConfig {
    pub(crate) project_url: String,
    pub(crate) token_issuer_api_key: String,
    pub(crate) database_id: String,
    pub(crate) token_ttl_seconds: u64,
}

impl SyncConfig {
    pub fn new(
        project_url: impl Into<String>,
        token_issuer_api_key: impl Into<String>,
        database_id: impl Into<String>,
    ) -> Result<Self, String> {
        Ok(Self {
            project_url: validate_project_url(project_url.into())?,
            token_issuer_api_key: token_issuer_api_key.into(),
            database_id: database_id.into(),
            token_ttl_seconds: DEFAULT_TOKEN_TTL_SECONDS,
        })
    }

    pub fn with_token_ttl_seconds(mut self, token_ttl_seconds: u64) -> Result<Self, String> {
        validate_token_ttl(token_ttl_seconds)?;
        self.token_ttl_seconds = token_ttl_seconds;
        Ok(self)
    }

    pub fn from_env(env: &SyncEnv) -> Result<Option<Self>, String> {
        let project_url = nonempty(env.sqlitecloud_project_url.as_deref());
        let token_issuer_api_key = nonempty(env.sqlitecloud_token_issuer_api_key.as_deref());
        let database_id = nonempty(env.anarlog_cloudsync_database_id.as_deref());

        if project_url.is_none() && token_issuer_api_key.is_none() && database_id.is_none() {
            return Ok(None);
        }
        let project_url = project_url.ok_or_else(|| {
            "SQLITECLOUD_PROJECT_URL is required when CloudSync token exchange is configured"
                .to_string()
        })?;
        let token_issuer_api_key = token_issuer_api_key.ok_or_else(|| {
            "SQLITECLOUD_TOKEN_ISSUER_API_KEY is required when CloudSync token exchange is configured"
                .to_string()
        })?;
        let database_id = database_id.ok_or_else(|| {
            "ANARLOG_CLOUDSYNC_DATABASE_ID is required when CloudSync token exchange is configured"
                .to_string()
        })?;
        let token_ttl_seconds = env
            .anarlog_cloudsync_token_ttl_seconds
            .unwrap_or(DEFAULT_TOKEN_TTL_SECONDS);
        validate_token_ttl(token_ttl_seconds)?;

        Ok(Some(
            Self::new(project_url, token_issuer_api_key, database_id)?
                .with_token_ttl_seconds(token_ttl_seconds)?,
        ))
    }
}

fn validate_project_url(value: String) -> Result<String, String> {
    let url = reqwest::Url::parse(&value)
        .map_err(|_| "SQLITECLOUD_PROJECT_URL must be a valid URL".to_string())?;
    let host = url
        .host_str()
        .ok_or_else(|| "SQLITECLOUD_PROJECT_URL must include a host".to_string())?;
    if url.scheme() != "https" || !host.ends_with(".sqlite.cloud") {
        return Err(
            "SQLITECLOUD_PROJECT_URL must be an HTTPS SQLite Cloud project URL".to_string(),
        );
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("SQLITECLOUD_PROJECT_URL must contain only the project origin".to_string());
    }

    Ok(url.origin().ascii_serialization())
}

fn validate_token_ttl(token_ttl_seconds: u64) -> Result<(), String> {
    if !(MIN_TOKEN_TTL_SECONDS..=MAX_TOKEN_TTL_SECONDS).contains(&token_ttl_seconds) {
        return Err(format!(
            "ANARLOG_CLOUDSYNC_TOKEN_TTL_SECONDS must be between {MIN_TOKEN_TTL_SECONDS} and {MAX_TOKEN_TTL_SECONDS}"
        ));
    }
    Ok(())
}

fn nonempty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(project_url: &str, token_ttl_seconds: Option<u64>) -> SyncEnv {
        SyncEnv {
            sqlitecloud_project_url: Some(project_url.to_string()),
            sqlitecloud_token_issuer_api_key: Some("issuer-key".to_string()),
            anarlog_cloudsync_database_id: Some("database-id".to_string()),
            anarlog_cloudsync_token_ttl_seconds: token_ttl_seconds,
        }
    }

    #[test]
    fn accepts_https_sqlite_cloud_project_url() {
        let config =
            SyncConfig::from_env(&env("https://project.region.gateway.sqlite.cloud/", None))
                .unwrap()
                .unwrap();

        assert_eq!(
            config.project_url,
            "https://project.region.gateway.sqlite.cloud"
        );
        assert_eq!(config.token_ttl_seconds, DEFAULT_TOKEN_TTL_SECONDS);
    }

    #[test]
    fn rejects_non_https_or_non_sqlite_cloud_project_url() {
        assert!(SyncConfig::from_env(&env("http://project.gateway.sqlite.cloud", None)).is_err());
        assert!(SyncConfig::from_env(&env("https://example.com", None)).is_err());
    }

    #[test]
    fn bounds_token_ttl() {
        assert!(
            SyncConfig::from_env(&env(
                "https://project.gateway.sqlite.cloud",
                Some(MIN_TOKEN_TTL_SECONDS - 1),
            ))
            .is_err()
        );
        assert!(
            SyncConfig::from_env(&env(
                "https://project.gateway.sqlite.cloud",
                Some(MAX_TOKEN_TTL_SECONDS + 1),
            ))
            .is_err()
        );
    }
}
