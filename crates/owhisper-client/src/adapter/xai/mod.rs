mod batch;
mod live;

use crate::adapter::{LanguageQuality, LanguageSupport};

#[derive(Clone, Default)]
pub struct XaiAdapter;

impl XaiAdapter {
    pub fn language_support_live(_languages: &[anlg_language::Language]) -> LanguageSupport {
        LanguageSupport::Supported {
            quality: LanguageQuality::NoData,
        }
    }

    pub fn language_support_batch(languages: &[anlg_language::Language]) -> LanguageSupport {
        Self::language_support_live(languages)
    }
}
