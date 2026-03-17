#[cfg(feature = "argmax")]
mod batch;
pub(crate) mod keywords;
pub(crate) mod language;
mod live;

#[cfg(feature = "argmax")]
pub use batch::StreamingBatchConfig;

pub use language::PARAKEET_V3_LANGS;

use super::{LanguageQuality, LanguageSupport};

#[derive(Clone, Default)]
pub struct ArgmaxAdapter;

impl ArgmaxAdapter {
    pub fn language_support_live(
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> LanguageSupport {
        let model = model.unwrap_or("");

        if languages.len() > 1 {
            return LanguageSupport::NotSupported;
        }

        if model.contains("parakeet") && model.contains("v2") {
            if languages.iter().any(|lang| lang.iso639().code() == "en") {
                LanguageSupport::Supported {
                    quality: LanguageQuality::NoData,
                }
            } else {
                LanguageSupport::NotSupported
            }
        } else if model.contains("parakeet") && model.contains("v3") {
            if languages
                .iter()
                .any(|lang| PARAKEET_V3_LANGS.contains(&lang.iso639().code()))
            {
                LanguageSupport::Supported {
                    quality: LanguageQuality::NoData,
                }
            } else {
                LanguageSupport::NotSupported
            }
        } else {
            LanguageSupport::Supported {
                quality: LanguageQuality::NoData,
            }
        }
    }

    pub fn language_support_batch(
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> LanguageSupport {
        Self::language_support_live(languages, model)
    }

    pub fn is_supported_languages_live(
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> bool {
        Self::language_support_live(languages, model).is_supported()
    }

    pub fn is_supported_languages_batch(
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> bool {
        Self::language_support_batch(languages, model).is_supported()
    }
}
