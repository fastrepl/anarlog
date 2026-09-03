mod batch;

use super::{LanguageQuality, LanguageSupport};

pub(crate) const DEFAULT_MODEL: &str = "avalon-v1.5";
const RETIRED_V1_MODEL: &str = "avalon-v1-en";

#[derive(Clone, Default)]
pub struct AquaVoiceAdapter;

impl AquaVoiceAdapter {
    /// Avalon 1.5 is multilingual with server-side language detection, so any
    /// language set is accepted; a single configured language is pinned.
    pub fn language_support_batch(_languages: &[anlg_language::Language]) -> LanguageSupport {
        LanguageSupport::Supported {
            quality: LanguageQuality::NoData,
        }
    }

    pub fn is_supported_languages_batch(languages: &[anlg_language::Language]) -> bool {
        Self::language_support_batch(languages).is_supported()
    }

    /// `avalon-v1.5` is the only model the API accepts; Avalon v1 was retired
    /// and stored selections of it must keep working.
    pub(crate) fn resolve_model(model: Option<&str>) -> &str {
        match model {
            Some(model)
                if !model.is_empty()
                    && !crate::providers::is_meta_model(model)
                    && model != RETIRED_V1_MODEL =>
            {
                model
            }
            _ => DEFAULT_MODEL,
        }
    }
}

#[cfg(test)]
mod tests {
    use anlg_language::ISO639;

    use super::*;

    #[test]
    fn resolve_model_maps_retired_and_meta_models_to_current() {
        assert_eq!(AquaVoiceAdapter::resolve_model(None), "avalon-v1.5");
        assert_eq!(AquaVoiceAdapter::resolve_model(Some("")), "avalon-v1.5");
        assert_eq!(
            AquaVoiceAdapter::resolve_model(Some("cloud")),
            "avalon-v1.5"
        );
        assert_eq!(
            AquaVoiceAdapter::resolve_model(Some("avalon-v1-en")),
            "avalon-v1.5"
        );
        assert_eq!(
            AquaVoiceAdapter::resolve_model(Some("avalon-v1.5")),
            "avalon-v1.5"
        );
    }

    #[test]
    fn accepts_any_language_set() {
        assert!(AquaVoiceAdapter::is_supported_languages_batch(&[]));
        assert!(AquaVoiceAdapter::is_supported_languages_batch(&[
            ISO639::Ko.into(),
            ISO639::En.into()
        ]));
    }
}
