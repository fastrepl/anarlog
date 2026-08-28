mod batch;
mod language;
mod live;

use crate::adapter::LanguageSupport;
use crate::providers;

pub const DEFAULT_LIVE_MODEL: &str = "gemini-3.5-transcribe-live";
pub const DEFAULT_BATCH_MODEL: &str = "gemini-3.5-transcribe";
pub const WS_PATH: &str =
    "/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent";

#[derive(Clone, Default)]
pub struct GoogleGenerativeAiAdapter;

impl GoogleGenerativeAiAdapter {
    pub fn language_support_live(languages: &[anlg_language::Language]) -> LanguageSupport {
        language::language_support(languages)
    }

    pub fn language_support_batch(languages: &[anlg_language::Language]) -> LanguageSupport {
        language::language_support(languages)
    }

    pub fn documented_language_codes() -> &'static [&'static str] {
        language::DOCUMENTED_BCP47_CODES
    }

    pub fn is_live_model(model: &str) -> bool {
        model_id(model).contains("transcribe-live")
    }

    pub fn resolve_live_model(model: Option<&str>) -> String {
        match model {
            Some(model) if providers::is_meta_model(model) || model.is_empty() => {
                DEFAULT_LIVE_MODEL.to_string()
            }
            Some(model) if Self::is_live_model(model) => model_id(model).to_string(),
            _ => DEFAULT_LIVE_MODEL.to_string(),
        }
    }

    pub fn resolve_batch_model(model: Option<&str>) -> String {
        match model {
            Some(model) if providers::is_meta_model(model) || model.is_empty() => {
                DEFAULT_BATCH_MODEL.to_string()
            }
            Some(model) if Self::is_live_model(model) => DEFAULT_BATCH_MODEL.to_string(),
            Some(model) => model_id(model).to_string(),
            None => DEFAULT_BATCH_MODEL.to_string(),
        }
    }

    fn setup_model_name(model: &str) -> String {
        let model = model_id(model);
        if model.starts_with("models/") {
            model.to_string()
        } else {
            format!("models/{model}")
        }
    }

    fn language_codes(params: &owhisper_interface::ListenParams) -> Vec<String> {
        params
            .languages
            .iter()
            .map(anlg_language::Language::bcp47_code)
            .filter(|code| !code.is_empty())
            .collect()
    }
}

fn model_id(model: &str) -> &str {
    model.strip_prefix("models/").unwrap_or(model)
}

pub(crate) fn parse_duration_secs(value: &serde_json::Value) -> f64 {
    match value {
        serde_json::Value::Number(number) => number.as_f64().unwrap_or_default(),
        serde_json::Value::String(text) => parse_duration_str(text),
        serde_json::Value::Object(object) => {
            let seconds = object
                .get("seconds")
                .and_then(|value| value.as_f64().or_else(|| value.as_i64().map(|n| n as f64)))
                .unwrap_or_default();
            let nanos = object
                .get("nanos")
                .and_then(|value| value.as_f64().or_else(|| value.as_i64().map(|n| n as f64)))
                .unwrap_or_default();
            seconds + nanos / 1_000_000_000.0
        }
        _ => 0.0,
    }
}

fn parse_duration_str(value: &str) -> f64 {
    value
        .strip_suffix('s')
        .unwrap_or(value)
        .parse()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::LanguageQuality;

    #[test]
    fn classifies_live_and_batch_model_ids() {
        assert!(GoogleGenerativeAiAdapter::is_live_model(
            "gemini-3.5-transcribe-live"
        ));
        assert!(GoogleGenerativeAiAdapter::is_live_model(
            "models/gemini-3.5-transcribe-live-preview"
        ));
        assert!(!GoogleGenerativeAiAdapter::is_live_model(
            "gemini-3.5-transcribe"
        ));
        assert!(!GoogleGenerativeAiAdapter::is_live_model(
            "gemini-3.5-transcribe-preview"
        ));
    }

    #[test]
    fn maps_live_selection_to_file_model_for_batch() {
        assert_eq!(
            GoogleGenerativeAiAdapter::resolve_batch_model(Some("gemini-3.5-transcribe-live")),
            DEFAULT_BATCH_MODEL
        );
        assert_eq!(
            GoogleGenerativeAiAdapter::resolve_batch_model(Some("gemini-3.5-transcribe-preview")),
            "gemini-3.5-transcribe-preview"
        );
    }

    #[test]
    fn maps_file_selection_to_live_model_for_streaming() {
        assert_eq!(
            GoogleGenerativeAiAdapter::resolve_live_model(Some("gemini-3.5-transcribe")),
            DEFAULT_LIVE_MODEL
        );
        assert_eq!(
            GoogleGenerativeAiAdapter::resolve_live_model(Some(
                "gemini-3.5-transcribe-live-preview"
            )),
            "gemini-3.5-transcribe-live-preview"
        );
    }

    #[test]
    fn parses_protobuf_durations() {
        assert_eq!(parse_duration_secs(&serde_json::json!("0.250s")), 0.25);
        assert_eq!(
            parse_duration_secs(&serde_json::json!({ "seconds": 1, "nanos": 500_000_000 })),
            1.5
        );
        assert_eq!(parse_duration_secs(&serde_json::json!(2.5)), 2.5);
    }

    #[test]
    fn language_support_covers_documented_and_unknown_codes() {
        let english = vec![anlg_language::ISO639::En.into()];
        let korean = vec![anlg_language::ISO639::Ko.into()];

        assert!(GoogleGenerativeAiAdapter::language_support_live(&english).is_supported());
        assert!(GoogleGenerativeAiAdapter::language_support_batch(&korean).is_supported());
        assert!(GoogleGenerativeAiAdapter::language_support_live(&[]).is_supported());
        assert_eq!(
            GoogleGenerativeAiAdapter::language_support_live(&english).quality(),
            Some(LanguageQuality::High)
        );
    }
}
