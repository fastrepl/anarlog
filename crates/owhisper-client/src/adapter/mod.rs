pub mod parsing;
mod url_builder;

mod anarlog;
mod aquavoice;
mod argmax;
pub(crate) mod assemblyai;
mod aws_transcribe;
mod azure_speech;
pub(crate) mod cartesia;
mod cohere;
mod dashscope;
pub mod deepgram;
mod deepgram_compat;
pub(crate) mod elevenlabs;
mod fireworks;
mod gladia;
mod google_cloud;
mod groq;
pub mod http;
mod language;
mod mistral;
mod openai;
mod openai_compatible_batch;
mod openrouter;
mod owhisper;
mod pyannote;
mod revai;
mod smallestai;
pub(crate) mod soniox;
mod speechmatics;
mod together;
mod whispercpp;
mod xai;

pub use anarlog::*;
pub use aquavoice::*;
pub use argmax::*;
pub use assemblyai::*;
pub use aws_transcribe::*;
pub use azure_speech::*;
pub use cartesia::*;
pub use cohere::*;
pub use dashscope::*;
pub use deepgram::*;
pub use elevenlabs::*;
pub use fireworks::*;
pub use gladia::*;
pub use google_cloud::*;
pub use groq::*;
pub use language::{LanguageQuality, LanguageSupport};
pub use mistral::*;
pub use openai::*;
pub use openrouter::*;
pub use pyannote::*;
pub use revai::*;
pub use smallestai::*;
pub use soniox::*;
pub use speechmatics::*;
pub use together::*;
pub use whispercpp::*;
pub use xai::*;

use std::collections::{BTreeSet, HashSet};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::str::FromStr;

use anlg_ws_client::client::Message;
use owhisper_interface::ListenParams;
use owhisper_interface::batch::Response as BatchResponse;
use owhisper_interface::batch_stream::BatchStreamEvent;
use owhisper_interface::stream::StreamResponse;

use crate::error::Error;

pub use reqwest_middleware::ClientWithMiddleware;

pub type BatchFuture<'a> = Pin<Box<dyn Future<Output = Result<BatchResponse, Error>> + Send + 'a>>;

pub(crate) const MIXED_CAPTURE_CHANNEL: i32 = 2;

pub type StreamingBatchEvent = BatchStreamEvent;

pub type StreamingBatchStream =
    Pin<Box<dyn futures_util::Stream<Item = Result<BatchStreamEvent, Error>> + Send>>;

fn canonical_menu_language_code(code: &str) -> Option<String> {
    let language = code
        .split(['-', '_'])
        .next()
        .filter(|part| !part.is_empty())?
        .to_lowercase();
    let language = match language.as_str() {
        "jw" => "jv",
        _ => language.as_str(),
    };

    anlg_language::ISO639::from_str(language)
        .ok()
        .map(|code| code.code().to_string())
}

fn simple_documented_language_codes(codes: impl IntoIterator<Item = &'static str>) -> Vec<String> {
    let set: BTreeSet<String> = codes
        .into_iter()
        .filter_map(canonical_menu_language_code)
        .collect();

    set.into_iter().collect()
}

pub fn documented_language_codes_live() -> Vec<String> {
    let mut codes = Vec::new();

    codes.extend(deepgram::documented_language_codes());
    codes.extend(cartesia::documented_language_codes_live());
    codes.extend(soniox::documented_language_codes().iter().copied());
    codes.extend(gladia::documented_language_codes().iter().copied());
    codes.extend(assemblyai::documented_language_codes_live().iter().copied());
    codes.extend(elevenlabs::documented_language_codes());
    codes.extend(argmax::PARAKEET_V3_LANGS.iter().copied());

    simple_documented_language_codes(codes)
}

pub fn documented_language_codes_batch() -> Vec<String> {
    let mut codes = Vec::new();

    codes.extend(cartesia::documented_language_codes_batch());
    codes.extend(deepgram::documented_language_codes());
    codes.extend(soniox::documented_language_codes().iter().copied());
    codes.extend(gladia::documented_language_codes().iter().copied());
    codes.extend(
        assemblyai::documented_language_codes_batch()
            .iter()
            .copied(),
    );
    codes.extend(elevenlabs::documented_language_codes());
    codes.extend(argmax::PARAKEET_V3_LANGS.iter().copied());
    codes.extend(pyannote::documented_language_codes());
    codes.extend(cohere::documented_language_codes().iter().copied());

    simple_documented_language_codes(codes)
}

pub trait RealtimeSttAdapter: Clone + Default + Send + Sync + 'static {
    fn fork_session(&self) -> Self {
        self.clone()
    }

    fn provider_name(&self) -> &'static str;

    fn is_supported_languages(
        &self,
        languages: &[anlg_language::Language],
        model: Option<&str>,
    ) -> bool;

    fn supports_native_multichannel(&self) -> bool;

    fn build_ws_url(&self, api_base: &str, params: &ListenParams, channels: u8) -> url::Url;

    fn build_ws_url_with_api_key(
        &self,
        api_base: &str,
        params: &ListenParams,
        channels: u8,
        _api_key: Option<&str>,
    ) -> impl std::future::Future<Output = Option<url::Url>> + Send {
        let url = self.build_ws_url(api_base, params, channels);
        async move { Some(url) }
    }

    fn build_auth_header(&self, api_key: Option<&str>) -> Option<(&'static str, String)>;

    fn keep_alive_message(&self) -> Option<Message>;

    fn finalize_message(&self) -> Message;

    fn audio_to_message(&self, audio: bytes::Bytes) -> Message {
        Message::Binary(audio)
    }

    fn initial_message(
        &self,
        _api_key: Option<&str>,
        _params: &ListenParams,
        _channels: u8,
    ) -> Option<Message> {
        None
    }

    fn parse_response(&self, raw: &str) -> Vec<StreamResponse>;
}

pub trait BatchSttAdapter: Clone + Default + Send + Sync + 'static {
    fn provider_name(&self) -> &'static str {
        "unknown"
    }

    fn is_supported_languages(
        &self,
        languages: &[anlg_language::Language],
        model: Option<&str>,
    ) -> bool;

    fn transcribe_file<'a, P: AsRef<Path> + Send + 'a>(
        &'a self,
        client: &'a ClientWithMiddleware,
        api_base: &'a str,
        api_key: &'a str,
        params: &'a ListenParams,
        file_path: P,
    ) -> BatchFuture<'a>;
}

pub enum CallbackResult {
    Done(serde_json::Value),
    ProviderError(String),
}

pub type CallbackSubmitFuture<'a> =
    Pin<Box<dyn Future<Output = Result<String, Error>> + Send + 'a>>;
pub type CallbackProcessFuture<'a> =
    Pin<Box<dyn Future<Output = Result<CallbackResult, Error>> + Send + 'a>>;

pub trait CallbackSttAdapter: Clone + Default + Send + Sync + 'static {
    fn submit_callback<'a>(
        &'a self,
        client: &'a reqwest::Client,
        api_key: &'a str,
        audio_url: &'a str,
        callback_url: &'a str,
    ) -> CallbackSubmitFuture<'a>;

    fn process_callback<'a>(
        &'a self,
        client: &'a reqwest::Client,
        api_key: &'a str,
        payload: serde_json::Value,
    ) -> CallbackProcessFuture<'a>;
}

pub(crate) fn build_url_with_scheme(
    parsed: &url::Url,
    default_host: &str,
    path: &str,
    use_ws: bool,
) -> url::Url {
    let host = parsed.host_str().unwrap_or(default_host);
    let is_local = is_local_host(host);
    let scheme = match (use_ws, is_local) {
        (true, true) => "ws",
        (true, false) => "wss",
        (false, true) => "http",
        (false, false) => "https",
    };
    let host_with_port = match parsed.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_string(),
    };
    format!("{scheme}://{host_with_port}{path}")
        .parse()
        .expect("invalid_url")
}

pub fn set_scheme_from_host(url: &mut url::Url) {
    if let Some(host) = url.host_str() {
        if is_local_host(host) {
            let _ = url.set_scheme("ws");
        } else {
            let _ = url.set_scheme("wss");
        }
    }
}

pub fn is_local_host(host: &str) -> bool {
    host == "127.0.0.1" || host == "localhost" || host == "0.0.0.0" || host == "::1"
}

pub fn extract_query_params(url: &url::Url) -> Vec<(String, String)> {
    url.query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

pub fn append_path_if_missing(url: &mut url::Url, suffix: &str) {
    let path = url.path().to_string();
    if !path.ends_with(suffix) && !path.ends_with(&format!("{}/", suffix)) {
        let mut new_path = path;
        if !new_path.ends_with('/') {
            new_path.push('/');
        }
        new_path.push_str(suffix.trim_start_matches('/'));
        url.set_path(&new_path);
    }
}

pub(crate) fn host_matches(base_url: &str, predicate: impl Fn(&str) -> bool) -> bool {
    url::Url::parse(base_url)
        .ok()
        .and_then(|u| u.host_str().map(&predicate))
        .unwrap_or(false)
}

const ANARLOG_PROXY_DOMAINS: &[&str] = &["hyprnote.com", "char.com", "anarlog.so"];

fn is_anarlog_cloud_host(host: &str) -> bool {
    ANARLOG_PROXY_DOMAINS.iter().any(|domain| {
        host == *domain
            || host
                .strip_suffix(domain)
                .is_some_and(|prefix| prefix.ends_with('.'))
    })
}

fn is_anarlog_cloud(base_url: &str) -> bool {
    host_matches(base_url, is_anarlog_cloud_host)
}

fn is_anarlog_local_proxy(base_url: &str) -> bool {
    url::Url::parse(base_url)
        .ok()
        .map(|u| is_local_host(u.host_str().unwrap_or("")) && u.path().contains("/stt"))
        .unwrap_or(false)
}

pub fn is_anarlog_proxy(base_url: &str) -> bool {
    is_anarlog_cloud(base_url) || is_anarlog_local_proxy(base_url)
}

pub fn normalize_languages(languages: &[anlg_language::Language]) -> Vec<anlg_language::Language> {
    let mut seen = HashSet::new();
    let mut result = Vec::with_capacity(languages.len());

    for lang in languages {
        let iso639 = lang.iso639();
        if seen.insert(iso639) {
            result.push(lang.clone());
        } else if lang.region().is_none()
            && let Some(pos) = result.iter().position(|l| l.iso639() == iso639)
        {
            result[pos] = lang.clone();
        }
    }

    result
}

fn is_local_argmax(base_url: &str) -> bool {
    host_matches(base_url, is_local_host) && !is_anarlog_local_proxy(base_url)
}

pub(crate) fn build_ws_url_from_base_with(
    provider: crate::providers::Provider,
    api_base: &str,
    make_url: impl FnOnce(&url::Url) -> url::Url,
) -> (url::Url, Vec<(String, String)>) {
    let default_url = || -> (url::Url, Vec<(String, String)>) {
        (
            provider
                .default_ws_url()
                .parse()
                .expect("invalid_default_ws_url"),
            Vec::new(),
        )
    };

    if api_base.is_empty() {
        return default_url();
    }

    if let Some(proxy_result) = build_proxy_ws_url(api_base) {
        return proxy_result;
    }

    let parsed: url::Url = match api_base.parse() {
        Ok(u) => u,
        Err(_) => return default_url(),
    };
    let existing_params = extract_query_params(&parsed);
    (make_url(&parsed), existing_params)
}

pub fn build_proxy_ws_url(api_base: &str) -> Option<(url::Url, Vec<(String, String)>)> {
    if api_base.is_empty() {
        return None;
    }

    let parsed: url::Url = api_base.parse().ok()?;
    let host = parsed.host_str()?;

    if !is_anarlog_cloud_host(host) && !is_local_host(host) {
        return None;
    }

    let existing_params = extract_query_params(&parsed);
    let mut url = parsed;
    url.set_query(None);
    append_path_if_missing(&mut url, "listen");
    set_scheme_from_host(&mut url);

    Some((url, existing_params))
}

pub fn append_provider_param(base_url: &str, provider: &str) -> String {
    match url::Url::parse(base_url) {
        Ok(mut url) => {
            let existing: Vec<(String, String)> = url
                .query_pairs()
                .filter(|(k, _)| k != "provider")
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect();

            url.query_pairs_mut().clear().extend_pairs(&existing);
            url.query_pairs_mut().append_pair("provider", provider);
            url.to_string()
        }
        Err(_) => base_url.to_string(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumString)]
pub enum AdapterKind {
    #[strum(serialize = "aquavoice")]
    AquaVoice,
    #[strum(serialize = "cartesia")]
    Cartesia,
    #[strum(serialize = "argmax")]
    Argmax,
    #[strum(serialize = "soniox")]
    Soniox,
    #[strum(serialize = "fireworks")]
    Fireworks,
    #[strum(serialize = "deepgram")]
    Deepgram,
    #[strum(serialize = "assemblyai")]
    AssemblyAI,
    #[strum(serialize = "openai")]
    OpenAI,
    #[strum(serialize = "openrouter")]
    OpenRouter,
    #[strum(serialize = "gladia")]
    Gladia,
    #[strum(serialize = "elevenlabs")]
    ElevenLabs,
    #[strum(serialize = "dashscope")]
    DashScope,
    #[strum(serialize = "mistral")]
    Mistral,
    #[strum(serialize = "pyannote")]
    Pyannote,
    #[strum(serialize = "cohere")]
    Cohere,
    #[strum(serialize = "aws_transcribe")]
    AwsTranscribe,
    #[strum(serialize = "azure_speech")]
    AzureSpeech,
    #[strum(serialize = "google_cloud")]
    GoogleCloud,
    #[strum(serialize = "groq")]
    Groq,
    #[strum(serialize = "revai")]
    RevAi,
    #[strum(serialize = "speechmatics")]
    Speechmatics,
    #[strum(serialize = "together")]
    Together,
    #[strum(serialize = "xai")]
    Xai,
    #[strum(serialize = "anarlog")]
    Anarlog,
}

impl AdapterKind {
    pub fn from_url_and_languages(
        base_url: &str,
        _languages: &[anlg_language::Language],
        _model: Option<&str>,
    ) -> Self {
        use crate::providers::Provider;

        if is_anarlog_proxy(base_url) {
            return Self::Anarlog;
        }

        if is_local_argmax(base_url) {
            return Self::Argmax;
        }

        if host_matches(base_url, |host| {
            host == "openrouter.ai" || host.ends_with(".openrouter.ai")
        }) {
            return Self::OpenRouter;
        }

        Provider::from_url(base_url)
            .map(Self::from)
            .unwrap_or(Self::Deepgram)
    }

    pub fn has_live_mode(&self) -> bool {
        match self {
            Self::AquaVoice
            | Self::Argmax
            | Self::Pyannote
            | Self::Cohere
            | Self::AwsTranscribe
            | Self::AzureSpeech
            | Self::GoogleCloud
            | Self::Groq
            | Self::OpenRouter
            | Self::RevAi
            | Self::Speechmatics
            | Self::Together => false,
            Self::Soniox
            | Self::Cartesia
            | Self::Fireworks
            | Self::Deepgram
            | Self::AssemblyAI
            | Self::OpenAI
            | Self::Gladia
            | Self::ElevenLabs
            | Self::DashScope
            | Self::Mistral
            | Self::Xai
            | Self::Anarlog => true,
        }
    }

    pub fn language_support_live(
        &self,
        languages: &[anlg_language::Language],
        model: Option<&str>,
    ) -> LanguageSupport {
        match self {
            Self::AquaVoice => LanguageSupport::NotSupported,
            Self::Cartesia => CartesiaAdapter::language_support_live(languages),
            Self::Deepgram => {
                let model = model.and_then(|m| m.parse::<deepgram::DeepgramModel>().ok());
                DeepgramAdapter::language_support_live(languages, model)
            }
            Self::Soniox => SonioxAdapter::language_support_live(languages),
            Self::AssemblyAI => AssemblyAIAdapter::language_support_live(languages),
            Self::Gladia => GladiaAdapter::language_support_live(languages, model),
            Self::OpenAI => OpenAIAdapter::language_support_live(languages),
            Self::OpenRouter => LanguageSupport::NotSupported,
            Self::Fireworks => FireworksAdapter::language_support_live(languages),
            Self::ElevenLabs => ElevenLabsAdapter::language_support_live(languages),
            Self::DashScope => DashScopeAdapter::language_support_live(languages),
            Self::Argmax => ArgmaxAdapter::language_support_live(languages, model),
            Self::Mistral => MistralAdapter::language_support_live(languages),
            Self::Pyannote => LanguageSupport::NotSupported,
            Self::Cohere => LanguageSupport::NotSupported,
            Self::AwsTranscribe
            | Self::AzureSpeech
            | Self::GoogleCloud
            | Self::Groq
            | Self::RevAi
            | Self::Speechmatics
            | Self::Together => LanguageSupport::NotSupported,
            Self::Xai => XaiAdapter::language_support_live(languages),
            Self::Anarlog => AnarlogAdapter::language_support_live(languages, model),
        }
    }

    pub fn language_support_batch(
        &self,
        languages: &[anlg_language::Language],
        model: Option<&str>,
    ) -> LanguageSupport {
        match self {
            Self::AquaVoice => AquaVoiceAdapter::language_support_batch(languages),
            Self::Cartesia => CartesiaAdapter::language_support_batch(languages),
            Self::Deepgram => {
                let model = model.and_then(|m| m.parse::<deepgram::DeepgramModel>().ok());
                DeepgramAdapter::language_support_batch(languages, model)
            }
            Self::Soniox => SonioxAdapter::language_support_batch(languages),
            Self::AssemblyAI => AssemblyAIAdapter::language_support_batch(languages),
            Self::Gladia => GladiaAdapter::language_support_batch(languages, model),
            Self::OpenAI => OpenAIAdapter::language_support_batch(languages),
            Self::OpenRouter => OpenRouterAdapter::language_support_batch(languages),
            Self::Fireworks => FireworksAdapter::language_support_batch(languages),
            Self::ElevenLabs => ElevenLabsAdapter::language_support_batch(languages),
            Self::DashScope => DashScopeAdapter::language_support_batch(languages),
            Self::Argmax => ArgmaxAdapter::language_support_batch(languages, model),
            Self::Mistral => MistralAdapter::language_support_batch(languages),
            Self::Pyannote => PyannoteAdapter::language_support_batch(languages, model),
            Self::Cohere => CohereAdapter::language_support_batch(languages),
            Self::AwsTranscribe => AwsTranscribeAdapter::language_support_batch(languages),
            Self::AzureSpeech => AzureSpeechAdapter::language_support_batch(languages),
            Self::GoogleCloud => GoogleCloudAdapter::language_support_batch(languages),
            Self::Groq => GroqAdapter::language_support_batch(languages),
            Self::RevAi => RevAiAdapter::language_support_batch(languages),
            Self::Speechmatics => SpeechmaticsAdapter::language_support_batch(languages),
            Self::Together => TogetherAdapter::language_support_batch(languages),
            Self::Xai => XaiAdapter::language_support_batch(languages),
            Self::Anarlog => AnarlogAdapter::language_support_batch(languages, model),
        }
    }

    pub fn is_supported_languages_live(
        &self,
        languages: &[anlg_language::Language],
        model: Option<&str>,
    ) -> bool {
        self.language_support_live(languages, model).is_supported()
    }

    pub fn is_supported_languages_batch(
        &self,
        languages: &[anlg_language::Language],
        model: Option<&str>,
    ) -> bool {
        self.language_support_batch(languages, model).is_supported()
    }

    pub fn recommended_model_live(
        &self,
        languages: &[anlg_language::Language],
    ) -> Option<&'static str> {
        match self {
            Self::Deepgram => DeepgramAdapter::recommended_model_live(languages),
            _ => None,
        }
    }

    pub fn recommended_model_batch(
        &self,
        languages: &[anlg_language::Language],
    ) -> Option<&'static str> {
        match self {
            Self::Deepgram => DeepgramAdapter::recommended_model_live(languages),
            _ => None,
        }
    }
}

impl From<crate::providers::Provider> for AdapterKind {
    fn from(p: crate::providers::Provider) -> Self {
        use crate::providers::Provider;
        match p {
            Provider::AquaVoice => Self::AquaVoice,
            Provider::Cartesia => Self::Cartesia,
            Provider::Deepgram => Self::Deepgram,
            Provider::AssemblyAI => Self::AssemblyAI,
            Provider::Soniox => Self::Soniox,
            Provider::Fireworks => Self::Fireworks,
            Provider::OpenAI => Self::OpenAI,
            Provider::Gladia => Self::Gladia,
            Provider::ElevenLabs => Self::ElevenLabs,
            Provider::DashScope => Self::DashScope,
            Provider::Mistral => Self::Mistral,
            Provider::Pyannote => Self::Pyannote,
            Provider::Cohere => Self::Cohere,
            Provider::AwsTranscribe => Self::AwsTranscribe,
            Provider::AzureSpeech => Self::AzureSpeech,
            Provider::GoogleCloud => Self::GoogleCloud,
            Provider::Groq => Self::Groq,
            Provider::RevAi => Self::RevAi,
            Provider::Speechmatics => Self::Speechmatics,
            Provider::Together => Self::Together,
            Provider::Xai => Self::Xai,
        }
    }
}

#[cfg(test)]
mod tests;
