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
    pub anarlog_cloudsync_e2ee_database_id: Option<String>,
    #[serde(default)]
    pub anarlog_cloudsync_database_id: Option<String>,
    #[serde(default)]
    pub anarlog_cloudsync_protocol_mode: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_u64")]
    pub anarlog_cloudsync_token_ttl_seconds: Option<u64>,
    #[serde(default)]
    pub anarlog_cloudsync_desktop_transport: Option<String>,
    #[serde(default)]
    pub anarlog_sqlitecloud_mode: Option<String>,
}

/// Lifecycle of the SQLite Cloud provider. It decides whether `/sync/token`
/// may still mint provider tokens, so it wins over the transport default and
/// over per-account overrides, which only choose among transports it allows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SqliteCloudMode {
    /// Provider tokens are issued to accounts not yet moved to the replica.
    Enabled,
    /// No new provider tokens; clients that cannot run the accepted replica
    /// are told to update. Provider access remains for audit and deletion.
    Draining,
    /// The provider is gone: no provider secret is read and no request is sent.
    Retired,
}

impl SqliteCloudMode {
    /// Unset keeps the behavior from before the lifecycle existed: provider
    /// variables enable it, and a deployment without any is replica-only.
    pub fn from_env(env: &SyncEnv) -> Result<Self, String> {
        match nonempty(env.anarlog_sqlitecloud_mode.as_deref()).as_deref() {
            Some("enabled") => Ok(Self::Enabled),
            Some("draining") => Ok(Self::Draining),
            Some("retired") => Ok(Self::Retired),
            Some(_) => {
                Err("ANARLOG_SQLITECLOUD_MODE must be enabled, draining, or retired".to_string())
            }
            None if env.has_sqlitecloud_variables() => Ok(Self::Enabled),
            None => Ok(Self::Retired),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Draining => "draining",
            Self::Retired => "retired",
        }
    }
}

impl SyncEnv {
    fn has_sqlitecloud_variables(&self) -> bool {
        [
            &self.sqlitecloud_project_url,
            &self.sqlitecloud_token_issuer_api_key,
            &self.anarlog_cloudsync_e2ee_database_id,
            &self.anarlog_cloudsync_database_id,
        ]
        .into_iter()
        .any(|value| nonempty(value.as_deref()).is_some())
    }
}

/// Which transport `/sync/token` hands to desktop clients that can accept
/// either while SQLite Cloud is enabled. Per-account rows in
/// `sync_transport_overrides` win over this default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum CloudsyncTransport {
    #[default]
    SqliteSync,
    Replica,
}

impl CloudsyncTransport {
    pub(crate) fn parse(value: Option<&str>) -> Result<Self, String> {
        match value.map(str::trim).filter(|value| !value.is_empty()) {
            None | Some("sqlite_sync") => Ok(Self::SqliteSync),
            Some("replica") => Ok(Self::Replica),
            Some(_) => Err(
                "ANARLOG_CLOUDSYNC_DESKTOP_TRANSPORT must be sqlite_sync or replica".to_string(),
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum CloudsyncProtocolMode {
    Dual,
    E2eeOnly,
    #[default]
    E2eeEnforced,
}

impl CloudsyncProtocolMode {
    fn parse(value: Option<&str>) -> Result<Self, String> {
        match value.map(str::trim).filter(|value| !value.is_empty()) {
            None | Some("e2ee_enforced") => Ok(Self::E2eeEnforced),
            Some("dual") => Ok(Self::Dual),
            Some("e2ee_only") => Ok(Self::E2eeOnly),
            Some(_) => Err(
                "ANARLOG_CLOUDSYNC_PROTOCOL_MODE must be dual, e2ee_only, or e2ee_enforced"
                    .to_string(),
            ),
        }
    }
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
    pub(crate) sqlitecloud_mode: SqliteCloudMode,
    /// Present exactly when the provider is not retired.
    pub(crate) sqlitecloud: Option<SqliteCloudConfig>,
    pub(crate) protocol_mode: CloudsyncProtocolMode,
    pub(crate) desktop_transport: CloudsyncTransport,
    pub(crate) supabase_url: String,
    pub(crate) supabase_anon_key: String,
    pub(crate) supabase_service_role_key: String,
}

#[derive(Clone)]
pub(crate) struct SqliteCloudConfig {
    pub(crate) project_url: String,
    pub(crate) token_issuer_api_key: String,
    pub(crate) database_id: String,
    pub(crate) legacy_database_id: Option<String>,
    pub(crate) token_ttl_seconds: u64,
}

#[derive(Clone)]
pub struct ReplicaConfig {
    pub(crate) supabase_url: String,
    pub(crate) supabase_anon_key: String,
    pub(crate) supabase_service_role_key: String,
}

#[derive(Clone)]
pub struct SharedNotesConfig {
    pub(crate) supabase_url: String,
    pub(crate) supabase_service_role_key: String,
    pub(crate) loops_api_key: Option<String>,
    pub(crate) loops_api_base: Option<reqwest::Url>,
    pub(crate) resend_api_key: Option<String>,
    pub(crate) resend_api_base: Option<reqwest::Url>,
    pub(crate) resend_from_email: Option<String>,
}

impl SharedNotesConfig {
    pub fn new(
        supabase_url: impl Into<String>,
        supabase_service_role_key: impl Into<String>,
    ) -> Result<Self, String> {
        let supabase_service_role_key = supabase_service_role_key.into();
        if supabase_service_role_key.trim().is_empty() {
            return Err(
                "SUPABASE_SERVICE_ROLE_KEY is required for shared note delivery".to_string(),
            );
        }

        Ok(Self {
            supabase_url: validate_supabase_url(supabase_url.into())?,
            supabase_service_role_key,
            loops_api_key: None,
            loops_api_base: None,
            resend_api_key: None,
            resend_api_base: None,
            resend_from_email: None,
        })
    }

    pub fn with_resend_email(
        mut self,
        api_key: impl Into<String>,
        from_email: impl Into<String>,
    ) -> Result<Self, String> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err("RESEND_API_KEY is required for shared note email".to_string());
        }
        let from_email = from_email.into();
        if !is_email_address(&from_email) {
            return Err(
                "RESEND_FROM_EMAIL must be a valid email address for shared note email".to_string(),
            );
        }
        self.resend_api_key = Some(api_key);
        self.resend_from_email = Some(from_email);
        Ok(self)
    }

    #[cfg(test)]
    pub(crate) fn with_resend_api_base(mut self, api_base: reqwest::Url) -> Self {
        self.resend_api_base = Some(api_base);
        self
    }

    pub fn with_invitation_email(
        mut self,
        loops_api_key: impl Into<String>,
    ) -> Result<Self, String> {
        let loops_api_key = loops_api_key.into();
        if loops_api_key.trim().is_empty() {
            return Err("LOOPS_KEY is required for shared note invitations".to_string());
        }
        self.loops_api_key = Some(loops_api_key);
        Ok(self)
    }

    #[cfg(test)]
    pub(crate) fn with_invitation_email_api_base(mut self, api_base: reqwest::Url) -> Self {
        self.loops_api_base = Some(api_base);
        self
    }
}

fn is_email_address(value: &str) -> bool {
    value.len() <= 320
        && value.trim() == value
        && !value.chars().any(char::is_control)
        && value
            .split_once('@')
            .is_some_and(|(local, domain)| !local.is_empty() && domain.contains('.'))
}

impl SyncConfig {
    /// Sync backed only by Supabase; SQLite Cloud is retired.
    pub fn new(
        supabase_url: impl Into<String>,
        supabase_anon_key: impl Into<String>,
        supabase_service_role_key: impl Into<String>,
    ) -> Result<Self, String> {
        let supabase_anon_key = supabase_anon_key.into();
        if supabase_anon_key.trim().is_empty() {
            return Err(
                "SUPABASE_ANON_KEY is required for CloudSync workspace projection".to_string(),
            );
        }
        let supabase_service_role_key = supabase_service_role_key.into();
        if supabase_service_role_key.trim().is_empty() {
            return Err(
                "SUPABASE_SERVICE_ROLE_KEY is required for shared note publication".to_string(),
            );
        }

        Ok(Self {
            sqlitecloud_mode: SqliteCloudMode::Retired,
            sqlitecloud: None,
            protocol_mode: CloudsyncProtocolMode::E2eeEnforced,
            desktop_transport: CloudsyncTransport::SqliteSync,
            supabase_url: validate_supabase_url(supabase_url.into())?,
            supabase_anon_key,
            supabase_service_role_key,
        })
    }

    /// Keeps SQLite Cloud reachable in `enabled` or `draining` mode.
    pub fn with_sqlitecloud(
        mut self,
        mode: SqliteCloudMode,
        project_url: impl Into<String>,
        token_issuer_api_key: impl Into<String>,
        database_id: impl Into<String>,
    ) -> Result<Self, String> {
        if mode == SqliteCloudMode::Retired {
            return Err("A retired SQLite Cloud provider cannot be configured".to_string());
        }
        self.sqlitecloud_mode = mode;
        self.sqlitecloud = Some(SqliteCloudConfig {
            project_url: validate_project_url(project_url.into())?,
            token_issuer_api_key: token_issuer_api_key.into(),
            database_id: database_id.into(),
            legacy_database_id: None,
            token_ttl_seconds: DEFAULT_TOKEN_TTL_SECONDS,
        });
        Ok(self)
    }

    pub fn sqlitecloud_mode(&self) -> SqliteCloudMode {
        self.sqlitecloud_mode
    }

    pub fn from_env(
        env: &SyncEnv,
        supabase_url: &str,
        supabase_anon_key: &str,
        supabase_service_role_key: &str,
    ) -> Result<Self, String> {
        let mode = SqliteCloudMode::from_env(env)?;
        let protocol_mode = CloudsyncProtocolMode::parse(
            nonempty(env.anarlog_cloudsync_protocol_mode.as_deref()).as_deref(),
        )?;
        let desktop_transport = CloudsyncTransport::parse(
            nonempty(env.anarlog_cloudsync_desktop_transport.as_deref()).as_deref(),
        )?;
        let token_ttl_seconds = env
            .anarlog_cloudsync_token_ttl_seconds
            .unwrap_or(DEFAULT_TOKEN_TTL_SECONDS);
        validate_token_ttl(token_ttl_seconds)?;

        let mut config = Self::new(supabase_url, supabase_anon_key, supabase_service_role_key)?;
        config.desktop_transport = desktop_transport;
        // Retired deployments may still carry stale provider secrets, because
        // deploys import secrets without unsetting them. They are never read.
        if mode == SqliteCloudMode::Retired {
            return Ok(config);
        }

        let required = |value: &Option<String>, name: &str| {
            nonempty(value.as_deref()).ok_or_else(|| {
                format!(
                    "{name} is required while ANARLOG_SQLITECLOUD_MODE is {}",
                    mode.as_str()
                )
            })
        };
        let project_url = required(&env.sqlitecloud_project_url, "SQLITECLOUD_PROJECT_URL")?;
        let token_issuer_api_key = required(
            &env.sqlitecloud_token_issuer_api_key,
            "SQLITECLOUD_TOKEN_ISSUER_API_KEY",
        )?;
        let database_id = required(
            &env.anarlog_cloudsync_e2ee_database_id,
            "ANARLOG_CLOUDSYNC_E2EE_DATABASE_ID",
        )?;
        let legacy_database_id = nonempty(env.anarlog_cloudsync_database_id.as_deref());
        validate_protocol_databases(&database_id, legacy_database_id.as_deref(), protocol_mode)?;

        let mut config =
            config.with_sqlitecloud(mode, project_url, token_issuer_api_key, database_id)?;
        config.protocol_mode = protocol_mode;
        if let Some(sqlitecloud) = config.sqlitecloud.as_mut() {
            sqlitecloud.legacy_database_id = legacy_database_id;
            sqlitecloud.token_ttl_seconds = token_ttl_seconds;
        }
        Ok(config)
    }

    pub(crate) fn replica_config(&self) -> ReplicaConfig {
        ReplicaConfig {
            supabase_url: self.supabase_url.clone(),
            supabase_anon_key: self.supabase_anon_key.clone(),
            supabase_service_role_key: self.supabase_service_role_key.clone(),
        }
    }
}

impl ReplicaConfig {
    pub fn new(
        supabase_url: impl Into<String>,
        supabase_anon_key: impl Into<String>,
        supabase_service_role_key: impl Into<String>,
    ) -> Result<Self, String> {
        let supabase_anon_key = supabase_anon_key.into();
        if supabase_anon_key.trim().is_empty() {
            return Err("SUPABASE_ANON_KEY is required for encrypted replica sync".to_string());
        }
        let supabase_service_role_key = supabase_service_role_key.into();
        if supabase_service_role_key.trim().is_empty() {
            return Err(
                "SUPABASE_SERVICE_ROLE_KEY is required for encrypted replica sync".to_string(),
            );
        }

        Ok(Self {
            supabase_url: validate_supabase_url(supabase_url.into())?,
            supabase_anon_key,
            supabase_service_role_key,
        })
    }
}

fn validate_protocol_databases(
    database_id: &str,
    legacy_database_id: Option<&str>,
    protocol_mode: CloudsyncProtocolMode,
) -> Result<(), String> {
    if legacy_database_id == Some(database_id) {
        return Err(
            "ANARLOG_CLOUDSYNC_DATABASE_ID must differ from ANARLOG_CLOUDSYNC_E2EE_DATABASE_ID"
                .to_string(),
        );
    }
    if protocol_mode == CloudsyncProtocolMode::Dual && legacy_database_id.is_none() {
        return Err("ANARLOG_CLOUDSYNC_DATABASE_ID is required in dual protocol mode".to_string());
    }
    Ok(())
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

fn validate_supabase_url(value: String) -> Result<String, String> {
    let url =
        reqwest::Url::parse(&value).map_err(|_| "SUPABASE_URL must be a valid URL".to_string())?;
    let host = url
        .host_str()
        .ok_or_else(|| "SUPABASE_URL must include a host".to_string())?;
    let address_host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    let is_loopback = host.eq_ignore_ascii_case("localhost")
        || address_host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    if (url.scheme() != "https" && !(url.scheme() == "http" && is_loopback))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "SUPABASE_URL must use HTTPS, except for HTTP loopback development origins".to_string(),
        );
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
            anarlog_cloudsync_e2ee_database_id: Some("database-id".to_string()),
            anarlog_cloudsync_database_id: None,
            anarlog_cloudsync_protocol_mode: None,
            anarlog_cloudsync_token_ttl_seconds: token_ttl_seconds,
            anarlog_cloudsync_desktop_transport: None,
            anarlog_sqlitecloud_mode: None,
        }
    }

    fn sqlitecloud(config: &SyncConfig) -> &SqliteCloudConfig {
        config.sqlitecloud.as_ref().unwrap()
    }

    #[test]
    fn default_config_is_enforced_e2ee_over_sqlite_sync() {
        let config = config(&env("https://project.region.gateway.sqlite.cloud/", None)).unwrap();

        assert_eq!(config.sqlitecloud_mode, SqliteCloudMode::Enabled);
        assert_eq!(config.protocol_mode, CloudsyncProtocolMode::E2eeEnforced);
        assert_eq!(config.desktop_transport, CloudsyncTransport::SqliteSync);
        assert!(sqlitecloud(&config).legacy_database_id.is_none());
        assert_eq!(
            sqlitecloud(&config).project_url,
            "https://project.region.gateway.sqlite.cloud"
        );
        assert_eq!(
            sqlitecloud(&config).token_ttl_seconds,
            DEFAULT_TOKEN_TTL_SECONDS
        );
    }

    #[test]
    fn sqlitecloud_mode_decides_whether_provider_configuration_is_read() {
        let provider = || env("https://project.region.gateway.sqlite.cloud/", None);
        let no_provider = || SyncEnv {
            sqlitecloud_project_url: None,
            sqlitecloud_token_issuer_api_key: None,
            anarlog_cloudsync_e2ee_database_id: None,
            ..provider()
        };

        for (case, mut sync_env, mode, expected) in [
            (
                "unset mode without provider variables is replica-only",
                no_provider(),
                None,
                Ok((SqliteCloudMode::Retired, false)),
            ),
            (
                "draining keeps provider access",
                provider(),
                Some("draining"),
                Ok((SqliteCloudMode::Draining, true)),
            ),
            (
                "retired ignores stale provider secrets",
                provider(),
                Some(" retired "),
                Ok((SqliteCloudMode::Retired, false)),
            ),
            (
                "retired needs no provider variables",
                no_provider(),
                Some("retired"),
                Ok((SqliteCloudMode::Retired, false)),
            ),
            (
                "enabled does not fall back when the provider is missing",
                no_provider(),
                Some("enabled"),
                Err("SQLITECLOUD_PROJECT_URL"),
            ),
            (
                "draining does not fall back when the provider is missing",
                no_provider(),
                Some("draining"),
                Err("SQLITECLOUD_PROJECT_URL"),
            ),
            (
                "an incomplete provider is rejected",
                SyncEnv {
                    sqlitecloud_token_issuer_api_key: None,
                    ..provider()
                },
                None,
                Err("SQLITECLOUD_TOKEN_ISSUER_API_KEY"),
            ),
            (
                "unknown modes are rejected",
                provider(),
                Some("disabled"),
                Err("ANARLOG_SQLITECLOUD_MODE"),
            ),
        ] {
            sync_env.anarlog_sqlitecloud_mode = mode.map(ToString::to_string);
            let result = config(&sync_env)
                .map(|config| (config.sqlitecloud_mode, config.sqlitecloud.is_some()));
            match expected {
                Ok(expected) => assert_eq!(result, Ok(expected), "{case}"),
                Err(variable) => assert!(result.unwrap_err().contains(variable), "{case}"),
            }
        }
    }

    #[test]
    fn accepts_only_known_desktop_transports() {
        let mut sync_env = env("https://project.region.gateway.sqlite.cloud/", None);
        sync_env.anarlog_cloudsync_desktop_transport = Some("replica".to_string());
        let replica = config(&sync_env).unwrap();
        assert_eq!(replica.desktop_transport, CloudsyncTransport::Replica);

        sync_env.anarlog_cloudsync_desktop_transport = Some(" sqlite_sync ".to_string());
        let sqlite_sync = config(&sync_env).unwrap();
        assert_eq!(
            sqlite_sync.desktop_transport,
            CloudsyncTransport::SqliteSync
        );

        sync_env.anarlog_cloudsync_desktop_transport = Some("witness".to_string());
        assert!(config(&sync_env).is_err());
    }

    #[test]
    fn validates_protocol_mode_and_database_ids() {
        for (case, mode, legacy_database_id, expect_ok) in [
            ("dual requires a legacy database", "dual", None, false),
            (
                "dual rejects the e2ee database as legacy",
                "dual",
                Some("database-id"),
                false,
            ),
            (
                "e2ee_only rejects the e2ee database as legacy",
                "e2ee_only",
                Some("database-id"),
                false,
            ),
            (
                "e2ee_enforced rejects the e2ee database as legacy",
                "e2ee_enforced",
                Some("database-id"),
                false,
            ),
            ("e2ee_only is supported", "e2ee_only", None, true),
            ("e2ee_enforced is supported", "e2ee_enforced", None, true),
            ("unknown protocol modes are rejected", "legacy", None, false),
        ] {
            let mut sync_env = env("https://project.region.gateway.sqlite.cloud/", None);
            sync_env.anarlog_cloudsync_protocol_mode = Some(mode.to_string());
            sync_env.anarlog_cloudsync_database_id = legacy_database_id.map(ToString::to_string);

            assert_eq!(config(&sync_env).is_ok(), expect_ok, "{case}");
        }

        let mut sync_env = env("https://project.region.gateway.sqlite.cloud/", None);
        sync_env.anarlog_cloudsync_protocol_mode = Some("dual".to_string());
        sync_env.anarlog_cloudsync_database_id = Some("legacy-database-id".to_string());
        let config = config(&sync_env).unwrap();
        assert_eq!(config.protocol_mode, CloudsyncProtocolMode::Dual);
        assert_eq!(
            sqlitecloud(&config).legacy_database_id.as_deref(),
            Some("legacy-database-id")
        );
    }

    fn config(env: &SyncEnv) -> Result<SyncConfig, String> {
        SyncConfig::from_env(
            env,
            "https://project.supabase.co",
            "anon-key",
            "service-role-key",
        )
    }

    #[test]
    fn validates_shared_note_delivery_configuration() {
        assert!(SharedNotesConfig::new("https://project.supabase.co", "service-role-key").is_ok());
        assert!(SharedNotesConfig::new("http://project.supabase.co", "service-role-key").is_err());
        assert!(SharedNotesConfig::new("https://project.supabase.co", "").is_err());

        let invitation_api_base = reqwest::Url::parse("https://api.loops.so").unwrap();
        let config = SharedNotesConfig::new("https://project.supabase.co", "service-role-key")
            .unwrap()
            .with_invitation_email_api_base(invitation_api_base);
        assert_eq!(
            config.loops_api_base.unwrap().as_str(),
            "https://api.loops.so/"
        );
    }

    #[test]
    fn rejects_unsafe_project_urls_and_out_of_range_ttls() {
        for (case, project_url, token_ttl_seconds) in [
            (
                "non-https project URLs are rejected",
                "http://project.gateway.sqlite.cloud",
                None,
            ),
            (
                "non-SQLite Cloud project URLs are rejected",
                "https://example.com",
                None,
            ),
            (
                "token TTL below the minimum is rejected",
                "https://project.gateway.sqlite.cloud",
                Some(MIN_TOKEN_TTL_SECONDS - 1),
            ),
            (
                "token TTL above the maximum is rejected",
                "https://project.gateway.sqlite.cloud",
                Some(MAX_TOKEN_TTL_SECONDS + 1),
            ),
        ] {
            assert!(
                config(&env(project_url, token_ttl_seconds)).is_err(),
                "{case}"
            );
        }
    }

    #[test]
    fn validates_supabase_workspace_projection_config() {
        let sync_env = env("https://project.gateway.sqlite.cloud", None);

        for (case, supabase_url, anon_key, service_role_key, expect_ok) in [
            (
                "invalid Supabase URLs are rejected",
                "not-a-url",
                "anon-key",
                "service-role-key",
                false,
            ),
            (
                "remote Supabase URLs require HTTPS",
                "http://project.supabase.co",
                "anon-key",
                "service-role-key",
                false,
            ),
            (
                "localhost Supabase URLs allow HTTP",
                "http://localhost:54321",
                "anon-key",
                "service-role-key",
                true,
            ),
            (
                "IPv4 loopback Supabase URLs allow HTTP",
                "http://127.0.0.1:54321",
                "anon-key",
                "service-role-key",
                true,
            ),
            (
                "IPv6 loopback Supabase URLs allow HTTP",
                "http://[::1]:54321",
                "anon-key",
                "service-role-key",
                true,
            ),
            (
                "Supabase URL paths are rejected",
                "https://project.supabase.co/path",
                "anon-key",
                "service-role-key",
                false,
            ),
            (
                "empty Supabase anon keys are rejected",
                "https://project.supabase.co",
                "   ",
                "service-role-key",
                false,
            ),
            (
                "empty Supabase service-role keys are rejected",
                "https://project.supabase.co",
                "anon-key",
                "   ",
                false,
            ),
        ] {
            let result = SyncConfig::from_env(&sync_env, supabase_url, anon_key, service_role_key);
            assert_eq!(result.is_ok(), expect_ok, "{case}");
        }
    }
}
