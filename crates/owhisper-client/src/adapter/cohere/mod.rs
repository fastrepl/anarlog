mod batch;

use super::{LanguageQuality, LanguageSupport};

#[derive(Clone, Default)]
pub struct CohereAdapter;

const SUPPORTED_LANGUAGE_CODES: &[&str] = &[
    "ar", "de", "el", "en", "es", "fr", "it", "ja", "ko", "nl", "pl", "pt", "vi", "zh",
];

impl CohereAdapter {
    pub fn language_support_batch(languages: &[anlg_language::Language]) -> LanguageSupport {
        if languages.len() <= 1
            && languages
                .iter()
                .all(|language| SUPPORTED_LANGUAGE_CODES.contains(&language.iso639_code()))
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
        assert!(CohereAdapter::language_support_batch(&[]).is_supported());
        assert!(CohereAdapter::language_support_batch(&[ISO639::En.into()]).is_supported());
        assert!(CohereAdapter::language_support_batch(&[ISO639::Ko.into()]).is_supported());
    }

    #[test]
    fn rejects_multiple_or_unsupported_languages() {
        assert!(
            !CohereAdapter::language_support_batch(&[ISO639::En.into(), ISO639::Ko.into()])
                .is_supported()
        );
        assert!(!CohereAdapter::language_support_batch(&[ISO639::Ru.into()]).is_supported());
    }
}
