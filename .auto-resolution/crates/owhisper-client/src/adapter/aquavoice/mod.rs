mod batch;

use super::{LanguageQuality, LanguageSupport};

#[derive(Clone, Default)]
pub struct AquaVoiceAdapter;

impl AquaVoiceAdapter {
    pub fn language_support_batch(languages: &[hypr_language::Language]) -> LanguageSupport {
        if languages
            .iter()
            .all(|l| l.iso639() == hypr_language::ISO639::En)
        {
            LanguageSupport::Supported {
                quality: LanguageQuality::NoData,
            }
        } else {
            LanguageSupport::NotSupported
        }
    }

    pub fn is_supported_languages_batch(languages: &[hypr_language::Language]) -> bool {
        Self::language_support_batch(languages).is_supported()
    }
}
