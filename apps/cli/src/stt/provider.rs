use clap::ValueEnum;

use hypr_listener2_core::BatchProvider;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum SttProvider {
    Deepgram,
    Soniox,
    Assemblyai,
    Fireworks,
    Openai,
    Gladia,
    Elevenlabs,
    Mistral,
    Pyannote,
    #[cfg(target_os = "macos")]
    Whispercpp,
    #[cfg(all(target_os = "macos", any(target_arch = "arm", target_arch = "aarch64")))]
    Cactus,
}

impl SttProvider {
    fn meta(self) -> ProviderMeta {
        match self {
            Self::Deepgram => ProviderMeta::cloud(
                "deepgram",
                owhisper_client::Provider::Deepgram,
                BatchProvider::Deepgram,
            ),
            Self::Soniox => ProviderMeta::cloud(
                "soniox",
                owhisper_client::Provider::Soniox,
                BatchProvider::Soniox,
            ),
            Self::Assemblyai => ProviderMeta::cloud(
                "assemblyai",
                owhisper_client::Provider::AssemblyAI,
                BatchProvider::AssemblyAI,
            ),
            Self::Fireworks => ProviderMeta::cloud(
                "fireworks",
                owhisper_client::Provider::Fireworks,
                BatchProvider::Fireworks,
            ),
            Self::Openai => ProviderMeta::cloud(
                "openai",
                owhisper_client::Provider::OpenAI,
                BatchProvider::OpenAI,
            ),
            Self::Gladia => ProviderMeta::cloud(
                "gladia",
                owhisper_client::Provider::Gladia,
                BatchProvider::Gladia,
            ),
            Self::Elevenlabs => ProviderMeta::cloud(
                "elevenlabs",
                owhisper_client::Provider::ElevenLabs,
                BatchProvider::ElevenLabs,
            ),
            Self::Mistral => ProviderMeta::cloud(
                "mistral",
                owhisper_client::Provider::Mistral,
                BatchProvider::Mistral,
            ),
            Self::Pyannote => ProviderMeta::cloud(
                "pyannote",
                owhisper_client::Provider::Pyannote,
                BatchProvider::Pyannote,
            ),
            #[cfg(target_os = "macos")]
            Self::Whispercpp => ProviderMeta::local("whispercpp", BatchProvider::WhisperLocal),
            #[cfg(all(target_os = "macos", any(target_arch = "arm", target_arch = "aarch64")))]
            Self::Cactus => ProviderMeta::local("cactus", BatchProvider::Cactus),
        }
    }

    pub fn id(self) -> &'static str {
        self.meta().id
    }

    pub fn from_id(id: &str) -> Option<Self> {
        providers()
            .iter()
            .find(|provider| provider.id() == id)
            .copied()
    }

    pub fn is_local(&self) -> bool {
        self.meta().is_local
    }

    pub(crate) fn cloud_provider(&self) -> Option<owhisper_client::Provider> {
        self.meta().cloud_provider
    }

    pub(crate) fn to_batch_provider(self) -> BatchProvider {
        self.meta().batch_provider
    }
}

struct ProviderMeta {
    id: &'static str,
    cloud_provider: Option<owhisper_client::Provider>,
    batch_provider: BatchProvider,
    is_local: bool,
}

impl ProviderMeta {
    fn cloud(
        id: &'static str,
        cloud_provider: owhisper_client::Provider,
        batch_provider: BatchProvider,
    ) -> Self {
        Self {
            id,
            cloud_provider: Some(cloud_provider),
            batch_provider,
            is_local: false,
        }
    }

    fn local(id: &'static str, batch_provider: BatchProvider) -> Self {
        Self {
            id,
            cloud_provider: None,
            batch_provider,
            is_local: true,
        }
    }
}

fn providers() -> &'static [SttProvider] {
    &[
        SttProvider::Deepgram,
        SttProvider::Soniox,
        SttProvider::Assemblyai,
        SttProvider::Fireworks,
        SttProvider::Openai,
        SttProvider::Gladia,
        SttProvider::Elevenlabs,
        SttProvider::Mistral,
        SttProvider::Pyannote,
        #[cfg(target_os = "macos")]
        SttProvider::Whispercpp,
        #[cfg(all(target_os = "macos", any(target_arch = "arm", target_arch = "aarch64")))]
        SttProvider::Cactus,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_ids_round_trip() {
        for provider in providers() {
            assert_eq!(SttProvider::from_id(provider.id()), Some(*provider));
        }
    }
}
