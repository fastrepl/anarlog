use std::path::Path;
use std::sync::OnceLock;

use envy::Error as EnvyError;
use serde::Deserialize;

fn default_port() -> u16 {
    3001
}

#[derive(Default, Deserialize)]
struct OptionalNangoEnv {
    #[serde(default, deserialize_with = "anlg_api_env::filter_empty")]
    nango_api_base: Option<String>,
    #[serde(default, deserialize_with = "anlg_api_env::filter_empty")]
    nango_api_key: Option<String>,
    #[serde(default, deserialize_with = "anlg_api_env::filter_empty")]
    nango_webhook_signing_key: Option<String>,
}

#[derive(Default, Deserialize)]
struct OptionalStripeEnv {
    #[serde(default, deserialize_with = "anlg_api_env::filter_empty")]
    stripe_secret_key: Option<String>,
    #[serde(default, deserialize_with = "anlg_api_env::filter_empty")]
    stripe_monthly_price_id: Option<String>,
    #[serde(default, deserialize_with = "anlg_api_env::filter_empty")]
    stripe_yearly_price_id: Option<String>,
}

#[derive(Default, Deserialize)]
struct OptionalPyannoteEnv {
    #[serde(default, deserialize_with = "anlg_api_env::filter_empty")]
    pyannote_api_key: Option<String>,
    #[serde(default, deserialize_with = "anlg_api_env::filter_empty")]
    pyannote_api_base: Option<String>,
}

#[derive(Default, Deserialize)]
struct OptionalLoopsEnv {
    #[serde(default, deserialize_with = "anlg_api_env::filter_empty")]
    loops_key: Option<String>,
}

#[derive(Deserialize)]
pub struct Env {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default, deserialize_with = "anlg_api_env::filter_empty")]
    pub sentry_dsn: Option<String>,
    #[serde(default, deserialize_with = "anlg_api_env::filter_empty")]
    pub posthog_api_key: Option<String>,
    #[serde(default)]
    pub anarlog_attachment_backup_gc_enabled: bool,
    #[serde(default, deserialize_with = "anlg_api_env::filter_empty")]
    pub sqlitecloud_cloudsync_management_api_key: Option<String>,

    #[serde(flatten)]
    pub observability: crate::observability::Env,

    #[serde(flatten)]
    pub supabase: anlg_api_env::SupabaseEnv,
    #[serde(flatten)]
    pub sync: anlg_api_sync::SyncEnv,
    #[serde(flatten)]
    nango: OptionalNangoEnv,
    #[serde(flatten)]
    stripe: OptionalStripeEnv,
    #[serde(flatten)]
    pyannote: OptionalPyannoteEnv,
    #[serde(default, deserialize_with = "anlg_api_env::filter_empty")]
    pub exa_api_key: Option<String>,
    #[serde(default, deserialize_with = "anlg_api_env::filter_empty")]
    pub jina_api_key: Option<String>,
    #[serde(flatten)]
    loops: OptionalLoopsEnv,

    #[serde(flatten)]
    pub resend: anlg_api_env::ResendEnv,

    #[serde(flatten)]
    pub llm: anlg_llm_proxy::Env,
    #[serde(flatten)]
    pub stt: anlg_transcribe_proxy::Env,
}

// Raw environment resolved exactly once at startup: every optional integration
// group is either absent or completely validated, so router/state construction
// can consume the concrete groups without re-deriving or re-validating them.
pub struct RuntimeConfig {
    env: Env,
    pub nango: Option<anlg_api_env::NangoEnv>,
    pub subscription: Option<(anlg_api_env::StripeEnv, anlg_api_env::LoopsEnv)>,
    pub pyannote: Option<anlg_api_env::PyannoteEnv>,
    pub research: Option<anlg_api_research::ResearchConfig>,
}

impl std::ops::Deref for RuntimeConfig {
    type Target = Env;

    fn deref(&self) -> &Self::Target {
        &self.env
    }
}

impl RuntimeConfig {
    pub(crate) fn resolve(env: Env) -> Result<Self, String> {
        validate_supabase_env(&env.supabase)?;
        let nango = resolve_nango(&env.nango)?;
        let subscription = resolve_subscription(&env.stripe, &env.loops)?;
        let pyannote = resolve_pyannote(&env.pyannote)?;
        let research = resolve_research(&env.exa_api_key, &env.jina_api_key)?;

        if !cfg!(debug_assertions)
            && subscription
                .as_ref()
                .is_some_and(|(stripe, _)| is_stripe_test_key(&stripe.stripe_secret_key))
        {
            return Err("STRIPE_SECRET_KEY must be a live key in production".to_string());
        }
        if env.anarlog_attachment_backup_gc_enabled && subscription.is_none() {
            return Err(
                "Stripe and Loops configuration is required when ANARLOG_ATTACHMENT_BACKUP_GC_ENABLED is true"
                    .to_string(),
            );
        }

        Ok(Self {
            env,
            nango,
            subscription,
            pyannote,
            research,
        })
    }
}

fn resolve_nango(nango: &OptionalNangoEnv) -> Result<Option<anlg_api_env::NangoEnv>, String> {
    let configured = nango.nango_api_base.is_some()
        || nango.nango_api_key.is_some()
        || nango.nango_webhook_signing_key.is_some();
    if !configured {
        return Ok(None);
    }

    Ok(Some(anlg_api_env::NangoEnv {
        nango_api_base: nango.nango_api_base.clone(),
        nango_api_key: required_integration_value(
            &nango.nango_api_key,
            "NANGO_API_KEY",
            "Nango is configured",
        )?,
        nango_webhook_signing_key: required_integration_value(
            &nango.nango_webhook_signing_key,
            "NANGO_WEBHOOK_SIGNING_KEY",
            "Nango is configured",
        )?,
    }))
}

fn resolve_subscription(
    stripe: &OptionalStripeEnv,
    loops: &OptionalLoopsEnv,
) -> Result<Option<(anlg_api_env::StripeEnv, anlg_api_env::LoopsEnv)>, String> {
    let stripe_configured = stripe.stripe_secret_key.is_some()
        || stripe.stripe_monthly_price_id.is_some()
        || stripe.stripe_yearly_price_id.is_some();
    let stripe = stripe_configured
        .then(|| -> Result<_, String> {
            Ok(anlg_api_env::StripeEnv {
                stripe_secret_key: required_integration_value(
                    &stripe.stripe_secret_key,
                    "STRIPE_SECRET_KEY",
                    "subscriptions are configured",
                )?,
                stripe_monthly_price_id: required_integration_value(
                    &stripe.stripe_monthly_price_id,
                    "STRIPE_MONTHLY_PRICE_ID",
                    "subscriptions are configured",
                )?,
                stripe_yearly_price_id: required_integration_value(
                    &stripe.stripe_yearly_price_id,
                    "STRIPE_YEARLY_PRICE_ID",
                    "subscriptions are configured",
                )?,
            })
        })
        .transpose()?;
    let loops = loops
        .loops_key
        .as_ref()
        .map(|loops_key| anlg_api_env::LoopsEnv {
            loops_key: loops_key.clone(),
        });

    match (stripe, loops) {
        (None, None) => Ok(None),
        (Some(stripe), Some(loops)) => Ok(Some((stripe, loops))),
        (Some(_), None) => {
            Err("LOOPS_KEY is required when subscriptions are configured".to_string())
        }
        (None, Some(_)) => {
            Err("Stripe configuration is required when subscriptions are configured".to_string())
        }
    }
}

fn resolve_pyannote(
    pyannote: &OptionalPyannoteEnv,
) -> Result<Option<anlg_api_env::PyannoteEnv>, String> {
    let configured = pyannote.pyannote_api_key.is_some() || pyannote.pyannote_api_base.is_some();
    if !configured {
        return Ok(None);
    }

    Ok(Some(anlg_api_env::PyannoteEnv {
        pyannote_api_key: required_integration_value(
            &pyannote.pyannote_api_key,
            "PYANNOTE_API_KEY",
            "pyannote is configured",
        )?,
        pyannote_api_base: pyannote
            .pyannote_api_base
            .clone()
            .unwrap_or_else(|| "https://api.pyannote.ai".to_string()),
    }))
}

fn resolve_research(
    exa_api_key: &Option<String>,
    jina_api_key: &Option<String>,
) -> Result<Option<anlg_api_research::ResearchConfig>, String> {
    match (exa_api_key, jina_api_key) {
        (None, None) => Ok(None),
        (Some(exa_api_key), Some(jina_api_key)) => Ok(Some(anlg_api_research::ResearchConfig {
            exa_api_key: exa_api_key.clone(),
            jina_api_key: jina_api_key.clone(),
        })),
        (Some(_), None) => Err("JINA_API_KEY is required when research is configured".to_string()),
        (None, Some(_)) => Err("EXA_API_KEY is required when research is configured".to_string()),
    }
}

fn required_integration_value(
    value: &Option<String>,
    variable: &str,
    condition: &str,
) -> Result<String, String> {
    value
        .clone()
        .ok_or_else(|| format!("{variable} is required when {condition}"))
}

static ENV: OnceLock<RuntimeConfig> = OnceLock::new();

pub fn env() -> &'static RuntimeConfig {
    ENV.get_or_init(|| {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or(manifest_dir);

        let _ = dotenvy::from_path(repo_root.join(".env.supabase"));
        let _ = dotenvy::from_path(manifest_dir.join(".env"));
        let env: Env =
            envy::from_env().unwrap_or_else(|error| panic!("{}", format_env_error(error)));
        RuntimeConfig::resolve(env)
            .unwrap_or_else(|error| panic!("Failed to load environment: {error}"))
    })
}

fn validate_supabase_env(env: &anlg_api_env::SupabaseEnv) -> Result<(), String> {
    for (value, variable) in [
        (&env.supabase_url, "SUPABASE_URL"),
        (&env.supabase_anon_key, "SUPABASE_ANON_KEY"),
        (&env.supabase_service_role_key, "SUPABASE_SERVICE_ROLE_KEY"),
    ] {
        if value.trim().is_empty() {
            return Err(format!("{variable} must not be empty"));
        }
    }

    Ok(())
}

fn is_stripe_test_key(key: &str) -> bool {
    key.starts_with("sk_test_") || key.starts_with("rk_test_")
}

fn format_env_error(error: EnvyError) -> String {
    match error {
        EnvyError::MissingValue(field) => {
            let env_var = field_name_to_env_var(field);
            format!("Failed to load environment: missing {env_var} (field: {field})")
        }
        other => format!("Failed to load environment: {other}"),
    }
}

fn field_name_to_env_var(field: &str) -> String {
    field
        .chars()
        .flat_map(|ch| ch.to_uppercase())
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct SyncOnlyEnv {
        #[serde(flatten)]
        sync: anlg_api_sync::SyncEnv,
    }

    #[test]
    fn deserializes_cloudsync_ttl_from_environment_string() {
        let env: SyncOnlyEnv = envy::from_iter([(
            "ANARLOG_CLOUDSYNC_TOKEN_TTL_SECONDS".to_string(),
            "300".to_string(),
        )])
        .unwrap();

        assert_eq!(env.sync.anarlog_cloudsync_token_ttl_seconds, Some(300));
    }

    #[test]
    fn durable_cleanup_is_opt_in() {
        #[derive(Deserialize)]
        struct CleanupOnlyEnv {
            #[serde(default)]
            anarlog_attachment_backup_gc_enabled: bool,
        }

        let disabled: CleanupOnlyEnv = envy::from_iter(Vec::<(String, String)>::new()).unwrap();
        let enabled: CleanupOnlyEnv = envy::from_iter([(
            "ANARLOG_ATTACHMENT_BACKUP_GC_ENABLED".to_string(),
            "true".to_string(),
        )])
        .unwrap();

        assert!(!disabled.anarlog_attachment_backup_gc_enabled);
        assert!(enabled.anarlog_attachment_backup_gc_enabled);
    }

    #[test]
    fn core_supabase_configuration_remains_required() {
        #[derive(Deserialize)]
        struct SupabaseOnlyEnv {
            #[serde(flatten)]
            _supabase: anlg_api_env::SupabaseEnv,
        }

        for missing in [
            "SUPABASE_URL",
            "SUPABASE_ANON_KEY",
            "SUPABASE_SERVICE_ROLE_KEY",
        ] {
            let values = [
                ("SUPABASE_URL", "http://127.0.0.1:54321"),
                ("SUPABASE_ANON_KEY", "anon-key"),
                ("SUPABASE_SERVICE_ROLE_KEY", "service-role-key"),
            ]
            .into_iter()
            .filter(|(key, _)| *key != missing)
            .map(|(key, value)| (key.to_string(), value.to_string()));
            let error = match envy::from_iter::<_, SupabaseOnlyEnv>(values) {
                Ok(_) => panic!("{missing} should remain required"),
                Err(error) => error,
            };

            assert!(matches!(
                error,
                EnvyError::MissingValue(field) if field == missing.to_lowercase()
            ));
        }
    }

    fn some(value: &str) -> Option<String> {
        Some(value.to_string())
    }

    #[test]
    fn nango_group_is_all_or_nothing() {
        let cases: Vec<(OptionalNangoEnv, Result<bool, &str>)> = vec![
            (OptionalNangoEnv::default(), Ok(false)),
            (
                OptionalNangoEnv {
                    nango_api_base: None,
                    nango_api_key: some("key"),
                    nango_webhook_signing_key: some("signing"),
                },
                Ok(true),
            ),
            (
                OptionalNangoEnv {
                    nango_api_base: some("https://nango.example"),
                    nango_api_key: some("key"),
                    nango_webhook_signing_key: some("signing"),
                },
                Ok(true),
            ),
            (
                OptionalNangoEnv {
                    nango_api_base: some("https://nango.example"),
                    nango_api_key: None,
                    nango_webhook_signing_key: some("signing"),
                },
                Err("NANGO_API_KEY is required when Nango is configured"),
            ),
            (
                OptionalNangoEnv {
                    nango_api_base: None,
                    nango_api_key: some("key"),
                    nango_webhook_signing_key: None,
                },
                Err("NANGO_WEBHOOK_SIGNING_KEY is required when Nango is configured"),
            ),
        ];

        for (nango, expected) in cases {
            assert_eq!(
                resolve_nango(&nango).map(|resolved| resolved.is_some()),
                expected.map_err(str::to_string)
            );
        }
    }

    #[test]
    fn subscription_group_requires_complete_stripe_and_loops() {
        let complete_stripe = OptionalStripeEnv {
            stripe_secret_key: some("sk_live_1"),
            stripe_monthly_price_id: some("price_m"),
            stripe_yearly_price_id: some("price_y"),
        };
        let loops = OptionalLoopsEnv {
            loops_key: some("loops"),
        };
        let resolved = |stripe: &OptionalStripeEnv, loops: &OptionalLoopsEnv| {
            resolve_subscription(stripe, loops).map(|resolved| resolved.is_some())
        };

        assert_eq!(
            resolved(&OptionalStripeEnv::default(), &OptionalLoopsEnv::default()),
            Ok(false)
        );
        assert_eq!(resolved(&complete_stripe, &loops), Ok(true));
        assert_eq!(
            resolved(
                &OptionalStripeEnv {
                    stripe_secret_key: some("sk_live_1"),
                    stripe_monthly_price_id: None,
                    stripe_yearly_price_id: some("price_y"),
                },
                &loops,
            ),
            Err(
                "STRIPE_MONTHLY_PRICE_ID is required when subscriptions are configured".to_string()
            )
        );
        assert_eq!(
            resolved(&complete_stripe, &OptionalLoopsEnv::default()),
            Err("LOOPS_KEY is required when subscriptions are configured".to_string())
        );
        assert_eq!(
            resolved(&OptionalStripeEnv::default(), &loops),
            Err("Stripe configuration is required when subscriptions are configured".to_string())
        );
    }

    #[test]
    fn pyannote_group_requires_key_and_defaults_base() {
        let resolved_base = |pyannote: &OptionalPyannoteEnv| {
            resolve_pyannote(pyannote)
                .map(|resolved| resolved.map(|pyannote| pyannote.pyannote_api_base))
        };

        assert_eq!(resolved_base(&OptionalPyannoteEnv::default()), Ok(None));
        assert_eq!(
            resolved_base(&OptionalPyannoteEnv {
                pyannote_api_key: some("key"),
                pyannote_api_base: None,
            }),
            Ok(some("https://api.pyannote.ai"))
        );
        assert_eq!(
            resolved_base(&OptionalPyannoteEnv {
                pyannote_api_key: None,
                pyannote_api_base: some("https://pyannote.example"),
            }),
            Err("PYANNOTE_API_KEY is required when pyannote is configured".to_string())
        );
    }

    #[test]
    fn research_group_requires_both_keys() {
        let resolved = |exa: &Option<String>, jina: &Option<String>| {
            resolve_research(exa, jina).map(|resolved| resolved.is_some())
        };

        assert_eq!(resolved(&None, &None), Ok(false));
        assert_eq!(resolved(&some("exa"), &some("jina")), Ok(true));
        assert_eq!(
            resolved(&some("exa"), &None),
            Err("JINA_API_KEY is required when research is configured".to_string())
        );
        assert_eq!(
            resolved(&None, &some("jina")),
            Err("EXA_API_KEY is required when research is configured".to_string())
        );
    }

    #[test]
    fn core_supabase_configuration_rejects_empty_values() {
        for (field, expected) in [
            ("url", "SUPABASE_URL must not be empty"),
            ("anon", "SUPABASE_ANON_KEY must not be empty"),
            (
                "service_role",
                "SUPABASE_SERVICE_ROLE_KEY must not be empty",
            ),
        ] {
            let mut env = anlg_api_env::SupabaseEnv {
                supabase_url: "http://127.0.0.1:54321".to_string(),
                supabase_anon_key: "anon-key".to_string(),
                supabase_service_role_key: "service-role-key".to_string(),
            };
            match field {
                "url" => env.supabase_url.clear(),
                "anon" => env.supabase_anon_key.clear(),
                "service_role" => env.supabase_service_role_key.clear(),
                _ => unreachable!(),
            }

            assert_eq!(validate_supabase_env(&env), Err(expected.to_string()));
        }
    }
}
