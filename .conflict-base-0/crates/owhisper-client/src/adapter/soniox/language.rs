use crate::adapter::{LanguageQuality, LanguageSupport};

// https://soniox.com/docs/stt/concepts/supported-languages
pub(super) const SUPPORTED_LANGUAGES: &[&str] = &[
    "af", "sq", "ar", "az", "eu", "be", "bn", "bs", "bg", "ca", "zh", "hr", "cs", "da", "nl", "en",
    "et", "fi", "fr", "gl", "de", "el", "gu", "he", "hi", "hu", "id", "it", "ja", "kn", "kk", "ko",
    "lv", "lt", "mk", "ms", "ml", "mr", "no", "fa", "pl", "pt", "pa", "ro", "ru", "sr", "sk", "sl",
    "es", "sw", "sv", "tl", "ta", "te", "th", "tr", "uk", "ur", "vi", "cy",
];

const EXCELLENT_LANGS: &[&str] = &["ko", "ro", "it", "pt", "es", "vi"];

const HIGH_LANGS: &[&str] = &[
    "bg", "hu", "fr", "pl", "ru", "bn", "ur", "en", "zh", "bs", "sl", "mr", "de", "be", "af", "el",
    "hi", "he",
];

const GOOD_LANGS: &[&str] = &[
    "sq", "da", "gu", "az", "sr", "sv", "te", "no", "sk", "uk", "ja", "id", "tr", "kk", "sw", "nl",
    "lt", "hr", "th",
];

const MODERATE_LANGS: &[&str] = &[
    "et", "ta", "cs", "ms", "pa", "lv", "fi", "eu", "ca", "ml", "tl", "kn", "fa", "gl",
];

pub(super) fn single_language_support(language: &hypr_language::Language) -> LanguageSupport {
    let code = language.iso639().code();
    let quality = if EXCELLENT_LANGS.contains(&code) {
        LanguageQuality::Excellent
    } else if HIGH_LANGS.contains(&code) {
        LanguageQuality::High
    } else if GOOD_LANGS.contains(&code) {
        LanguageQuality::Good
    } else if MODERATE_LANGS.contains(&code) {
        LanguageQuality::Moderate
    } else if SUPPORTED_LANGUAGES.contains(&code) {
        LanguageQuality::NoData
    } else {
        return LanguageSupport::NotSupported;
    };
    LanguageSupport::Supported { quality }
}
