#[cfg(feature = "local")]
mod batch;
mod language;
mod live;

use serde::Serialize;

use super::{LanguageQuality, LanguageSupport};
use crate::providers::Provider;

// https://dev.meta.ai/docs/speech-to-text
pub(crate) const MODEL: &str = "muse-voice-transcribe-1.0";
pub(crate) const WS_PATH: &str = "/v1/asr/realtime";
pub(crate) const DEFAULT_API_BASE: &str = "https://api.meta.ai/v1";

#[derive(Clone, Default)]
pub struct MetaAdapter {
    live: std::sync::Arc<std::sync::Mutex<live::LiveState>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Authorization {
    access_token: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionConfig<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    authorization: Option<Authorization>,
    audio_encoding: &'static str,
    model: &'static str,
    mode: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    partial_mode: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    emit_audio_progress: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    language_bias: Vec<String>,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    keywords: &'a [String],
}

impl MetaAdapter {
    pub fn language_support_live(languages: &[anlg_language::Language]) -> LanguageSupport {
        Self::language_support_impl(languages)
    }

    pub fn language_support_batch(languages: &[anlg_language::Language]) -> LanguageSupport {
        Self::language_support_impl(languages)
    }

    fn language_support_impl(languages: &[anlg_language::Language]) -> LanguageSupport {
        if language::all_supported(languages) {
            LanguageSupport::Supported {
                quality: LanguageQuality::NoData,
            }
        } else {
            LanguageSupport::NotSupported
        }
    }

    pub(crate) fn build_ws_url_from_base(api_base: &str) -> (url::Url, Vec<(String, String)>) {
        super::build_ws_url_from_base_with(Provider::Meta, api_base, |parsed| {
            super::build_url_with_scheme(parsed, Provider::Meta.default_ws_host(), WS_PATH, true)
        })
    }

    // Muse Voice has turn-level timing only, so words share the turn evenly.
    fn word_spans(text: &str, start: f64, end: f64) -> Vec<(&str, f64, f64)> {
        let tokens: Vec<&str> = text.split_whitespace().collect();
        if tokens.is_empty() {
            return vec![];
        }
        let end = end.max(start);
        let word_duration = (end - start) / tokens.len() as f64;
        tokens
            .iter()
            .enumerate()
            .map(|(i, token)| {
                let word_start = start + word_duration * i as f64;
                let word_end = if i + 1 == tokens.len() {
                    end
                } else {
                    word_start + word_duration
                };
                (*token, word_start, word_end)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_ws_url_from_base() {
        for (input, expected_url, expected_params) in [
            ("", "wss://api.meta.ai/v1/asr/realtime", vec![]),
            (
                "https://api.meta.ai/v1",
                "wss://api.meta.ai/v1/asr/realtime",
                vec![],
            ),
            (
                "https://api.anarlog.so?provider=meta",
                "wss://api.anarlog.so/listen",
                vec![("provider", "meta")],
            ),
        ] {
            let (url, params) = MetaAdapter::build_ws_url_from_base(input);
            assert_eq!(url.as_str(), expected_url, "input: {input}");
            assert_eq!(
                params,
                expected_params
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect::<Vec<_>>(),
                "input: {input}"
            );
        }
    }

    #[test]
    fn test_is_meta_host() {
        assert!(Provider::Meta.is_host("api.meta.ai"));
        assert!(!Provider::Meta.is_host("api.openai.com"));
        assert_eq!(
            Provider::from_url("https://api.meta.ai/v1"),
            Some(Provider::Meta)
        );
    }

    #[test]
    fn word_spans_split_turn_evenly() {
        let spans = MetaAdapter::word_spans("hello big world", 1.0, 4.0);
        assert_eq!(
            spans,
            vec![("hello", 1.0, 2.0), ("big", 2.0, 3.0), ("world", 3.0, 4.0)]
        );
        assert!(MetaAdapter::word_spans("  ", 0.0, 1.0).is_empty());
        assert_eq!(
            MetaAdapter::word_spans("x", 2.0, 1.0),
            vec![("x", 2.0, 2.0)]
        );
    }
}
