pub mod parsing;
mod url_builder;

mod argmax;
pub(crate) mod assemblyai;
mod cactus;
mod dashscope;
pub mod deepgram;
mod deepgram_compat;
pub(crate) mod elevenlabs;
mod fireworks;
mod gladia;
pub mod http;
mod hyprnote;
mod language;
mod mistral;
mod openai;
mod owhisper;
mod pyannote;
pub(crate) mod soniox;
mod whispercpp;

pub use argmax::*;
pub use assemblyai::*;
pub use cactus::*;
pub use dashscope::*;
pub use deepgram::*;
pub use elevenlabs::*;
pub use fireworks::*;
pub use gladia::*;
pub use hyprnote::*;
pub use language::{LanguageQuality, LanguageSupport};
pub use mistral::*;
pub use openai::*;
pub use pyannote::*;
pub use soniox::*;
pub use whispercpp::*;

use std::collections::{BTreeSet, HashSet};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use hypr_ws_client::client::Message;
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

pub fn documented_language_codes_live() -> Vec<String> {
    let mut set: BTreeSet<&'static str> = BTreeSet::new();

    set.extend(deepgram::documented_language_codes());
    set.extend(soniox::documented_language_codes().iter().copied());
    set.extend(gladia::documented_language_codes().iter().copied());
    set.extend(assemblyai::documented_language_codes_live().iter().copied());
    set.extend(elevenlabs::documented_language_codes());
    set.extend(argmax::PARAKEET_V3_LANGS.iter().copied());

    set.into_iter().map(str::to_string).collect()
}

pub fn documented_language_codes_batch() -> Vec<String> {
    let mut set: BTreeSet<&'static str> = BTreeSet::new();

    set.extend(deepgram::documented_language_codes());
    set.extend(soniox::documented_language_codes().iter().copied());
    set.extend(gladia::documented_language_codes().iter().copied());
    set.extend(
        assemblyai::documented_language_codes_batch()
            .iter()
            .copied(),
    );
    set.extend(elevenlabs::documented_language_codes());
    set.extend(argmax::PARAKEET_V3_LANGS.iter().copied());
    set.extend(pyannote::documented_language_codes());

    set.into_iter().map(str::to_string).collect()
}

pub trait RealtimeSttAdapter: Clone + Default + Send + Sync + 'static {
    fn provider_name(&self) -> &'static str;

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
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
        languages: &[hypr_language::Language],
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

fn is_hyprnote_cloud(base_url: &str) -> bool {
    host_matches(base_url, |h| {
        h.contains("hyprnote.com") || h.contains("char.com")
    })
}

fn is_hyprnote_local_proxy(base_url: &str) -> bool {
    url::Url::parse(base_url)
        .ok()
        .map(|u| is_local_host(u.host_str().unwrap_or("")) && u.path().contains("/stt"))
        .unwrap_or(false)
}

pub fn is_hyprnote_proxy(base_url: &str) -> bool {
    is_hyprnote_cloud(base_url) || is_hyprnote_local_proxy(base_url)
}

pub fn normalize_languages(languages: &[hypr_language::Language]) -> Vec<hypr_language::Language> {
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
    host_matches(base_url, is_local_host) && !is_hyprnote_local_proxy(base_url)
}

fn is_cactus_model(model: &str) -> bool {
    model.parse::<hypr_cactus_model::CactusSttModel>().is_ok()
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

    if !host.contains("hyprnote.com") && !is_local_host(host) {
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
    #[strum(serialize = "hyprnote")]
    Hyprnote,
    #[strum(serialize = "cactus")]
    Cactus,
}

impl AdapterKind {
    pub fn from_url_and_languages(
        base_url: &str,
        _languages: &[hypr_language::Language],
        _model: Option<&str>,
    ) -> Self {
        use crate::providers::Provider;

        if is_hyprnote_proxy(base_url) {
            return Self::Hyprnote;
        }

        if is_local_argmax(base_url) {
            if let Some(model) = _model
                && is_cactus_model(model)
            {
                return Self::Cactus;
            }
            return Self::Argmax;
        }

        Provider::from_url(base_url)
            .map(Self::from)
            .unwrap_or(Self::Deepgram)
    }

    pub fn language_support_live(
        &self,
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> LanguageSupport {
        match self {
            Self::Deepgram => {
                let model = model.and_then(|m| m.parse::<deepgram::DeepgramModel>().ok());
                DeepgramAdapter::language_support_live(languages, model)
            }
            Self::Soniox => SonioxAdapter::language_support_live(languages),
            Self::AssemblyAI => AssemblyAIAdapter::language_support_live(languages),
            Self::Gladia => GladiaAdapter::language_support_live(languages),
            Self::OpenAI => LanguageSupport::NotSupported,
            Self::Fireworks => FireworksAdapter::language_support_live(languages),
            Self::ElevenLabs => ElevenLabsAdapter::language_support_live(languages),
            Self::DashScope => DashScopeAdapter::language_support_live(languages),
            Self::Argmax => ArgmaxAdapter::language_support_live(languages, model),
            Self::Mistral => MistralAdapter::language_support_live(languages),
            Self::Pyannote => LanguageSupport::NotSupported,
            Self::Hyprnote | Self::Cactus => LanguageSupport::Supported {
                quality: LanguageQuality::NoData,
            },
        }
    }

    pub fn language_support_batch(
        &self,
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> LanguageSupport {
        match self {
            Self::Deepgram => {
                let model = model.and_then(|m| m.parse::<deepgram::DeepgramModel>().ok());
                DeepgramAdapter::language_support_batch(languages, model)
            }
            Self::Soniox => SonioxAdapter::language_support_batch(languages),
            Self::AssemblyAI => AssemblyAIAdapter::language_support_batch(languages),
            Self::Gladia => GladiaAdapter::language_support_batch(languages),
            Self::OpenAI => OpenAIAdapter::language_support_batch(languages),
            Self::Fireworks => FireworksAdapter::language_support_batch(languages),
            Self::ElevenLabs => ElevenLabsAdapter::language_support_batch(languages),
            Self::DashScope => DashScopeAdapter::language_support_batch(languages),
            Self::Argmax => ArgmaxAdapter::language_support_batch(languages, model),
            Self::Mistral => MistralAdapter::language_support_batch(languages),
            Self::Pyannote => PyannoteAdapter::language_support_batch(languages, model),
            Self::Hyprnote | Self::Cactus => LanguageSupport::Supported {
                quality: LanguageQuality::NoData,
            },
        }
    }

    pub fn is_supported_languages_live(
        &self,
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> bool {
        self.language_support_live(languages, model).is_supported()
    }

    pub fn is_supported_languages_batch(
        &self,
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> bool {
        self.language_support_batch(languages, model).is_supported()
    }

    pub fn recommended_model_live(
        &self,
        languages: &[hypr_language::Language],
    ) -> Option<&'static str> {
        match self {
            Self::Deepgram => DeepgramAdapter::recommended_model_live(languages),
            _ => None,
        }
    }

    pub fn recommended_model_batch(
        &self,
        languages: &[hypr_language::Language],
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_languages_deduplicates_same_base() {
        use hypr_language::{ISO639, Language};

        let en: Language = ISO639::En.into();
        let en_gb = Language::with_region(ISO639::En, "GB");
        let es: Language = ISO639::Es.into();

        let result = normalize_languages(&[en.clone(), en_gb.clone(), es.clone()]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].iso639(), ISO639::En);
        assert_eq!(result[0].region(), None);
        assert_eq!(result[1].iso639(), ISO639::Es);
    }

    #[test]
    fn test_normalize_languages_prefers_base_over_regional() {
        use hypr_language::{ISO639, Language};

        let en_gb = Language::with_region(ISO639::En, "GB");
        let en: Language = ISO639::En.into();

        let result = normalize_languages(&[en_gb.clone(), en.clone()]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].iso639(), ISO639::En);
        assert_eq!(result[0].region(), None);
    }

    #[test]
    fn test_normalize_languages_keeps_regional_if_no_base() {
        use hypr_language::{ISO639, Language};

        let en_gb = Language::with_region(ISO639::En, "GB");
        let es: Language = ISO639::Es.into();

        let result = normalize_languages(&[en_gb.clone(), es.clone()]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].iso639(), ISO639::En);
        assert_eq!(result[0].region(), Some("GB"));
        assert_eq!(result[1].iso639(), ISO639::Es);
    }

    #[test]
    fn test_normalize_languages_multiple_variants() {
        use hypr_language::{ISO639, Language};

        let en_us = Language::with_region(ISO639::En, "US");
        let en_gb = Language::with_region(ISO639::En, "GB");
        let en: Language = ISO639::En.into();

        let result = normalize_languages(&[en_us.clone(), en_gb.clone(), en.clone()]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].iso639(), ISO639::En);
        assert_eq!(result[0].region(), None);
    }

    #[test]
    fn test_is_hyprnote_proxy() {
        assert!(is_hyprnote_proxy("https://api.hyprnote.com/stt"));
        assert!(is_hyprnote_proxy("https://api.hyprnote.com"));
        assert!(is_hyprnote_proxy("https://api.char.com/stt"));
        assert!(is_hyprnote_proxy("https://api.char.com"));
        assert!(is_hyprnote_proxy("http://localhost:3001/stt"));
        assert!(is_hyprnote_proxy("http://127.0.0.1:3001/stt"));

        assert!(!is_hyprnote_proxy("https://api.deepgram.com"));
        assert!(!is_hyprnote_proxy("http://localhost:50060/v1"));
    }

    #[test]
    fn test_is_local_argmax() {
        assert!(is_local_argmax("http://localhost:50060/v1"));
        assert!(is_local_argmax("http://127.0.0.1:50060/v1"));

        assert!(!is_local_argmax("https://api.hyprnote.com/stt"));
        assert!(!is_local_argmax("http://localhost:3001/stt"));
        assert!(!is_local_argmax("https://api.deepgram.com"));
    }

    #[test]
    fn test_adapter_kind_from_url_and_languages() {
        use hypr_language::ISO639::*;

        let cases: &[(&str, &[hypr_language::ISO639], Option<&str>, AdapterKind)] = &[
            // HyprnoteCloud - always routes to Hyprnote adapter (proxy owns provider selection)
            (
                "https://api.hyprnote.com/stt",
                &[En],
                None,
                AdapterKind::Hyprnote,
            ),
            (
                "https://api.hyprnote.com/stt",
                &[En],
                Some("cloud"),
                AdapterKind::Hyprnote,
            ),
            (
                "https://api.hyprnote.com/stt",
                &[Zh],
                None,
                AdapterKind::Hyprnote,
            ),
            (
                "https://api.hyprnote.com/stt",
                &[Ja],
                None,
                AdapterKind::Hyprnote,
            ),
            (
                "https://api.hyprnote.com/stt",
                &[Ar],
                None,
                AdapterKind::Hyprnote,
            ),
            (
                "https://api.hyprnote.com/stt",
                &[De],
                None,
                AdapterKind::Hyprnote,
            ),
            // HyprnoteCloud - multi-language
            (
                "https://api.hyprnote.com/stt",
                &[En, Es],
                None,
                AdapterKind::Hyprnote,
            ),
            (
                "https://api.hyprnote.com/stt",
                &[En, Ko],
                None,
                AdapterKind::Hyprnote,
            ),
            (
                "https://api.hyprnote.com/stt",
                &[Ko, En],
                None,
                AdapterKind::Hyprnote,
            ),
            (
                "https://api.hyprnote.com/stt",
                &[En, De],
                None,
                AdapterKind::Hyprnote,
            ),
            // localhost proxy
            (
                "http://localhost:3001/stt",
                &[En],
                None,
                AdapterKind::Hyprnote,
            ),
            (
                "http://localhost:3001/stt",
                &[Ar],
                None,
                AdapterKind::Hyprnote,
            ),
            // localhost argmax
            (
                "http://localhost:50060/v1",
                &[En],
                None,
                AdapterKind::Argmax,
            ),
            // localhost cactus
            (
                "http://localhost:50060/v1",
                &[En],
                Some("cactus-parakeet-tdt-0.6b-v3-int8"),
                AdapterKind::Cactus,
            ),
        ];

        for (url, langs, model, expected) in cases {
            let langs: Vec<hypr_language::Language> = langs.iter().map(|l| (*l).into()).collect();
            assert_eq!(
                AdapterKind::from_url_and_languages(url, &langs, *model),
                *expected,
                "url={url}, langs={langs:?}, model={model:?}"
            );
        }
    }

    #[test]
    fn test_build_proxy_ws_url() {
        let cases: &[(&str, Option<(&str, Vec<(&str, &str)>)>)] = &[
            ("", None),
            ("https://api.deepgram.com", None),
            ("https://api.soniox.com", None),
            ("https://api.fireworks.ai", None),
            ("https://api.assemblyai.com", None),
            (
                "https://api.hyprnote.com/stt?provider=soniox",
                Some((
                    "wss://api.hyprnote.com/stt/listen",
                    vec![("provider", "soniox")],
                )),
            ),
            (
                "https://api.hyprnote.com/stt/listen?provider=deepgram",
                Some((
                    "wss://api.hyprnote.com/stt/listen",
                    vec![("provider", "deepgram")],
                )),
            ),
            (
                "https://api.hyprnote.com/stt/some/path?provider=fireworks",
                Some((
                    "wss://api.hyprnote.com/stt/some/path/listen",
                    vec![("provider", "fireworks")],
                )),
            ),
            (
                "http://localhost:8787/stt?provider=soniox",
                Some((
                    "ws://localhost:8787/stt/listen",
                    vec![("provider", "soniox")],
                )),
            ),
            (
                "http://localhost:8787/stt/listen?provider=deepgram",
                Some((
                    "ws://localhost:8787/stt/listen",
                    vec![("provider", "deepgram")],
                )),
            ),
            (
                "http://127.0.0.1:8787/stt?provider=assemblyai",
                Some((
                    "ws://127.0.0.1:8787/stt/listen",
                    vec![("provider", "assemblyai")],
                )),
            ),
        ];

        for (input, expected) in cases {
            let result = build_proxy_ws_url(input);
            match (result, expected) {
                (None, None) => {}
                (Some((url, params)), Some((expected_url, expected_params))) => {
                    assert_eq!(url.as_str(), *expected_url, "input: {}", input);
                    assert_eq!(
                        params,
                        expected_params
                            .iter()
                            .map(|(k, v)| (k.to_string(), v.to_string()))
                            .collect::<Vec<_>>(),
                        "input: {}",
                        input
                    );
                }
                (result, expected) => {
                    panic!(
                        "input: {}, expected: {:?}, got: {:?}",
                        input, expected, result
                    );
                }
            }
        }
    }

    #[test]
    fn test_hyprnote_proxy_always_selects_hyprnote_adapter() {
        use hypr_language::ISO639::*;

        let proxy_urls = &[
            "https://api.hyprnote.com/stt",
            "https://api.char.com/stt",
            "http://localhost:3001/stt",
            "http://127.0.0.1:3001/stt",
        ];

        let language_combos: &[&[hypr_language::ISO639]] =
            &[&[En], &[Ko], &[En, De], &[En, Ko], &[Ar]];

        for url in proxy_urls {
            for langs in language_combos {
                let langs: Vec<hypr_language::Language> =
                    langs.iter().map(|l| (*l).into()).collect();
                assert_eq!(
                    AdapterKind::from_url_and_languages(url, &langs, Some("cloud")),
                    AdapterKind::Hyprnote,
                    "proxy URL should always select Hyprnote adapter regardless of languages: url={url}, langs={langs:?}"
                );
            }
        }
    }

    #[test]
    fn test_hyprnote_adapter_supports_all_languages() {
        use hypr_language::ISO639::*;

        let combos: &[&[hypr_language::ISO639]] =
            &[&[En], &[Ko], &[Ar], &[En, De], &[En, Ko], &[Zh]];

        for langs in combos {
            let langs: Vec<hypr_language::Language> = langs.iter().map(|l| (*l).into()).collect();
            assert!(
                AdapterKind::Hyprnote.is_supported_languages_live(&langs, Some("cloud")),
                "Hyprnote adapter should support all languages: {langs:?}"
            );
        }
    }

    #[test]
    fn test_direct_provider_urls_not_affected() {
        use hypr_language::ISO639::*;

        let en: Vec<hypr_language::Language> = vec![En.into()];
        assert_eq!(
            AdapterKind::from_url_and_languages("https://api.deepgram.com/v1", &en, None),
            AdapterKind::Deepgram,
        );
        assert_eq!(
            AdapterKind::from_url_and_languages("https://api.soniox.com", &en, None),
            AdapterKind::Soniox,
        );
        assert_eq!(
            AdapterKind::from_url_and_languages("https://api.pyannote.ai", &en, None),
            AdapterKind::Pyannote,
        );
        assert_eq!(
            AdapterKind::from_url_and_languages("http://localhost:50060/v1", &en, None),
            AdapterKind::Argmax,
        );
        assert_eq!(
            AdapterKind::from_url_and_languages(
                "http://localhost:50060/v1",
                &en,
                Some("cactus-parakeet-tdt-0.6b-v3-int8"),
            ),
            AdapterKind::Cactus,
        );
    }

    #[test]
    fn test_append_provider_param_replaces_existing() {
        let url =
            append_provider_param("https://api.hyprnote.com/stt?provider=deepgram", "hyprnote");
        assert!(
            url.contains("provider=hyprnote"),
            "new provider value should be present: {url}"
        );
        assert!(
            !url.contains("provider=deepgram"),
            "old provider value should be removed: {url}"
        );
        assert_eq!(
            url.matches("provider=").count(),
            1,
            "exactly one provider param expected: {url}"
        );
    }

    #[test]
    fn test_append_provider_param_preserves_other_params() {
        let url = append_provider_param(
            "https://api.hyprnote.com/stt?model=cloud&provider=soniox&language=en",
            "hyprnote",
        );
        assert!(
            url.contains("model=cloud"),
            "model should be preserved: {url}"
        );
        assert!(
            url.contains("language=en"),
            "language should be preserved: {url}"
        );
        assert!(url.contains("provider=hyprnote"));
        assert!(!url.contains("provider=soniox"));
    }

    #[test]
    fn test_append_provider_param_no_existing_provider() {
        let url = append_provider_param("https://api.hyprnote.com/stt", "hyprnote");
        assert!(url.contains("provider=hyprnote"));
        assert_eq!(url.matches("provider=").count(), 1);
    }
}
