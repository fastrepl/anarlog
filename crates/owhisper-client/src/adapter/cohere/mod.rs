mod batch;

use super::{LanguageQuality, LanguageSupport};

#[derive(Clone, Default)]
pub struct CohereAdapter;

pub(crate) const DEFAULT_MODEL: &str = "cohere-transcribe-03-2026";
pub(crate) const ARABIC_MODEL: &str = "cohere-transcribe-arabic-07-2026";

const SUPPORTED_LANGUAGE_CODES: &[&str] = &[
    "ar", "de", "el", "en", "es", "fr", "it", "ja", "ko", "nl", "pl", "pt", "vi", "zh",
];
// The Arabic finetune handles Arabic dialects plus English code-switching.
const ARABIC_MODEL_LANGUAGE_CODES: &[&str] = &["ar", "en"];

impl CohereAdapter {
    pub(crate) fn resolve_model(model: Option<&str>) -> &str {
        match model {
            Some(model) if !model.is_empty() && !crate::providers::is_meta_model(model) => model,
            _ => DEFAULT_MODEL,
        }
    }

    pub fn language_support_batch(
        languages: &[anlg_language::Language],
        model: Option<&str>,
    ) -> LanguageSupport {
        let supported = if Self::resolve_model(model) == ARABIC_MODEL {
            ARABIC_MODEL_LANGUAGE_CODES
        } else {
            SUPPORTED_LANGUAGE_CODES
        };

        if languages.len() <= 1
            && languages
                .iter()
                .all(|language| supported.contains(&language.iso639_code()))
        {
            LanguageSupport::Supported {
                quality: LanguageQuality::NoData,
            }
        } else {
            LanguageSupport::NotSupported
        }
    }
}

pub(super) fn documented_language_codes() -> &'static [&'static str] {
    SUPPORTED_LANGUAGE_CODES
}

#[cfg(test)]
mod tests {
    use anlg_language::ISO639;

    use super::*;

    #[test]
    fn supports_one_documented_language() {
        assert!(CohereAdapter::language_support_batch(&[], None).is_supported());
        assert!(CohereAdapter::language_support_batch(&[ISO639::En.into()], None).is_supported());
        assert!(
            CohereAdapter::language_support_batch(&[ISO639::Ko.into()], Some("cloud"))
                .is_supported()
        );
    }

    #[test]
    fn rejects_multiple_or_unsupported_languages() {
        assert!(
            !CohereAdapter::language_support_batch(&[ISO639::En.into(), ISO639::Ko.into()], None)
                .is_supported()
        );
        assert!(!CohereAdapter::language_support_batch(&[ISO639::Ru.into()], None).is_supported());
    }

    #[test]
    fn arabic_model_only_accepts_arabic_or_english() {
        assert!(
            CohereAdapter::language_support_batch(&[ISO639::Ar.into()], Some(ARABIC_MODEL))
                .is_supported()
        );
        assert!(
            CohereAdapter::language_support_batch(&[ISO639::En.into()], Some(ARABIC_MODEL))
                .is_supported()
        );
        assert!(
            !CohereAdapter::language_support_batch(&[ISO639::Ko.into()], Some(ARABIC_MODEL))
                .is_supported()
        );
    }
}
