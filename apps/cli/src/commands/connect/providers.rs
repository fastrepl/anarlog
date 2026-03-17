use crate::cli::{ConnectProvider, ConnectionType};

pub(crate) const STT_PROVIDERS: &[ConnectProvider] = &[
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

pub(crate) const LLM_PROVIDERS: &[ConnectProvider] = &[
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

impl ConnectProvider {
    pub(crate) fn id(&self) -> &'static str {
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

    pub(crate) fn is_local(&self) -> bool {
        match self {
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            Self::Cactus => true,
            Self::Ollama | Self::Lmstudio => true,
            _ => false,
        }
    }

    pub(crate) fn default_base_url(&self) -> Option<&'static str> {
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

    pub(crate) fn valid_for(&self, ct: ConnectionType) -> bool {
        match ct {
            ConnectionType::Stt => STT_PROVIDERS.contains(self),
            ConnectionType::Llm => LLM_PROVIDERS.contains(self),
        }
    }
}
