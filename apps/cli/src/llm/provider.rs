use clap::ValueEnum;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum LlmProvider {
    Anthropic,
    Openai,
    Openrouter,
}

impl LlmProvider {
    pub fn id(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::Openai => "openai",
            Self::Openrouter => "openrouter",
        }
    }

    pub fn default_base_url(self) -> &'static str {
        match self {
            Self::Anthropic => "https://api.anthropic.com",
            Self::Openai => "https://api.openai.com/v1",
            Self::Openrouter => "https://openrouter.ai/api/v1",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        match id {
            "anthropic" => Some(Self::Anthropic),
            "openai" => Some(Self::Openai),
            "openrouter" => Some(Self::Openrouter),
            _ => None,
        }
    }
}
