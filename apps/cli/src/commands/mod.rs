pub mod auth;
pub mod batch;
pub mod chat;
pub mod connect;
#[cfg(debug_assertions)]
pub mod debug;
pub mod desktop;
pub mod listen;
pub mod model;
pub mod status;

use hypr_listener2_core::BatchProvider;

pub use crate::cli::{OutputFormat, Provider};

pub struct SttGlobalArgs {
    pub provider: Provider,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub language: String,
}

impl Provider {
    pub fn is_local(&self) -> bool {
        match self {
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            Provider::Cactus => true,
            _ => false,
        }
    }

    pub fn cloud_provider(&self) -> Option<owhisper_client::Provider> {
        match self {
            Provider::Deepgram => Some(owhisper_client::Provider::Deepgram),
            Provider::Soniox => Some(owhisper_client::Provider::Soniox),
            Provider::Assemblyai => Some(owhisper_client::Provider::AssemblyAI),
            Provider::Fireworks => Some(owhisper_client::Provider::Fireworks),
            Provider::Openai => Some(owhisper_client::Provider::OpenAI),
            Provider::Gladia => Some(owhisper_client::Provider::Gladia),
            Provider::Elevenlabs => Some(owhisper_client::Provider::ElevenLabs),
            Provider::Mistral => Some(owhisper_client::Provider::Mistral),
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            Provider::Cactus => None,
        }
    }
}

impl From<Provider> for BatchProvider {
    fn from(value: Provider) -> Self {
        match value {
            Provider::Deepgram => BatchProvider::Deepgram,
            Provider::Soniox => BatchProvider::Soniox,
            Provider::Assemblyai => BatchProvider::AssemblyAI,
            Provider::Fireworks => BatchProvider::Fireworks,
            Provider::Openai => BatchProvider::OpenAI,
            Provider::Gladia => BatchProvider::Gladia,
            Provider::Elevenlabs => BatchProvider::ElevenLabs,
            Provider::Mistral => BatchProvider::Mistral,
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            Provider::Cactus => BatchProvider::Cactus,
        }
    }
}
