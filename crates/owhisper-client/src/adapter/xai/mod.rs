mod batch;
mod live;

use std::sync::Arc;
use std::sync::atomic::AtomicU8;

use crate::adapter::{LanguageQuality, LanguageSupport};

#[derive(Clone)]
pub struct XaiAdapter {
    live_channels: Arc<AtomicU8>,
}

impl Default for XaiAdapter {
    fn default() -> Self {
        Self {
            live_channels: Arc::new(AtomicU8::new(1)),
        }
    }
}

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
