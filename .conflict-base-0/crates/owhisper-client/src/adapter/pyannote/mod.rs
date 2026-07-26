mod batch;
mod language;
mod model;

use super::{LanguageQuality, LanguageSupport, append_path_if_missing};
pub use model::PyannoteTranscriptionModel;
use model::TRANSCRIPTION_MODELS;

#[derive(Clone, Default)]
pub struct PyannoteAdapter;

const DEFAULT_BASE_URL: &str = "https://api.pyannote.ai";

impl PyannoteAdapter {
    pub fn language_support_batch(
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> LanguageSupport {
        if languages.is_empty() {
            return LanguageSupport::Supported {
                quality: LanguageQuality::NoData,
            };
        }

        let model = match model {
            Some(model) => match model.parse::<PyannoteTranscriptionModel>() {
                Ok(model) => model,
                Err(_) => return LanguageSupport::NotSupported,
            },
            None => match Self::find_model(languages) {
                Some(model) => model,
                None => return LanguageSupport::NotSupported,
            },
        };

        if languages.iter().all(|lang| model.supports_language(lang)) {
            LanguageSupport::Supported {
                quality: LanguageQuality::NoData,
            }
        } else {
            LanguageSupport::NotSupported
        }
    }

    pub fn find_model(languages: &[hypr_language::Language]) -> Option<PyannoteTranscriptionModel> {
        TRANSCRIPTION_MODELS
            .iter()
            .find(|model| languages.iter().all(|lang| model.supports_language(lang)))
            .copied()
    }

    pub fn resolve_transcription_model(model: Option<&str>) -> PyannoteTranscriptionModel {
        model
            .and_then(|model| model.parse::<PyannoteTranscriptionModel>().ok())
            .unwrap_or_default()
    }

    pub(crate) fn batch_api_url(api_base: &str) -> url::Url {
        if api_base.is_empty() {
            return DEFAULT_BASE_URL
                .parse()
                .expect("invalid_default_pyannote_api_url");
        }

        let mut url: url::Url = api_base.parse().expect("invalid_api_base");
        append_path_if_missing(&mut url, "v1");
        url
    }
}

pub(super) fn documented_language_codes() -> Vec<&'static str> {
    let mut codes = Vec::new();
    codes.extend_from_slice(language::PARAKEET_TDT_06B_V3_LANGUAGES);
    codes.extend_from_slice(language::FASTER_WHISPER_LARGE_V3_TURBO_LANGUAGES);
    codes
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_language::{ISO639, Language};

    #[test]
    fn test_batch_api_url_empty_uses_default() {
        let url = PyannoteAdapter::batch_api_url("");
        assert_eq!(url.as_str(), "https://api.pyannote.ai/");
    }

    #[test]
    fn test_batch_api_url_appends_v1() {
        let url = PyannoteAdapter::batch_api_url("https://api.pyannote.ai");
        assert_eq!(url.as_str(), "https://api.pyannote.ai/v1");
    }

    #[test]
    fn test_batch_api_url_preserves_nested_prefix() {
        let url = PyannoteAdapter::batch_api_url("https://api.char.com/pyannote");
        assert_eq!(url.as_str(), "https://api.char.com/pyannote/v1");
    }

    #[test]
    fn test_resolve_transcription_model_defaults() {
        assert_eq!(
            PyannoteAdapter::resolve_transcription_model(None),
            PyannoteTranscriptionModel::ParakeetTdt06bV3
        );
        assert_eq!(
            PyannoteAdapter::resolve_transcription_model(Some("unknown-model")),
            PyannoteTranscriptionModel::ParakeetTdt06bV3
        );
    }

    #[test]
    fn test_find_model_prefers_parakeet_when_possible() {
        let model = PyannoteAdapter::find_model(&[Language::new(ISO639::En)]);
        assert_eq!(model, Some(PyannoteTranscriptionModel::ParakeetTdt06bV3));
    }

    #[test]
    fn test_find_model_falls_back_to_faster_whisper() {
        let model = PyannoteAdapter::find_model(&[Language::new(ISO639::Ja)]);
        assert_eq!(
            model,
            Some(PyannoteTranscriptionModel::FasterWhisperLargeV3Turbo)
        );
    }

    #[test]
    fn test_language_support_rejects_unknown_model() {
        let support =
            PyannoteAdapter::language_support_batch(&[Language::new(ISO639::En)], Some("nope"));
        assert_eq!(support, LanguageSupport::NotSupported);
    }
}
