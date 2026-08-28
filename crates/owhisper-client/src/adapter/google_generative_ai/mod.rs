mod batch;
mod live;

use crate::providers::Provider;

use super::{LanguageQuality, LanguageSupport};

pub(crate) const LIVE_MODEL: &str = "gemini-3.5-transcribe-live";
pub(crate) const BATCH_MODEL: &str = "gemini-3.5-transcribe";

#[derive(Clone, Default)]
pub struct GoogleGenerativeAiAdapter;

impl GoogleGenerativeAiAdapter {
    pub fn language_support_live(_languages: &[anlg_language::Language]) -> LanguageSupport {
        LanguageSupport::Supported {
            quality: LanguageQuality::NoData,
        }
    }

    pub fn language_support_batch(languages: &[anlg_language::Language]) -> LanguageSupport {
        Self::language_support_live(languages)
    }

    pub(crate) fn resolve_live_model(model: Option<&str>) -> &str {
        match model {
            Some(model) if crate::providers::is_meta_model(model) => LIVE_MODEL,
            Some("gemini-3.5-transcribe-live" | "gemini-3.5-transcribe-live-preview") => LIVE_MODEL,
            Some(model) => model,
            None => LIVE_MODEL,
        }
    }

    pub(crate) fn resolve_batch_model(model: Option<&str>) -> &str {
        match model {
            Some(model) if crate::providers::is_meta_model(model) => BATCH_MODEL,
            Some(
                "gemini-3.5-transcribe"
                | "gemini-3.5-transcribe-preview"
                | "gemini-3.5-transcribe-live"
                | "gemini-3.5-transcribe-live-preview",
            ) => BATCH_MODEL,
            Some(model) => model,
            None => BATCH_MODEL,
        }
    }

    pub(crate) fn model_resource(model: &str) -> String {
        if model.starts_with("models/") {
            model.to_string()
        } else {
            format!("models/{model}")
        }
    }

    pub(crate) fn language_codes(languages: &[anlg_language::Language]) -> Vec<String> {
        languages
            .iter()
            .map(anlg_language::Language::bcp47_code)
            .filter(|code| !code.is_empty())
            .collect()
    }

    pub(crate) fn parse_duration_secs(value: &str) -> f64 {
        let value = value.trim();
        if let Some(rest) = value.strip_suffix("ms") {
            return rest.trim().parse::<f64>().ok().unwrap_or_default() / 1000.0;
        }
        if let Some(rest) = value.strip_suffix('s') {
            return rest.trim().parse().unwrap_or_default();
        }
        value.parse().unwrap_or_default()
    }

    pub(crate) fn parse_speaker_label(label: Option<&str>) -> Option<usize> {
        let label = label?.trim();
        if label.is_empty() {
            return None;
        }
        let digits: String = label.chars().filter(|ch| ch.is_ascii_digit()).collect();
        digits.parse().ok().filter(|speaker| *speaker > 0)
    }

    pub(crate) fn build_ws_url_from_base(api_base: &str) -> (url::Url, Vec<(String, String)>) {
        super::build_ws_url_from_base_with(Provider::GoogleGenerativeAi, api_base, |parsed| {
            super::build_url_with_scheme(
                parsed,
                Provider::GoogleGenerativeAi.default_ws_host(),
                Provider::GoogleGenerativeAi.ws_path(),
                true,
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_live_and_preview_ids_to_batch() {
        assert_eq!(
            GoogleGenerativeAiAdapter::resolve_batch_model(Some("gemini-3.5-transcribe-live")),
            BATCH_MODEL
        );
        assert_eq!(
            GoogleGenerativeAiAdapter::resolve_batch_model(Some(
                "gemini-3.5-transcribe-live-preview"
            )),
            BATCH_MODEL
        );
        assert_eq!(
            GoogleGenerativeAiAdapter::resolve_batch_model(Some("gemini-3.5-transcribe-preview")),
            BATCH_MODEL
        );
        assert_eq!(
            GoogleGenerativeAiAdapter::resolve_batch_model(None),
            BATCH_MODEL
        );
    }

    #[test]
    fn maps_preview_live_ids() {
        assert_eq!(
            GoogleGenerativeAiAdapter::resolve_live_model(Some(
                "gemini-3.5-transcribe-live-preview"
            )),
            LIVE_MODEL
        );
        assert_eq!(
            GoogleGenerativeAiAdapter::resolve_live_model(None),
            LIVE_MODEL
        );
    }

    #[test]
    fn prefixes_model_resources() {
        assert_eq!(
            GoogleGenerativeAiAdapter::model_resource("gemini-3.5-transcribe-live"),
            "models/gemini-3.5-transcribe-live"
        );
        assert_eq!(
            GoogleGenerativeAiAdapter::model_resource("models/gemini-3.5-transcribe"),
            "models/gemini-3.5-transcribe"
        );
    }

    #[test]
    fn parses_durations_and_speaker_labels() {
        assert_eq!(
            GoogleGenerativeAiAdapter::parse_duration_secs("0.500s"),
            0.5
        );
        assert_eq!(
            GoogleGenerativeAiAdapter::parse_duration_secs("250ms"),
            0.25
        );
        assert_eq!(
            GoogleGenerativeAiAdapter::parse_speaker_label(Some("Speaker 2")),
            Some(2)
        );
        assert_eq!(
            GoogleGenerativeAiAdapter::parse_speaker_label(Some("spk_1")),
            Some(1)
        );
        assert_eq!(
            GoogleGenerativeAiAdapter::parse_speaker_label(Some("")),
            None
        );
    }
}
