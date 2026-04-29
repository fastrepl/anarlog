use std::path::Path;
use std::sync::OnceLock;

use envy::Error as EnvyError;
use serde::Deserialize;

fn default_port() -> u16 {
    3001
}

#[derive(Deserialize)]
pub struct Env {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default, deserialize_with = "hypr_api_env::filter_empty")]
    pub sentry_dsn: Option<String>,
    #[serde(default, deserialize_with = "hypr_api_env::filter_empty")]
    pub posthog_api_key: Option<String>,

    #[serde(flatten)]
    pub observability: crate::observability::Env,

    #[serde(flatten)]
    pub supabase: hypr_api_env::SupabaseEnv,
    #[serde(flatten)]
    pub nango: hypr_api_env::NangoEnv,
    #[serde(flatten)]
    pub stripe: hypr_api_env::StripeEnv,
    #[serde(flatten)]
    pub pyannote: hypr_api_env::PyannoteEnv,
    #[serde(flatten)]
    pub github_app: hypr_api_support::GitHubAppEnv,
    #[serde(flatten)]
    pub support_database: hypr_api_support::SupportDatabaseEnv,

    pub cactus_api_key: String,
    pub exa_api_key: String,
    pub jina_api_key: String,

    #[serde(flatten)]
    pub loops: hypr_api_env::LoopsEnv,

    #[serde(flatten)]
    pub llm: hypr_llm_proxy::Env,
    #[serde(flatten)]
    pub stt: hypr_transcribe_proxy::Env,
}

static ENV: OnceLock<Env> = OnceLock::new();

pub fn env() -> &'static Env {
    ENV.get_or_init(|| {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or(manifest_dir);

        let _ = dotenvy::from_path(repo_root.join(".env.supabase"));
        let _ = dotenvy::from_path(manifest_dir.join(".env"));
        envy::from_env().unwrap_or_else(|error| panic!("{}", format_env_error(error)))
    })
}

fn format_env_error(error: EnvyError) -> String {
    match error {
        EnvyError::MissingValue(field) => {
            let env_var = field_name_to_env_var(&field);
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
