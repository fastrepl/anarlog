use std::{path::PathBuf, time::Duration};

use clap::{Parser, ValueEnum};
use hypr_activity_capture::{
    BrowserPolicy, BundleRule, Capabilities, CaptureAccess, CapturePolicy, DomainRule, PolicyMode,
};

use crate::vlm::{
    DEFAULT_VLM_MODEL_NAME, DEFAULT_VLM_PROMPT, DEFAULT_VLM_TIMEOUT_SECS, VlmSettings,
};

const DEFAULT_POLL_INTERVAL_MS: u64 = 750;
const DEFAULT_SCREENSHOT_DWELL_MS: u64 = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum RuntimePreference {
    Auto,
    Watch,
    Poll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaptureRuntimeMode {
    Watch,
    Poll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum PolicyModeArg {
    OptIn,
    OptOut,
}

impl From<PolicyModeArg> for PolicyMode {
    fn from(value: PolicyModeArg) -> Self {
        match value {
            PolicyModeArg::OptIn => Self::OptIn,
            PolicyModeArg::OptOut => Self::OptOut,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum AccessArg {
    None,
    Metadata,
    Url,
    Full,
}

impl From<AccessArg> for CaptureAccess {
    fn from(value: AccessArg) -> Self {
        match value {
            AccessArg::None => Self::None,
            AccessArg::Metadata => Self::Metadata,
            AccessArg::Url => Self::Url,
            AccessArg::Full => Self::Full,
        }
    }
}

#[derive(Debug, Clone, Parser)]
#[command(
    about = "Inspect activity capture transitions, policy, and raw events in a TUI",
    long_about = None
)]
pub struct Options {
    #[arg(
        long = "poll-ms",
        default_value_t = DEFAULT_POLL_INTERVAL_MS,
        value_parser = clap::value_parser!(u64).range(1..)
    )]
    pub poll_ms: u64,

    #[arg(long, value_enum, default_value_t = RuntimePreference::Auto)]
    pub mode: RuntimePreference,

    #[arg(long, value_enum, conflicts_with = "metadata_only")]
    policy_mode: Option<PolicyModeArg>,

    #[arg(long, conflicts_with = "policy_mode")]
    pub metadata_only: bool,

    #[arg(long = "app-rule", value_name = "APP=ACCESS", value_parser = parse_app_rule)]
    pub app_rules: Vec<BundleRule>,

    #[arg(
        long = "domain-rule",
        value_name = "[*.]DOMAIN=ACCESS",
        value_parser = parse_domain_rule
    )]
    pub domain_rules: Vec<DomainRule>,

    #[arg(long)]
    pub allow_private_browsing: bool,

    #[arg(long)]
    pub keep_query: bool,

    #[arg(long)]
    pub keep_fragment: bool,

    #[arg(long)]
    pub allow_text_without_url: bool,

    #[arg(long)]
    pub no_emit_initial: bool,

    #[arg(long)]
    pub no_color: bool,

    #[arg(long)]
    pub once: bool,

    #[arg(
        long = "screenshot-dwell-ms",
        default_value_t = DEFAULT_SCREENSHOT_DWELL_MS,
        value_parser = clap::value_parser!(u64).range(0..)
    )]
    pub screenshot_dwell_ms: u64,

    #[arg(long = "vlm-model", value_name = "PATH")]
    pub vlm_model: Option<PathBuf>,

    #[arg(long = "vlm-model-name", default_value = DEFAULT_VLM_MODEL_NAME)]
    pub vlm_model_name: String,

    #[arg(long = "vlm-prompt", default_value = DEFAULT_VLM_PROMPT)]
    pub vlm_prompt: String,

    #[arg(
        long = "vlm-timeout-secs",
        default_value_t = DEFAULT_VLM_TIMEOUT_SECS,
        value_parser = clap::value_parser!(u64).range(1..)
    )]
    pub vlm_timeout_secs: u64,
}

impl Options {
    pub fn poll_interval(&self) -> Duration {
        Duration::from_millis(self.poll_ms)
    }

    pub fn policy(&self) -> CapturePolicy {
        CapturePolicy {
            mode: self.effective_policy_mode(),
            app_rules: self.app_rules.clone(),
            browser: BrowserPolicy {
                rules: self.domain_rules.clone(),
                require_url_for_text_access: !self.allow_text_without_url,
                block_private_browsing: !self.allow_private_browsing,
                strip_query: !self.keep_query,
                strip_fragment: !self.keep_fragment,
            },
        }
    }

    pub fn resolve_runtime_mode(
        &self,
        capabilities: Capabilities,
    ) -> Result<CaptureRuntimeMode, String> {
        match self.mode {
            RuntimePreference::Auto if capabilities.can_watch => Ok(CaptureRuntimeMode::Watch),
            RuntimePreference::Auto => Ok(CaptureRuntimeMode::Poll),
            RuntimePreference::Watch if capabilities.can_watch => Ok(CaptureRuntimeMode::Watch),
            RuntimePreference::Watch => {
                Err("watch mode was requested, but this platform capture does not support watch streams".to_string())
            }
            RuntimePreference::Poll => Ok(CaptureRuntimeMode::Poll),
        }
    }

    pub fn runtime_label(&self, resolved: CaptureRuntimeMode) -> String {
        match (self.mode, resolved) {
            (RuntimePreference::Auto, CaptureRuntimeMode::Watch) => "watch(auto)".to_string(),
            (RuntimePreference::Auto, CaptureRuntimeMode::Poll) => {
                "poll(auto-fallback)".to_string()
            }
            (RuntimePreference::Watch, CaptureRuntimeMode::Watch) => "watch".to_string(),
            (RuntimePreference::Poll, CaptureRuntimeMode::Poll) => "poll".to_string(),
            (RuntimePreference::Watch, CaptureRuntimeMode::Poll) => "poll".to_string(),
            (RuntimePreference::Poll, CaptureRuntimeMode::Watch) => "watch".to_string(),
        }
    }

    pub fn policy_label(&self) -> String {
        format!(
            "mode={} apps={} domains={}",
            policy_mode_label(self.effective_policy_mode()),
            self.app_rules.len(),
            self.domain_rules.len(),
        )
    }

    pub fn browser_policy_label(&self) -> String {
        format!(
            "private={} query={} fragment={} text-needs-url={}",
            if self.allow_private_browsing {
                "allow"
            } else {
                "block"
            },
            if self.keep_query { "keep" } else { "strip" },
            if self.keep_fragment { "keep" } else { "strip" },
            yes_no(!self.allow_text_without_url),
        )
    }

    pub fn vlm_settings(&self) -> Option<VlmSettings> {
        self.vlm_model.as_ref().map(|model_path| VlmSettings {
            model_path: model_path.clone(),
            model_name: self.vlm_model_name.clone(),
            prompt: self.vlm_prompt.clone(),
            timeout: Duration::from_secs(self.vlm_timeout_secs),
        })
    }

    fn effective_policy_mode(&self) -> PolicyMode {
        if self.metadata_only {
            PolicyMode::OptIn
        } else {
            self.policy_mode.unwrap_or(PolicyModeArg::OptOut).into()
        }
    }
}

fn parse_app_rule(value: &str) -> Result<BundleRule, String> {
    let (bundle_id, access) = split_rule(value)?;
    Ok(BundleRule {
        bundle_id: bundle_id.to_string(),
        access,
    })
}

fn parse_domain_rule(value: &str) -> Result<DomainRule, String> {
    let (domain, access) = split_rule(value)?;
    let include_subdomains = domain.starts_with("*.");
    let domain = if include_subdomains {
        &domain[2..]
    } else {
        domain
    };

    let domain = domain.trim();
    if domain.is_empty() {
        return Err("domain rule is missing a domain".to_string());
    }

    Ok(DomainRule {
        domain: domain.to_string(),
        include_subdomains,
        access,
    })
}

fn split_rule(value: &str) -> Result<(&str, CaptureAccess), String> {
    let Some((key, access)) = value.rsplit_once('=') else {
        return Err("expected VALUE=ACCESS".to_string());
    };

    let key = key.trim();
    if key.is_empty() {
        return Err("rule key is empty".to_string());
    }

    let access = AccessArg::from_str(access.trim(), true)
        .map(CaptureAccess::from)
        .map_err(|_| "access must be one of: none, metadata, url, full".to_string())?;

    Ok((key, access))
}

fn policy_mode_label(mode: PolicyMode) -> &'static str {
    match mode {
        PolicyMode::OptIn => "opt-in",
        PolicyMode::OptOut => "opt-out",
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> Options {
        Options {
            poll_ms: DEFAULT_POLL_INTERVAL_MS,
            mode: RuntimePreference::Auto,
            policy_mode: None,
            metadata_only: false,
            app_rules: Vec::new(),
            domain_rules: Vec::new(),
            allow_private_browsing: false,
            keep_query: false,
            keep_fragment: false,
            allow_text_without_url: false,
            no_emit_initial: false,
            no_color: false,
            once: false,
            screenshot_dwell_ms: DEFAULT_SCREENSHOT_DWELL_MS,
            vlm_model: None,
            vlm_model_name: DEFAULT_VLM_MODEL_NAME.to_string(),
            vlm_prompt: DEFAULT_VLM_PROMPT.to_string(),
            vlm_timeout_secs: DEFAULT_VLM_TIMEOUT_SECS,
        }
    }

    #[test]
    fn parses_domain_rule_with_subdomains() {
        let rule = parse_domain_rule("*.example.com=full").unwrap();

        assert_eq!(rule.domain, "example.com");
        assert!(rule.include_subdomains);
        assert_eq!(rule.access, CaptureAccess::Full);
    }

    #[test]
    fn policy_uses_browser_flags_and_rules() {
        let mut options = options();
        options.app_rules = vec![BundleRule {
            bundle_id: "com.example.app".to_string(),
            access: CaptureAccess::Url,
        }];
        options.domain_rules = vec![DomainRule {
            domain: "example.com".to_string(),
            include_subdomains: true,
            access: CaptureAccess::Full,
        }];
        options.allow_private_browsing = true;
        options.keep_query = true;
        options.keep_fragment = true;
        options.allow_text_without_url = true;

        let policy = options.policy();

        assert_eq!(policy.mode, PolicyMode::OptOut);
        assert_eq!(policy.app_rules.len(), 1);
        assert_eq!(policy.browser.rules.len(), 1);
        assert!(!policy.browser.block_private_browsing);
        assert!(!policy.browser.require_url_for_text_access);
        assert!(!policy.browser.strip_fragment);
        assert!(!policy.browser.strip_query);
    }

    #[test]
    fn explicit_watch_mode_errors_when_capability_is_missing() {
        let mut options = options();
        options.mode = RuntimePreference::Watch;

        let error = options
            .resolve_runtime_mode(Capabilities::default())
            .unwrap_err();

        assert!(error.contains("does not support watch streams"));
    }

    #[test]
    fn metadata_only_forces_opt_in_policy() {
        let mut options = options();
        options.metadata_only = true;

        assert_eq!(options.policy().mode, PolicyMode::OptIn);
    }
}
