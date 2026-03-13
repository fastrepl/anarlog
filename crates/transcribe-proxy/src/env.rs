use std::collections::HashMap;

use owhisper_client::Provider;
use serde::Deserialize;

#[derive(Default, Deserialize)]
pub struct SttApiKeysEnv {
    #[serde(default)]
    pub deepgram_api_key: Option<String>,
    #[serde(default)]
    pub assemblyai_api_key: Option<String>,
    #[serde(default)]
    pub soniox_api_key: Option<String>,
    #[serde(default)]
    pub fireworks_api_key: Option<String>,
    #[serde(default)]
    pub openai_api_key: Option<String>,
    #[serde(default)]
    pub gladia_api_key: Option<String>,
    #[serde(default)]
    pub elevenlabs_api_key: Option<String>,
    #[serde(default)]
    pub dashscope_api_key: Option<String>,
    #[serde(default)]
    pub mistral_api_key: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct CallbackEnv {
    pub api_base_url: String,
    #[serde(default)]
    pub callback_secret: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct Env {
    #[serde(flatten)]
    pub stt: SttApiKeysEnv,
    #[serde(flatten)]
    pub callback: CallbackEnv,
}

pub struct ApiKeys(pub HashMap<Provider, String>);

impl ApiKeys {
    pub fn configured_providers(&self) -> Vec<Provider> {
        self.0.keys().copied().collect()
    }

    pub fn log_configured_providers(&self) {
        let providers = self.configured_providers();
        if providers.is_empty() {
            tracing::error!("no_stt_providers_configured");
        } else {
            let names: Vec<_> = providers.iter().map(|p| format!("{:?}", p)).collect();
            tracing::info!(providers = ?names, "stt_providers_configured");
        }
    }
}

impl From<&SttApiKeysEnv> for ApiKeys {
    fn from(env: &SttApiKeysEnv) -> Self {
        let mut map = HashMap::new();
        if let Some(key) = env.deepgram_api_key.as_ref().filter(|s| !s.is_empty()) {
            map.insert(Provider::Deepgram, key.clone());
        }
        if let Some(key) = env.assemblyai_api_key.as_ref().filter(|s| !s.is_empty()) {
            map.insert(Provider::AssemblyAI, key.clone());
        }
        if let Some(key) = env.soniox_api_key.as_ref().filter(|s| !s.is_empty()) {
            map.insert(Provider::Soniox, key.clone());
        }
        if let Some(key) = env.fireworks_api_key.as_ref().filter(|s| !s.is_empty()) {
            map.insert(Provider::Fireworks, key.clone());
        }
        if let Some(key) = env.openai_api_key.as_ref().filter(|s| !s.is_empty()) {
            map.insert(Provider::OpenAI, key.clone());
        }
        if let Some(key) = env.gladia_api_key.as_ref().filter(|s| !s.is_empty()) {
            map.insert(Provider::Gladia, key.clone());
        }
        if let Some(key) = env.elevenlabs_api_key.as_ref().filter(|s| !s.is_empty()) {
            map.insert(Provider::ElevenLabs, key.clone());
        }
        if let Some(key) = env.dashscope_api_key.as_ref().filter(|s| !s.is_empty()) {
            map.insert(Provider::DashScope, key.clone());
        }
        if let Some(key) = env.mistral_api_key.as_ref().filter(|s| !s.is_empty()) {
            map.insert(Provider::Mistral, key.clone());
        }
        Self(map)
    }
}
