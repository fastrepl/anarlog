pub use crate::cli::{ConnectProvider, ConnectionType};
use crate::config::desktop;
use crate::error::{CliError, CliResult};

impl std::fmt::Display for ConnectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stt => write!(f, "stt"),
            Self::Llm => write!(f, "llm"),
        }
    }
}

impl std::fmt::Display for ConnectProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id())
    }
}

const STT_PROVIDERS: &[ConnectProvider] = &[
    ConnectProvider::Deepgram,
    ConnectProvider::Soniox,
    ConnectProvider::Assemblyai,
    ConnectProvider::Openai,
    ConnectProvider::Gladia,
    ConnectProvider::Elevenlabs,
    ConnectProvider::Mistral,
    ConnectProvider::Fireworks,
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    ConnectProvider::Cactus,
    ConnectProvider::Custom,
];

const LLM_PROVIDERS: &[ConnectProvider] = &[
    ConnectProvider::Openai,
    ConnectProvider::Anthropic,
    ConnectProvider::Openrouter,
    ConnectProvider::GoogleGenerativeAi,
    ConnectProvider::Mistral,
    ConnectProvider::AzureOpenai,
    ConnectProvider::AzureAi,
    ConnectProvider::Ollama,
    ConnectProvider::Lmstudio,
    ConnectProvider::Custom,
];

impl ConnectProvider {
    fn id(&self) -> &'static str {
        match self {
            Self::Deepgram => "deepgram",
            Self::Soniox => "soniox",
            Self::Assemblyai => "assemblyai",
            Self::Openai => "openai",
            Self::Gladia => "gladia",
            Self::Elevenlabs => "elevenlabs",
            Self::Mistral => "mistral",
            Self::Fireworks => "fireworks",
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            Self::Cactus => "cactus",
            Self::Anthropic => "anthropic",
            Self::Openrouter => "openrouter",
            Self::GoogleGenerativeAi => "google_generative_ai",
            Self::AzureOpenai => "azure_openai",
            Self::AzureAi => "azure_ai",
            Self::Ollama => "ollama",
            Self::Lmstudio => "lmstudio",
            Self::Custom => "custom",
        }
    }

    fn is_local(&self) -> bool {
        match self {
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            Self::Cactus => true,
            Self::Ollama | Self::Lmstudio => true,
            _ => false,
        }
    }

    fn default_base_url(&self) -> Option<&'static str> {
        match self {
            Self::Deepgram => Some("https://api.deepgram.com/v1"),
            Self::Soniox => Some("https://api.soniox.com"),
            Self::Assemblyai => Some("https://api.assemblyai.com"),
            Self::Openai => Some("https://api.openai.com/v1"),
            Self::Gladia => Some("https://api.gladia.io"),
            Self::Elevenlabs => Some("https://api.elevenlabs.io"),
            Self::Mistral => Some("https://api.mistral.ai/v1"),
            Self::Fireworks => Some("https://api.fireworks.ai"),
            Self::Anthropic => Some("https://api.anthropic.com/v1"),
            Self::Openrouter => Some("https://openrouter.ai/api/v1"),
            Self::GoogleGenerativeAi => Some("https://generativelanguage.googleapis.com/v1beta"),
            Self::Ollama => Some("http://127.0.0.1:11434/v1"),
            Self::Lmstudio => Some("http://127.0.0.1:1234/v1"),
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            Self::Cactus => None,
            Self::AzureOpenai | Self::AzureAi | Self::Custom => None,
        }
    }

    fn valid_for(&self, ct: ConnectionType) -> bool {
        match ct {
            ConnectionType::Stt => STT_PROVIDERS.contains(self),
            ConnectionType::Llm => LLM_PROVIDERS.contains(self),
        }
    }
}

pub struct Args {
    pub connection_type: Option<ConnectionType>,
    pub provider: Option<ConnectProvider>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

pub fn run(args: Args) -> CliResult<()> {
    let interactive = std::io::IsTerminal::is_terminal(&std::io::stdin());

    let connection_type = match args.connection_type {
        Some(ct) => ct,
        None if interactive => prompt_connection_type()?,
        None => {
            return Err(CliError::required_argument_with_hint(
                "--type",
                "pass --type stt or --type llm (interactive prompts require a terminal)",
            ));
        }
    };

    let providers = match connection_type {
        ConnectionType::Stt => STT_PROVIDERS,
        ConnectionType::Llm => LLM_PROVIDERS,
    };

    let provider = match args.provider {
        Some(p) => {
            if !p.valid_for(connection_type) {
                return Err(CliError::invalid_argument(
                    "--provider",
                    p.id(),
                    format!("not a valid {connection_type} provider"),
                ));
            }
            p
        }
        None if interactive => prompt_provider(providers)?,
        None => {
            return Err(CliError::required_argument_with_hint(
                "--provider",
                "pass --provider <name> (interactive prompts require a terminal)",
            ));
        }
    };

    let base_url = match args.base_url {
        Some(url) => Some(url),
        None if provider.is_local() && provider.default_base_url().is_none() => None,
        None if provider.default_base_url().is_some() => {
            let default = provider.default_base_url().unwrap();
            if interactive {
                prompt_base_url(Some(default))?
            } else {
                Some(default.to_string())
            }
        }
        None if interactive => prompt_base_url(None)?,
        None => {
            return Err(CliError::required_argument_with_hint(
                "--base-url",
                format!("{} requires a base URL", provider.id()),
            ));
        }
    };

    let api_key = match args.api_key {
        Some(key) => Some(key),
        None if provider.is_local() => None,
        None if interactive => prompt_api_key(provider)?,
        None => {
            return Err(CliError::required_argument_with_hint(
                "--api-key",
                "pass --api-key <key> (interactive prompts require a terminal)",
            ));
        }
    };

    let type_key = connection_type.to_string();
    let provider_id = provider.id();

    let mut provider_config = serde_json::Map::new();
    if let Some(url) = &base_url {
        provider_config.insert("base_url".into(), serde_json::Value::String(url.clone()));
    }
    if let Some(key) = &api_key {
        provider_config.insert("api_key".into(), serde_json::Value::String(key.clone()));
    }

    let patch = serde_json::json!({
        "ai": {
            format!("current_{type_key}_provider"): provider_id,
            &type_key: {
                provider_id: provider_config,
            }
        }
    });

    let paths = desktop::resolve_paths();
    desktop::save_settings(&paths.settings_path, patch)
        .map_err(|e| CliError::operation_failed("save settings", e.to_string()))?;

    eprintln!(
        "Saved {type_key} provider: {provider_id} -> {}",
        paths.settings_path.display()
    );
    Ok(())
}

fn prompt_connection_type() -> CliResult<ConnectionType> {
    let options = vec!["stt", "llm"];
    let answer = inquire::Select::new("Connection type:", options)
        .prompt()
        .map_err(|e| CliError::msg(e.to_string()))?;
    match answer {
        "stt" => Ok(ConnectionType::Stt),
        "llm" => Ok(ConnectionType::Llm),
        _ => unreachable!(),
    }
}

fn prompt_provider(providers: &[ConnectProvider]) -> CliResult<ConnectProvider> {
    let labels: Vec<&str> = providers.iter().map(|p| p.id()).collect();
    let idx = inquire::Select::new("Provider:", labels)
        .prompt()
        .map_err(|e| CliError::msg(e.to_string()))?;
    let provider = providers.iter().find(|p| p.id() == idx).unwrap();
    Ok(*provider)
}

fn prompt_base_url(default: Option<&str>) -> CliResult<Option<String>> {
    let mut prompt = inquire::Text::new("Base URL:");
    if let Some(d) = default {
        prompt = prompt.with_default(d);
    }
    let answer = prompt.prompt().map_err(|e| CliError::msg(e.to_string()))?;
    let trimmed = answer.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

fn prompt_api_key(provider: ConnectProvider) -> CliResult<Option<String>> {
    let prompt_text = if provider.is_local() {
        "API key (optional, press Enter to skip):"
    } else {
        "API key:"
    };
    let answer = inquire::Password::new(prompt_text)
        .without_confirmation()
        .prompt()
        .map_err(|e| CliError::msg(e.to_string()))?;
    let trimmed = answer.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}
