use clap::ValueEnum;

use hypr_listener2_core::BatchProvider;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum SttProvider {
    Deepgram,
    Soniox,
    Assemblyai,
    Fireworks,
    Openai,
    Gladia,
    Elevenlabs,
    Mistral,
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    Cactus,
}

impl SttProvider {
    pub fn id(self) -> &'static str {
        match self {
            Self::Deepgram => "deepgram",
            Self::Soniox => "soniox",
            Self::Assemblyai => "assemblyai",
            Self::Fireworks => "fireworks",
            Self::Openai => "openai",
            Self::Gladia => "gladia",
            Self::Elevenlabs => "elevenlabs",
            Self::Mistral => "mistral",
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            Self::Cactus => "cactus",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "deepgram" => Some(Self::Deepgram),
            "soniox" => Some(Self::Soniox),
            "assemblyai" => Some(Self::Assemblyai),
            "fireworks" => Some(Self::Fireworks),
            "openai" => Some(Self::Openai),
            "gladia" => Some(Self::Gladia),
            "elevenlabs" => Some(Self::Elevenlabs),
            "mistral" => Some(Self::Mistral),
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            "cactus" => Some(Self::Cactus),
            _ => None,
        }
    }

    pub fn is_local(&self) -> bool {
        match self {
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            SttProvider::Cactus => true,
            _ => false,
        }
    }

    pub(crate) fn cloud_provider(&self) -> Option<owhisper_client::Provider> {
        match self {
            SttProvider::Deepgram => Some(owhisper_client::Provider::Deepgram),
            SttProvider::Soniox => Some(owhisper_client::Provider::Soniox),
            SttProvider::Assemblyai => Some(owhisper_client::Provider::AssemblyAI),
            SttProvider::Fireworks => Some(owhisper_client::Provider::Fireworks),
            SttProvider::Openai => Some(owhisper_client::Provider::OpenAI),
            SttProvider::Gladia => Some(owhisper_client::Provider::Gladia),
            SttProvider::Elevenlabs => Some(owhisper_client::Provider::ElevenLabs),
            SttProvider::Mistral => Some(owhisper_client::Provider::Mistral),
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            SttProvider::Cactus => None,
        }
    }

    pub(crate) fn to_batch_provider(&self) -> BatchProvider {
        match self {
            SttProvider::Deepgram => BatchProvider::Deepgram,
            SttProvider::Soniox => BatchProvider::Soniox,
            SttProvider::Assemblyai => BatchProvider::AssemblyAI,
            SttProvider::Fireworks => BatchProvider::Fireworks,
            SttProvider::Openai => BatchProvider::OpenAI,
            SttProvider::Gladia => BatchProvider::Gladia,
            SttProvider::Elevenlabs => BatchProvider::ElevenLabs,
            SttProvider::Mistral => BatchProvider::Mistral,
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            SttProvider::Cactus => BatchProvider::Cactus,
        }
    }
}
