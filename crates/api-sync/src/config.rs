#[derive(Clone)]
pub struct SyncConfig {
    pub(crate) supabase_url: String,
    pub(crate) supabase_anon_key: String,
    pub(crate) supabase_service_role_key: String,
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
            supabase_url: validate_supabase_url(supabase_url.into())?,
            supabase_anon_key,
            supabase_service_role_key,
        })
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn validates_supabase_workspace_projection_config() {
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
            let result = SyncConfig::new(supabase_url, anon_key, service_role_key);
            assert_eq!(result.is_ok(), expect_ok, "{case}");
        }
    }
}
