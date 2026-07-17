use crate::adapter::{LanguageQuality, LanguageSupport};

// https://docs.gladia.io/chapters/language/supported-languages
pub(super) const SUPPORTED_LANGUAGES: &[&str] = &[
    "af", "sq", "am", "ar", "hy", "as", "az", "ba", "eu", "be", "bn", "bs", "br", "bg", "ca", "zh",
    "hr", "cs", "da", "nl", "en", "et", "fo", "fi", "fr", "gl", "ka", "de", "el", "gu", "ht", "ha",
    "he", "hi", "hu", "is", "id", "it", "ja", "jw", "kn", "kk", "km", "ko", "lo", "la", "lv", "ln",
    "lt", "lb", "mk", "mg", "ms", "ml", "mt", "mi", "mr", "mn", "my", "ne", "no", "nn", "oc", "ps",
    "fa", "pl", "pt", "pa", "ro", "ru", "sa", "sr", "sn", "sd", "si", "sk", "sl", "so", "es", "su",
    "sw", "sv", "tl", "tg", "ta", "tt", "te", "th", "bo", "tr", "tk", "uk", "ur", "uz", "vi", "cy",
    "wo", "yi", "yo",
];

pub(super) fn single_language_support(language: &hypr_language::Language) -> LanguageSupport {
    let code = language.iso639().code();
    if SUPPORTED_LANGUAGES.contains(&code) {
        LanguageSupport::Supported {
            quality: LanguageQuality::NoData,
        }
    } else {
        LanguageSupport::NotSupported
    }
}
