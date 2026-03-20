use super::{ConnectProvider, ConnectionType};

pub(crate) struct ProviderMeta {
    pub provider: ConnectProvider,
    pub id: &'static str,
    pub display_name: &'static str,
    pub default_base_url: Option<&'static str>,
    pub capabilities: &'static [ConnectionType],
    pub is_local: bool,
    pub is_disabled: bool,
}

const CAP_STT: &[ConnectionType] = &[ConnectionType::Stt];
const CAP_LLM: &[ConnectionType] = &[ConnectionType::Llm];
const CAP_CAL: &[ConnectionType] = &[ConnectionType::Cal];
const CAP_STT_LLM: &[ConnectionType] = &[ConnectionType::Stt, ConnectionType::Llm];

pub(crate) const PROVIDERS: &[ProviderMeta] = &[
    // STT-only
    ProviderMeta {
        provider: ConnectProvider::Deepgram,
        id: "deepgram",
        display_name: "Deepgram",
        default_base_url: Some("https://api.deepgram.com/v1"),
        capabilities: CAP_STT,
        is_local: false,
        is_disabled: false,
    },
    ProviderMeta {
        provider: ConnectProvider::Soniox,
        id: "soniox",
        display_name: "Soniox",
        default_base_url: Some("https://api.soniox.com"),
        capabilities: CAP_STT,
        is_local: false,
        is_disabled: false,
    },
    ProviderMeta {
        provider: ConnectProvider::Assemblyai,
        id: "assemblyai",
        display_name: "AssemblyAI",
        default_base_url: Some("https://api.assemblyai.com"),
        capabilities: CAP_STT,
        is_local: false,
        is_disabled: false,
    },
    ProviderMeta {
        provider: ConnectProvider::Gladia,
        id: "gladia",
        display_name: "Gladia",
        default_base_url: Some("https://api.gladia.io"),
        capabilities: CAP_STT,
        is_local: false,
        is_disabled: false,
    },
    ProviderMeta {
        provider: ConnectProvider::Elevenlabs,
        id: "elevenlabs",
        display_name: "ElevenLabs",
        default_base_url: Some("https://api.elevenlabs.io"),
        capabilities: CAP_STT,
        is_local: false,
        is_disabled: false,
    },
    ProviderMeta {
        provider: ConnectProvider::Fireworks,
        id: "fireworks",
        display_name: "Fireworks",
        default_base_url: Some("https://api.fireworks.ai"),
        capabilities: CAP_STT,
        is_local: false,
        is_disabled: false,
    },
    // Dual (STT + LLM)
    ProviderMeta {
        provider: ConnectProvider::Openai,
        id: "openai",
        display_name: "OpenAI",
        default_base_url: Some("https://api.openai.com/v1"),
        capabilities: CAP_STT_LLM,
        is_local: false,
        is_disabled: false,
    },
    ProviderMeta {
        provider: ConnectProvider::Mistral,
        id: "mistral",
        display_name: "Mistral",
        default_base_url: Some("https://api.mistral.ai/v1"),
        capabilities: CAP_STT_LLM,
        is_local: false,
        is_disabled: false,
    },
    // LLM-only
    ProviderMeta {
        provider: ConnectProvider::Anthropic,
        id: "anthropic",
        display_name: "Anthropic",
        default_base_url: Some("https://api.anthropic.com/v1"),
        capabilities: CAP_LLM,
        is_local: false,
        is_disabled: false,
    },
    ProviderMeta {
        provider: ConnectProvider::Openrouter,
        id: "openrouter",
        display_name: "OpenRouter",
        default_base_url: Some("https://openrouter.ai/api/v1"),
        capabilities: CAP_LLM,
        is_local: false,
        is_disabled: false,
    },
    ProviderMeta {
        provider: ConnectProvider::GoogleGenerativeAi,
        id: "google_generative_ai",
        display_name: "Google Generative AI",
        default_base_url: Some("https://generativelanguage.googleapis.com/v1beta"),
        capabilities: CAP_LLM,
        is_local: false,
        is_disabled: false,
    },
    ProviderMeta {
        provider: ConnectProvider::AzureOpenai,
        id: "azure_openai",
        display_name: "Azure OpenAI",
        default_base_url: None,
        capabilities: CAP_LLM,
        is_local: false,
        is_disabled: false,
    },
    ProviderMeta {
        provider: ConnectProvider::AzureAi,
        id: "azure_ai",
        display_name: "Azure AI",
        default_base_url: None,
        capabilities: CAP_LLM,
        is_local: false,
        is_disabled: false,
    },
    // Local
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    ProviderMeta {
        provider: ConnectProvider::Cactus,
        id: "cactus",
        display_name: "Cactus",
        default_base_url: None,
        capabilities: CAP_STT,
        is_local: true,
        is_disabled: false,
    },
    ProviderMeta {
        provider: ConnectProvider::Ollama,
        id: "ollama",
        display_name: "Ollama",
        default_base_url: Some("http://127.0.0.1:11434/v1"),
        capabilities: CAP_LLM,
        is_local: true,
        is_disabled: false,
    },
    ProviderMeta {
        provider: ConnectProvider::Lmstudio,
        id: "lmstudio",
        display_name: "LM Studio",
        default_base_url: Some("http://127.0.0.1:1234/v1"),
        capabilities: CAP_LLM,
        is_local: true,
        is_disabled: false,
    },
    // Custom
    ProviderMeta {
        provider: ConnectProvider::Custom,
        id: "custom",
        display_name: "Custom",
        default_base_url: None,
        capabilities: CAP_STT_LLM,
        is_local: false,
        is_disabled: false,
    },
    // Calendar
    #[cfg(target_os = "macos")]
    ProviderMeta {
        provider: ConnectProvider::AppleCalendar,
        id: "apple_calendar",
        display_name: "Apple Calendar",
        default_base_url: None,
        capabilities: CAP_CAL,
        is_local: true,
        is_disabled: false,
    },
    ProviderMeta {
        provider: ConnectProvider::GoogleCalendar,
        id: "google_calendar",
        display_name: "Google Calendar",
        default_base_url: None,
        capabilities: CAP_CAL,
        is_local: false,
        is_disabled: true,
    },
    ProviderMeta {
        provider: ConnectProvider::OutlookCalendar,
        id: "outlook_calendar",
        display_name: "Outlook Calendar",
        default_base_url: None,
        capabilities: CAP_CAL,
        is_local: false,
        is_disabled: true,
    },
];

impl std::fmt::Display for ConnectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stt => write!(f, "stt"),
            Self::Llm => write!(f, "llm"),
            Self::Cal => write!(f, "cal"),
        }
    }
}

impl std::fmt::Display for ConnectProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id())
    }
}

impl ConnectProvider {
    pub(crate) fn meta(&self) -> &'static ProviderMeta {
        PROVIDERS
            .iter()
            .find(|m| m.provider == *self)
            .expect("all providers must have metadata")
    }

    pub(crate) fn id(&self) -> &'static str {
        self.meta().id
    }

    pub(crate) fn display_name(&self) -> &'static str {
        self.meta().display_name
    }

    pub(crate) fn capabilities(&self) -> Vec<ConnectionType> {
        self.meta().capabilities.to_vec()
    }

    pub(crate) fn is_disabled(&self) -> bool {
        self.meta().is_disabled
    }

    pub(crate) fn is_local(&self) -> bool {
        self.meta().is_local
    }

    pub(crate) fn default_base_url(&self) -> Option<&'static str> {
        self.meta().default_base_url
    }

    pub(crate) fn valid_for(&self, ct: ConnectionType) -> bool {
        self.meta().capabilities.contains(&ct)
    }

    pub(crate) fn is_calendar_provider(&self) -> bool {
        self.meta().capabilities.contains(&ConnectionType::Cal)
    }
}
