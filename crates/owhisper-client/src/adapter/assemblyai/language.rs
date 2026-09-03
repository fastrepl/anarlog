use crate::adapter::{LanguageQuality, LanguageSupport};

// https://www.assemblyai.com/docs/streaming/select-the-speech-model
// Streaming only offers universal-3-5-pro (and its cheaper universal-streaming-*
// tiers, whose languages are a subset), so this is the whole live language set.
pub(super) const STREAMING_LANGUAGES: &[&str] = &[
    "en", "es", "de", "fr", "pt", "it", "tr", "nl", "sv", "no", "da", "fi", "hi", "vi", "ar", "he",
    "ja", "zh",
];

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

pub(super) fn single_language_support_live(language: &anlg_language::Language) -> LanguageSupport {
    if STREAMING_LANGUAGES.contains(&language.iso639().code()) {
        LanguageSupport::Supported {
            quality: LanguageQuality::High,
        }
    } else {
        LanguageSupport::NotSupported
    }
}

pub(super) fn single_language_support_batch(language: &anlg_language::Language) -> LanguageSupport {
    let code = language.iso639().code();
    if BATCH_LANGUAGES.contains(&code) {
        LanguageSupport::Supported {
            quality: LanguageQuality::NoData,
        }
    } else {
        LanguageSupport::NotSupported
    }
}
