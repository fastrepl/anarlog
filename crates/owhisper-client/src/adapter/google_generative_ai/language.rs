use crate::adapter::{LanguageQuality, LanguageSupport};

// https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/gemini/3-5-transcribe
pub const DOCUMENTED_BCP47_CODES: &[&str] = &[
    "af-ZA",
    "am-ET",
    "ar-EG",
    "hy-AM",
    "as-IN",
    "az-AZ",
    "be-BY",
    "bn-BD",
    "bn-IN",
    "bs-BA",
    "bg-BG",
    "rup-BG",
    "my-MM",
    "yue-Hant-HK",
    "ca-ES",
    "ceb",
    "km-KH",
    "hr-HR",
    "cs-CZ",
    "da-DK",
    "nl-NL",
    "en-AU",
    "en-GB",
    "en-IN",
    "en-US",
    "et-EE",
    "fa-IR",
    "fil-PH",
    "fi-FI",
    "fr-FR",
    "fr-CA",
    "gl-ES",
    "ka-GE",
    "de-DE",
    "el-GR",
    "gu-IN",
    "ha-NG",
    "he-IL",
    "hi-IN",
    "hu-HU",
    "is-IS",
    "id-ID",
    "it-IT",
    "ja-JP",
    "jv-ID",
    "kn-IN",
    "kk-KZ",
    "ko-KR",
    "ky-KG",
    "lv-LV",
    "ln-CD",
    "lt-LT",
    "mk-MK",
    "ms-MY",
    "ml-IN",
    "mt-MT",
    "cmn-Hans-CN",
    "mr-IN",
    "mn-MN",
    "ne-NP",
    "nb-NO",
    "or-IN",
    "pl-PL",
    "pt-BR",
    "pt-PT",
    "pa-IN",
    "pa-Guru-IN",
    "ro-RO",
    "ru-RU",
    "sr-RS",
    "sd-Arab-IN",
    "sk-SK",
    "sl-SI",
    "kea-CV",
    "es-419",
    "es-ES",
    "es-US",
    "sw-KE",
    "sv-SE",
    "tg-TJ",
    "te-IN",
    "th-TH",
    "tr-TR",
    "uk-UA",
    "uz-UZ",
    "vi-VN",
];

const HIGH_QUALITY: &[&str] = &[
    "ca", "hr", "da", "nl", "en", "fi", "fr", "de", "el", "hi", "it", "ja", "ko", "zh", "pl", "pt",
    "ro", "ru", "es", "sv", "tr", "uk", "vi",
];

const DOCUMENTED_ISO: &[&str] = &[
    "af", "am", "ar", "hy", "as", "az", "be", "bn", "bs", "bg", "my", "yue", "ca", "ceb", "km",
    "hr", "cs", "da", "nl", "en", "et", "fa", "tl", "fi", "fr", "gl", "ka", "de", "el", "gu", "ha",
    "he", "hi", "hu", "is", "id", "it", "ja", "jv", "kn", "kk", "ko", "ky", "lv", "ln", "lt", "mk",
    "ms", "ml", "mt", "zh", "mr", "mn", "ne", "no", "or", "pl", "pt", "pa", "ro", "ru", "sr", "sk",
    "sl", "es", "sw", "sv", "tg", "te", "th", "tr", "uk", "uz", "vi",
];

pub fn language_support(languages: &[anlg_language::Language]) -> LanguageSupport {
    if languages.is_empty() {
        return LanguageSupport::Supported {
            quality: LanguageQuality::High,
        };
    }

    LanguageSupport::min(languages.iter().map(|language| {
        let code = language.iso639().code();
        if HIGH_QUALITY.contains(&code) {
            LanguageSupport::Supported {
                quality: LanguageQuality::High,
            }
        } else if DOCUMENTED_ISO.contains(&code) {
            LanguageSupport::Supported {
                quality: LanguageQuality::Moderate,
            }
        } else {
            LanguageSupport::Supported {
                quality: LanguageQuality::NoData,
            }
        }
    }))
}
