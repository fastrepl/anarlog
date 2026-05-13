#[cfg(feature = "local")]
mod batch;
mod live;

#[cfg(feature = "local")]
mod retry;

#[derive(Clone, Default)]
pub struct CactusAdapter;

impl CactusAdapter {
    pub fn is_supported_languages_live(
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> bool {
        let _ = (languages, model);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::CactusAdapter;
    use hypr_language::ISO639;

    #[test]
    fn rejects_live_for_parakeet_models() {
        assert!(!CactusAdapter::is_supported_languages_live(
            &[],
            Some("cactus-parakeet-tdt-0.6b-v3-int8"),
        ));

        assert!(!CactusAdapter::is_supported_languages_live(
            &[ISO639::En.into()],
            Some("cactus-parakeet-tdt-0.6b-v3-int8"),
        ));

        assert!(!CactusAdapter::is_supported_languages_live(
            &[ISO639::De.into()],
            Some("cactus-parakeet-tdt-0.6b-v3-int8"),
        ));
    }

    #[test]
    fn rejects_live_for_non_parakeet_models() {
        assert!(!CactusAdapter::is_supported_languages_live(
            &[],
            Some("cactus-whisper-small-int8"),
        ));

        assert!(!CactusAdapter::is_supported_languages_live(&[], None));
    }

    #[test]
    fn rejects_live_for_unsupported_parakeet_languages() {
        assert!(!CactusAdapter::is_supported_languages_live(
            &[ISO639::Ko.into()],
            Some("cactus-parakeet-tdt-0.6b-v3-int8"),
        ));
    }
}
