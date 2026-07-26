use crate::adapter::{LanguageQuality, LanguageSupport};

// https://www.assemblyai.com/docs/api-reference/streaming-api/universal-3-pro-streaming/universal-3-pro-streaming
pub(super) const U3_STREAMING_LANGUAGES: &[&str] = &["en", "es", "fr", "de", "it", "pt"];

// https://www.assemblyai.com/docs/pre-recorded-audio/supported-languages
pub(super) const BATCH_LANGUAGES: &[&str] = &[
    // High
    "en", "es", "fr", "de", "id", "it", "ja", "nl", "pl", "pt", "ru", "tr", "uk", "ca",
    // Good
    "ar", "az", "bg", "bs", "zh", "cs", "da", "el", "et", "fi", "gl", "hi", "hr", "hu", "ko", "mk",
    "ms", "no", "ro", "sk", "sv", "th", "ur", "vi", // Moderate
    "af", "be", "cy", "fa", "he", "hy", "is", "kk", "lt", "lv", "mi", "mr", "sl", "sw", "ta",
    // Fair
    "am", "bn", "gu", "ka", "km", "kn", "lo", "ml", "mn", "mt", "my", "ne", "pa", "ps", "so", "sr",
    "te", "uz",
];

pub(super) const STREAMING_LANGUAGES: &[&str] = BATCH_LANGUAGES;

pub(super) fn single_language_support_live(language: &hypr_language::Language) -> LanguageSupport {
    let code = language.iso639().code();
    if U3_STREAMING_LANGUAGES.contains(&code) {
        LanguageSupport::Supported {
            quality: LanguageQuality::High,
        }
    } else if STREAMING_LANGUAGES.contains(&code) {
        LanguageSupport::Supported {
            quality: LanguageQuality::NoData,
        }
    } else {
        LanguageSupport::NotSupported
    }
}

pub(super) fn single_language_support_batch(language: &hypr_language::Language) -> LanguageSupport {
    let code = language.iso639().code();
    if BATCH_LANGUAGES.contains(&code) {
        LanguageSupport::Supported {
            quality: LanguageQuality::NoData,
        }
    } else {
        LanguageSupport::NotSupported
    }
}
