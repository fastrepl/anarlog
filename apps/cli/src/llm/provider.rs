use clap::ValueEnum;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum LlmProvider {
    Anthropic,
    Openrouter,
}

impl LlmProvider {
    pub fn id(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::Openrouter => "openrouter",
        }
    }

    pub fn default_base_url(self) -> &'static str {
        match self {
            Self::Anthropic => "https://api.anthropic.com",
            Self::Openrouter => "https://openrouter.ai/api/v1",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        match id {
            "anthropic" => Some(Self::Anthropic),
            "openrouter" => Some(Self::Openrouter),
            _ => None,
        }
    }
}
