use crate::adapter::{LanguageQuality, LanguageSupport};

// https://elevenlabs.io/docs/overview/capabilities/speech-to-text
// Accepts both ISO 639-1 (2-letter) and ISO 639-3 (3-letter) codes

pub(super) const EXCELLENT_LANGS: &[&str] = &[
    "be", "bs", "bg", "ca", "hr", "cs", "da", "nl", "en", "et", "fi", "fr", "gl", "de", "el", "hu",
    "is", "id", "it", "ja", "kn", "lv", "mk", "ms", "ml", "no", "pl", "pt", "ro", "ru", "sk", "es",
    "sv", "tr", "uk", "vi",
];

pub(super) const HIGH_LANGS: &[&str] = &[
    "hy", "az", "bn", "tl", "ka", "gu", "hi", "kk", "lt", "mt", "zh", "mr", "ne", "or", "fa", "sr",
    "sl", "sw", "ta", "te",
];

pub(super) const GOOD_LANGS: &[&str] = &[
    "af", "ar", "as", "my", "ha", "he", "jv", "ko", "ky", "lb", "mi", "oc", "pa", "tg", "th", "uz",
    "cy",
];

pub(super) const MODERATE_LANGS: &[&str] = &[
    "am", "lg", "ig", "ga", "km", "ku", "lo", "mn", "ps", "sn", "sd", "so", "ur", "wo", "xh", "yo",
    "zu",
];

pub(super) const NO_DATA_LANGS: &[&str] = &["ff", "ln", "ny"];

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
    } else if NO_DATA_LANGS.contains(&code) {
        LanguageQuality::NoData
    } else {
        return LanguageSupport::NotSupported;
    };
    LanguageSupport::Supported { quality }
}
