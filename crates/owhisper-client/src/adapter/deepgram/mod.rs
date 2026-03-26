mod batch;
mod callback;
pub mod error;
mod keywords;
mod language;
mod live;

use super::{LanguageQuality, LanguageSupport};

// https://developers.deepgram.com/docs/models-languages-overview
const NOVA3_GENERAL_LANGUAGES: &[&str] = &[
    "ar", "ar-AE", "ar-SA", "ar-QA", "ar-KW", "ar-SY", "ar-LB", "ar-PS", "ar-JO", "ar-EG", "ar-SD",
    "ar-TD", "ar-MA", "ar-DZ", "ar-TN", "ar-IQ", "ar-IR", "be", "bn", "bs", "bg", "ca", "hr", "cs",
    "da", "da-DK", "nl", "en", "en-US", "en-AU", "en-GB", "en-IN", "en-NZ", "et", "fi", "nl-BE",
    "fr", "fr-CA", "de", "de-CH", "el", "he", "hi", "hu", "id", "it", "ja", "kn", "ko", "ko-KR",
    "lv", "lt", "mk", "ms", "mr", "no", "fa", "pl", "pt", "pt-BR", "pt-PT", "ro", "ru", "sr", "sk",
    "sl", "es", "es-419", "sv", "sv-SE", "tl", "ta", "te", "tr", "uk", "ur", "vi",
];

const NOVA2_GENERAL_LANGUAGES: &[&str] = &[
    "bg", "ca", "cs", "da", "da-DK", "de", "de-CH", "el", "en", "en-AU", "en-GB", "en-IN", "en-NZ",
    "en-US", "es", "es-419", "et", "fi", "fr", "fr-CA", "hi", "hu", "id", "it", "ja", "ko",
    "ko-KR", "lt", "lv", "ms", "nl", "nl-BE", "no", "pl", "pt", "pt-BR", "pt-PT", "ro", "ru", "sk",
    "sv", "sv-SE", "th", "th-TH", "tr", "uk", "vi", "zh", "zh-CN", "zh-HK", "zh-Hans", "zh-Hant",
    "zh-TW",
];

const NOVA3_MEDICAL_LANGUAGES: &[&str] = &[
    "en", "en-AU", "en-CA", "en-GB", "en-IE", "en-IN", "en-NZ", "en-US",
];

const ENGLISH_ONLY: &[&str] = &["en", "en-US"];

const EXCELLENT_LANGS: &[&str] = &["ru", "en", "es", "pl", "fr", "it"];

const HIGH_LANGS: &[&str] = &["nl", "de", "ko", "pt", "sv", "uk", "vi"];

const GOOD_LANGS: &[&str] = &["tr", "fi", "da", "id", "el", "no", "ca"];

const MODERATE_LANGS: &[&str] = &["ja", "cs", "sk", "hu", "bg", "hi", "ms", "ro", "et"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, strum::EnumString, strum::AsRefStr)]
pub enum DeepgramModel {
    #[default]
    #[strum(serialize = "nova-3", serialize = "nova-3-general")]
    Nova3General,
    #[strum(serialize = "nova-3-medical")]
    Nova3Medical,
    #[strum(serialize = "nova-2", serialize = "nova-2-general")]
    Nova2General,
    #[strum(
        serialize = "nova-2-meeting",
        serialize = "nova-2-phonecall",
        serialize = "nova-2-finance",
        serialize = "nova-2-conversationalai",
        serialize = "nova-2-voicemail",
        serialize = "nova-2-video",
        serialize = "nova-2-medical",
        serialize = "nova-2-drivethru",
        serialize = "nova-2-automotive",
        serialize = "nova-2-atc"
    )]
    Nova2Specialized,
}

impl DeepgramModel {
    pub fn supported_languages(&self) -> &'static [&'static str] {
        match self {
            Self::Nova3General => NOVA3_GENERAL_LANGUAGES,
            Self::Nova3Medical => NOVA3_MEDICAL_LANGUAGES,
            Self::Nova2General => NOVA2_GENERAL_LANGUAGES,
            Self::Nova2Specialized => ENGLISH_ONLY,
        }
    }

    pub fn supports_language(&self, lang: &hypr_language::Language) -> bool {
        lang.matches_any_code(self.supported_languages())
    }

    pub fn supports_multi(&self, languages: &[hypr_language::Language]) -> bool {
        language::can_use_multi(self.as_ref(), languages)
    }
}

const MODELS: &[DeepgramModel] = &[
    DeepgramModel::Nova3General,
    DeepgramModel::Nova3Medical,
    DeepgramModel::Nova2General,
];

#[derive(Clone, Default)]
pub struct DeepgramAdapter;

impl DeepgramAdapter {
    pub fn find_model(languages: &[hypr_language::Language]) -> Option<DeepgramModel> {
        if languages.len() >= 2 {
            MODELS.iter().find(|m| m.supports_multi(languages)).copied()
        } else {
            let primary = languages.first()?;
            MODELS
                .iter()
                .find(|m| m.supports_language(primary))
                .copied()
        }
    }

    pub fn language_support_live(
        languages: &[hypr_language::Language],
        model: Option<DeepgramModel>,
    ) -> LanguageSupport {
        Self::language_support_impl(languages, model)
    }

    pub fn language_support_batch(
        languages: &[hypr_language::Language],
        model: Option<DeepgramModel>,
    ) -> LanguageSupport {
        Self::language_support_impl(languages, model)
    }

    fn language_support_impl(
        languages: &[hypr_language::Language],
        model: Option<DeepgramModel>,
    ) -> LanguageSupport {
        if languages.is_empty() {
            return LanguageSupport::Supported {
                quality: LanguageQuality::NoData,
            };
        }

        if languages.len() >= 2 {
            let effective_model = model.unwrap_or_default();
            if !effective_model.supports_multi(languages) && !Self::can_use_multi(languages) {
                return LanguageSupport::NotSupported;
            }
        }

        if let Some(m) = model {
            if !languages.iter().all(|lang| m.supports_language(lang)) {
                return LanguageSupport::NotSupported;
            }
        } else if Self::find_model(languages).is_none() {
            return LanguageSupport::NotSupported;
        }

        LanguageSupport::min(languages.iter().map(Self::single_language_support))
    }

    pub fn is_supported_languages_live(
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> bool {
        let model = model.and_then(|m| m.parse::<DeepgramModel>().ok());
        Self::language_support_live(languages, model).is_supported()
    }

    pub fn is_supported_languages_batch(
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> bool {
        let model = model.and_then(|m| m.parse::<DeepgramModel>().ok());
        Self::language_support_batch(languages, model).is_supported()
    }

    fn can_use_multi(languages: &[hypr_language::Language]) -> bool {
        language::can_use_multi(DeepgramModel::Nova3General.as_ref(), languages)
            || language::can_use_multi(DeepgramModel::Nova2General.as_ref(), languages)
    }

    fn single_language_support(language: &hypr_language::Language) -> LanguageSupport {
        let code = language.iso639().code();
        let quality = if EXCELLENT_LANGS.contains(&code) {
            LanguageQuality::Excellent
        } else if HIGH_LANGS.contains(&code) {
            LanguageQuality::High
        } else if GOOD_LANGS.contains(&code) {
            LanguageQuality::Good
        } else if MODERATE_LANGS.contains(&code) {
            LanguageQuality::Moderate
        } else if Self::find_model(std::slice::from_ref(language)).is_some() {
            LanguageQuality::NoData
        } else {
            return LanguageSupport::NotSupported;
        };
        LanguageSupport::Supported { quality }
    }

    pub fn recommended_model_live(languages: &[hypr_language::Language]) -> Option<&'static str> {
        match Self::find_model(languages) {
            Some(DeepgramModel::Nova3General) => Some("nova-3"),
            Some(DeepgramModel::Nova3Medical) => Some("nova-3-medical"),
            Some(DeepgramModel::Nova2General) => Some("nova-2"),
            Some(DeepgramModel::Nova2Specialized) => Some("nova-2"),
            None => None,
        }
    }
}

pub(super) fn documented_language_codes() -> Vec<&'static str> {
    let mut codes = Vec::new();
    codes.extend_from_slice(NOVA3_GENERAL_LANGUAGES);
    codes.extend_from_slice(NOVA2_GENERAL_LANGUAGES);
    codes.extend_from_slice(NOVA3_MEDICAL_LANGUAGES);
    codes.extend_from_slice(ENGLISH_ONLY);
    codes
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_language::{ISO639, Language};

    #[test]
    fn test_recommended_model_live() {
        let cases: Vec<(Vec<Language>, Option<&str>)> = vec![
            (vec![Language::new(ISO639::En)], Some("nova-3")),
            (vec![Language::new(ISO639::Ja)], Some("nova-3")),
            (vec![Language::new(ISO639::Zh)], Some("nova-2")),
            (vec![Language::new(ISO639::Th)], Some("nova-2")),
            (vec![Language::new(ISO639::Ar)], Some("nova-3")),
            (vec![], None),
            (
                vec![Language::with_region(ISO639::En, "CA")],
                Some("nova-3-medical"),
            ),
            (
                vec![Language::new(ISO639::En), Language::new(ISO639::Es)],
                Some("nova-3"),
            ),
            (
                vec![Language::new(ISO639::En), Language::new(ISO639::Fr)],
                Some("nova-3"),
            ),
            (
                vec![Language::new(ISO639::En), Language::new(ISO639::Ja)],
                Some("nova-3"),
            ),
            (
                vec![Language::new(ISO639::Fr), Language::new(ISO639::De)],
                Some("nova-3"),
            ),
            (
                vec![Language::new(ISO639::En), Language::new(ISO639::Ko)],
                None,
            ),
            (
                vec![Language::new(ISO639::En), Language::new(ISO639::Zh)],
                None,
            ),
            (
                vec![Language::new(ISO639::Ko), Language::new(ISO639::En)],
                None,
            ),
        ];

        for (languages, expected) in cases {
            assert_eq!(
                DeepgramAdapter::recommended_model_live(&languages),
                expected,
                "failed for {:?}",
                languages
            );
        }
    }

    #[test]
    fn test_language_support_with_model() {
        let cases: Vec<(Vec<Language>, DeepgramModel, bool)> = vec![
            (
                vec![Language::new(ISO639::En)],
                DeepgramModel::Nova3General,
                true,
            ),
            (
                vec![Language::new(ISO639::En)],
                DeepgramModel::Nova3Medical,
                true,
            ),
            (
                vec![Language::new(ISO639::En)],
                DeepgramModel::Nova2General,
                true,
            ),
            (
                vec![Language::with_region(ISO639::En, "CA")],
                DeepgramModel::Nova3Medical,
                true,
            ),
        ];

        for (languages, model, expected) in cases {
            assert_eq!(
                DeepgramAdapter::language_support_live(&languages, Some(model)).is_supported(),
                expected,
                "failed for {:?} with {:?}",
                languages,
                model
            );
        }
    }

    #[test]
    fn test_language_support_quality() {
        let en: Vec<hypr_language::Language> = vec![ISO639::En.into()];
        let support = DeepgramAdapter::language_support_live(&en, None);
        assert_eq!(support.quality(), Some(LanguageQuality::Excellent));

        let ja: Vec<hypr_language::Language> = vec![ISO639::Ja.into()];
        let support = DeepgramAdapter::language_support_live(&ja, None);
        assert_eq!(support.quality(), Some(LanguageQuality::Moderate));
    }

    #[test]
    fn test_model_supports_language() {
        let en: hypr_language::Language = ISO639::En.into();
        let zh: hypr_language::Language = ISO639::Zh.into();

        assert!(DeepgramModel::Nova3General.supports_language(&en));
        assert!(!DeepgramModel::Nova3General.supports_language(&zh));
        assert!(DeepgramModel::Nova2General.supports_language(&zh));
    }

    #[test]
    fn test_en_ca_with_nova3_general_not_supported() {
        let en_ca: hypr_language::Language = "en-CA".parse().unwrap();
        let languages = vec![en_ca];

        assert!(!DeepgramAdapter::is_supported_languages_live(
            &languages,
            Some("nova-3-general")
        ));

        assert!(
            !DeepgramAdapter::language_support_live(&languages, Some(DeepgramModel::Nova3General))
                .is_supported()
        );
    }

    #[test]
    fn test_en_ca_with_nova3_medical_supported() {
        let en_ca: hypr_language::Language = "en-CA".parse().unwrap();
        let languages = vec![en_ca];

        assert!(DeepgramAdapter::is_supported_languages_live(
            &languages,
            Some("nova-3-medical")
        ));

        assert!(
            DeepgramAdapter::language_support_live(&languages, Some(DeepgramModel::Nova3Medical))
                .is_supported()
        );
    }

    #[test]
    fn test_en_ca_auto_selects_nova3_medical() {
        let en_ca: hypr_language::Language = "en-CA".parse().unwrap();
        let languages = vec![en_ca];

        assert_eq!(
            DeepgramAdapter::recommended_model_live(&languages),
            Some("nova-3-medical")
        );
    }

    #[test]
    fn test_en_us_with_nova3_general_supported() {
        let en_us: hypr_language::Language = "en-US".parse().unwrap();
        let languages = vec![en_us];

        assert!(DeepgramAdapter::is_supported_languages_live(
            &languages,
            Some("nova-3-general")
        ));
    }

    #[test]
    fn test_is_supported_languages_live() {
        let cases: &[(&[ISO639], bool)] = &[
            (&[ISO639::En], true),
            (&[ISO639::Zh], true),
            (&[ISO639::Th], true),
            (&[ISO639::Ar], true),
            (&[], true),
            (&[ISO639::En, ISO639::Es], true),
            (&[ISO639::En, ISO639::Ko], false),
            (&[ISO639::En, ISO639::Es, ISO639::Ko], false),
        ];

        for (iso_codes, expected) in cases {
            let langs: Vec<Language> = iso_codes.iter().map(|&iso| iso.into()).collect();
            assert_eq!(
                DeepgramAdapter::is_supported_languages_live(&langs, None),
                *expected,
                "failed for {:?}",
                iso_codes
            );
        }
    }

    #[test]
    fn test_can_use_multi() {
        let cases: &[(&str, &[ISO639], bool)] = &[
            ("nova-3", &[ISO639::En, ISO639::Es], true),
            ("nova-3", &[ISO639::En, ISO639::Fr], true),
            ("nova-3", &[ISO639::Fr, ISO639::De], true),
            ("nova-3", &[ISO639::En, ISO639::Ko], false),
            ("nova-3", &[ISO639::En, ISO639::Es, ISO639::Ko], false),
            ("nova-3", &[ISO639::En], false),
            ("nova-3", &[], false),
            ("nova-2", &[ISO639::En, ISO639::Es], true),
            ("nova-2", &[ISO639::En, ISO639::Fr], false),
            ("nova-2", &[ISO639::En, ISO639::Ja], false),
            ("nova-2", &[ISO639::Fr, ISO639::De], false),
            ("nova", &[ISO639::En, ISO639::Es], false),
            ("nova-1", &[ISO639::En, ISO639::Es], false),
            ("enhanced", &[ISO639::En, ISO639::Es], false),
            ("base", &[ISO639::En, ISO639::Es], false),
            ("whisper", &[ISO639::En, ISO639::Es], false),
            ("NOVA-3", &[ISO639::En, ISO639::Es], false),
            ("Nova-3", &[ISO639::En, ISO639::Es], false),
            ("", &[ISO639::En, ISO639::Es], false),
            ("   ", &[ISO639::En, ISO639::Es], false),
            ("nova-3-general", &[ISO639::En, ISO639::Es], true),
            ("nova-3-medical", &[ISO639::En, ISO639::Es], true),
            ("my-nova-3-custom", &[ISO639::En, ISO639::Es], true),
            ("nova-2-general", &[ISO639::En, ISO639::Es], true),
            ("nova-2-phonecall", &[ISO639::En, ISO639::Es], true),
        ];

        for (model, iso_codes, expected) in cases {
            let langs: Vec<Language> = iso_codes.iter().map(|&iso| iso.into()).collect();
            assert_eq!(
                language::can_use_multi(model, &langs),
                *expected,
                "failed for model={}, langs={:?}",
                model,
                iso_codes
            );
        }
    }

    #[test]
    fn test_nova3_multi_supports_all_10_languages() {
        let all_nova3_multi: Vec<Language> = vec![
            ISO639::En.into(),
            ISO639::Es.into(),
            ISO639::Fr.into(),
            ISO639::De.into(),
            ISO639::Hi.into(),
            ISO639::Ru.into(),
            ISO639::Pt.into(),
            ISO639::Ja.into(),
            ISO639::It.into(),
            ISO639::Nl.into(),
        ];
        assert!(language::can_use_multi("nova-3", &all_nova3_multi));
    }

    #[test]
    fn test_model_enum_parsing() {
        let valid_cases: &[(&str, DeepgramModel)] = &[
            ("nova-3", DeepgramModel::Nova3General),
            ("nova-3-general", DeepgramModel::Nova3General),
            ("nova-3-medical", DeepgramModel::Nova3Medical),
            ("nova-2", DeepgramModel::Nova2General),
            ("nova-2-general", DeepgramModel::Nova2General),
            ("nova-2-meeting", DeepgramModel::Nova2Specialized),
            ("nova-2-phonecall", DeepgramModel::Nova2Specialized),
            ("nova-2-medical", DeepgramModel::Nova2Specialized),
        ];

        for (input, expected) in valid_cases {
            assert_eq!(
                input.parse::<DeepgramModel>().unwrap(),
                *expected,
                "failed for {}",
                input
            );
        }

        let invalid_cases: &[&str] = &["nova-1", "nova", "whisper", "NOVA-3"];
        for input in invalid_cases {
            assert!(
                input.parse::<DeepgramModel>().is_err(),
                "should fail for {}",
                input
            );
        }
    }

    #[test]
    fn test_nova2_exclusive_languages() {
        let nova2_only: &[&str] = &[
            "zh", "zh-CN", "zh-TW", "zh-HK", "zh-Hans", "zh-Hant", "th", "th-TH",
        ];
        for code in nova2_only {
            assert!(
                NOVA2_GENERAL_LANGUAGES.contains(code),
                "{} should be in NOVA2_GENERAL_LANGUAGES",
                code
            );
            assert!(
                !NOVA3_GENERAL_LANGUAGES.contains(code),
                "{} should NOT be in NOVA3_GENERAL_LANGUAGES",
                code
            );
        }
    }

    #[test]
    fn test_nova3_medical_exclusive_regional_variants() {
        let medical_only: &[&str] = &["en-CA", "en-IE"];
        for code in medical_only {
            assert!(
                NOVA3_MEDICAL_LANGUAGES.contains(code),
                "{} should be in NOVA3_MEDICAL_LANGUAGES",
                code
            );
            assert!(
                !NOVA3_GENERAL_LANGUAGES.contains(code),
                "{} should NOT be in NOVA3_GENERAL_LANGUAGES",
                code
            );
            assert!(
                !NOVA2_GENERAL_LANGUAGES.contains(code),
                "{} should NOT be in NOVA2_GENERAL_LANGUAGES",
                code
            );
        }
    }

    #[test]
    fn test_regional_variants_in_nova3_general() {
        let expected: &[&str] = &[
            "en-US", "en-GB", "en-AU", "en-IN", "en-NZ", "pt-BR", "pt-PT", "fr-CA", "de-CH",
            "nl-BE",
        ];
        for code in expected {
            assert!(
                NOVA3_GENERAL_LANGUAGES.contains(code),
                "{} should be in NOVA3_GENERAL_LANGUAGES",
                code
            );
        }
    }

    #[test]
    fn test_find_model() {
        let cases: &[(&[ISO639], Option<DeepgramModel>)] = &[
            (&[ISO639::En], Some(DeepgramModel::Nova3General)),
            (&[ISO639::Zh], Some(DeepgramModel::Nova2General)),
            (&[], None),
        ];

        for (iso_codes, expected) in cases {
            let langs: Vec<Language> = iso_codes.iter().map(|&iso| iso.into()).collect();
            assert_eq!(
                DeepgramAdapter::find_model(&langs),
                *expected,
                "failed for {:?}",
                iso_codes
            );
        }
    }
}
