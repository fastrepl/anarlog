mod batch;
mod live;

use super::{LanguageQuality, LanguageSupport};

pub const DEFAULT_MODEL: &str = "ink-whisper";
pub(crate) const API_VERSION: &str = "2026-03-01";

#[derive(Clone, Default)]
pub struct CartesiaAdapter;

impl CartesiaAdapter {
    pub fn language_support_live(languages: &[hypr_language::Language]) -> LanguageSupport {
        LanguageSupport::min(languages.iter().map(|language| {
            if language.iso639() == hypr_language::ISO639::En {
                LanguageSupport::Supported {
                    quality: LanguageQuality::NoData,
                }
            } else {
                LanguageSupport::NotSupported
            }
        }))
    }

    pub fn language_support_batch(languages: &[hypr_language::Language]) -> LanguageSupport {
        LanguageSupport::min(languages.iter().map(|language| {
            if BATCH_LANGUAGE_CODES.contains(&language.iso639().code()) {
                LanguageSupport::Supported {
                    quality: LanguageQuality::NoData,
                }
            } else {
                LanguageSupport::NotSupported
            }
        }))
    }

    pub fn is_supported_languages_batch(languages: &[hypr_language::Language]) -> bool {
        Self::language_support_batch(languages).is_supported()
    }

    pub fn is_supported_languages_live(languages: &[hypr_language::Language]) -> bool {
        Self::language_support_live(languages).is_supported()
    }

    pub(crate) fn build_ws_url_from_base(api_base: &str) -> (url::Url, Vec<(String, String)>) {
        super::build_ws_url_from_base_with(
            crate::providers::Provider::Cartesia,
            api_base,
            |parsed| {
                super::build_url_with_scheme(
                    parsed,
                    crate::providers::Provider::Cartesia.default_api_host(),
                    crate::providers::Provider::Cartesia.ws_path(),
                    true,
                )
            },
        )
    }
}

const BATCH_LANGUAGE_CODES: &[&str] = &[
    "af", "am", "ar", "as", "az", "ba", "be", "bg", "bn", "bo", "br", "bs", "ca", "cs", "cy", "da",
    "de", "el", "en", "es", "et", "eu", "fa", "fi", "fo", "fr", "gl", "gu", "ha", "haw", "he",
    "hi", "hr", "ht", "hu", "hy", "id", "is", "it", "ja", "jw", "ka", "kk", "km", "kn", "ko", "la",
    "lb", "ln", "lo", "lt", "lv", "mg", "mi", "mk", "ml", "mn", "mr", "ms", "mt", "my", "ne", "nl",
    "nn", "no", "oc", "pa", "pl", "ps", "pt", "ro", "ru", "sa", "sd", "si", "sk", "sl", "sn", "so",
    "sq", "sr", "su", "sv", "sw", "ta", "te", "tg", "th", "tk", "tl", "tr", "tt", "uk", "ur", "uz",
    "vi", "yi", "yo", "zh",
];

pub(super) fn documented_language_codes_live() -> impl Iterator<Item = &'static str> {
    ["en"].into_iter()
}

pub(super) fn documented_language_codes_batch() -> impl Iterator<Item = &'static str> {
    BATCH_LANGUAGE_CODES.iter().copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documented_language_codes_live_matches_ink_2_support() {
        let codes: Vec<_> = documented_language_codes_live().collect();

        assert_eq!(codes, vec!["en"]);
    }

    #[test]
    fn documented_language_codes_batch_includes_batch_catalog() {
        let codes: Vec<_> = documented_language_codes_batch().collect();

        assert!(codes.contains(&"ko"));
        assert!(codes.contains(&"zh"));
    }
}
