use anlg_language::Language;

// Streaming and pre-recorded accept different single-language codes.
// Cantonese (`yue`) is streaming-only but has no ISO 639-1 code, so it is omitted.
pub const LIVE_LANGUAGES: &[&str] = &[
    "en", "hi", "de", "es", "ru", "it", "fr", "nl", "pt", "zh", "ja", "ko", "gu", "mr", "or", "bn",
    "ta", "te", "kn", "ml",
];

pub const BATCH_LANGUAGES: &[&str] = &[
    "en", "hi", "de", "es", "ru", "it", "fr", "nl", "pt", "uk", "pl", "cs", "sk", "lv", "et", "ro",
    "fi", "sv", "bg", "hu", "da", "lt", "mt", "zh", "ja", "ko",
];

const MULTI_ASIAN: &[&str] = &["en", "zh", "ja", "ko"];
const INDIC: &[&str] = &["en", "hi", "gu", "mr", "bn", "or"];

const LIVE_AGGREGATORS: &[(&str, &[&str])] = &[
    ("north_indic", INDIC),
    ("multi-asian", MULTI_ASIAN),
    ("multi-south-indic", &["en", "ta", "te", "kn", "ml"]),
];

const BATCH_AGGREGATORS: &[(&str, &[&str])] = &[
    (
        "multi-eu",
        &[
            "en", "de", "es", "ru", "it", "fr", "nl", "pt", "uk", "pl", "cs", "sk", "lv", "et",
            "ro", "fi", "sv", "bg", "hu", "da", "lt", "mt",
        ],
    ),
    ("multi-asian", MULTI_ASIAN),
    ("multi-indic", INDIC),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Live,
    Batch,
}

impl Mode {
    fn single_codes(self) -> &'static [&'static str] {
        match self {
            Self::Live => LIVE_LANGUAGES,
            Self::Batch => BATCH_LANGUAGES,
        }
    }

    fn aggregators(self) -> &'static [(&'static str, &'static [&'static str])] {
        match self {
            Self::Live => LIVE_AGGREGATORS,
            Self::Batch => BATCH_AGGREGATORS,
        }
    }
}

/// Pulse pins one language per session and handles English code-switching on
/// its own, so `en` plus one other language collapses to that language. Any
/// other multi-language set only works when a regional aggregator covers it.
pub(super) fn query_value(mode: Mode, languages: &[Language]) -> Option<String> {
    let mut codes: Vec<&str> = Vec::new();
    for language in languages {
        let code = language.iso639().code();
        if !codes.contains(&code) {
            codes.push(code);
        }
    }

    let mut non_english = codes.iter().copied().filter(|code| *code != "en");
    match (non_english.next(), non_english.next()) {
        (None, _) => Some("en".to_string()),
        (Some(code), None) => mode
            .single_codes()
            .contains(&code)
            .then(|| code.to_string()),
        (Some(_), Some(_)) => mode
            .aggregators()
            .iter()
            .find(|(_, set)| codes.iter().all(|code| set.contains(code)))
            .map(|(name, _)| (*name).to_string()),
    }
}

#[cfg(test)]
mod tests {
    use anlg_language::ISO639;

    use super::*;

    fn langs(codes: &[ISO639]) -> Vec<Language> {
        codes.iter().map(|code| (*code).into()).collect()
    }

    #[test]
    fn empty_defaults_to_english() {
        assert_eq!(query_value(Mode::Live, &[]), Some("en".to_string()));
        assert_eq!(query_value(Mode::Batch, &[]), Some("en".to_string()));
    }

    #[test]
    fn single_language_uses_its_code() {
        assert_eq!(
            query_value(Mode::Live, &langs(&[ISO639::Hi])),
            Some("hi".to_string())
        );
        assert_eq!(
            query_value(Mode::Batch, &langs(&[ISO639::Pl])),
            Some("pl".to_string())
        );
    }

    #[test]
    fn english_code_switching_collapses_to_other_language() {
        assert_eq!(
            query_value(Mode::Live, &langs(&[ISO639::En, ISO639::Hi])),
            Some("hi".to_string())
        );
        assert_eq!(
            query_value(Mode::Batch, &langs(&[ISO639::Es, ISO639::En])),
            Some("es".to_string())
        );
    }

    #[test]
    fn multiple_languages_use_a_covering_aggregator() {
        assert_eq!(
            query_value(Mode::Batch, &langs(&[ISO639::De, ISO639::Fr, ISO639::En])),
            Some("multi-eu".to_string())
        );
        assert_eq!(
            query_value(Mode::Live, &langs(&[ISO639::Ja, ISO639::Ko])),
            Some("multi-asian".to_string())
        );
        assert_eq!(
            query_value(Mode::Live, &langs(&[ISO639::Hi, ISO639::Gu])),
            Some("north_indic".to_string())
        );
    }

    #[test]
    fn rejects_languages_outside_the_mode() {
        assert_eq!(query_value(Mode::Live, &langs(&[ISO639::Pl])), None);
        assert_eq!(query_value(Mode::Batch, &langs(&[ISO639::Gu])), None);
        assert_eq!(
            query_value(Mode::Batch, &langs(&[ISO639::De, ISO639::Ja])),
            None
        );
    }

    #[test]
    fn regional_variants_map_to_base_code() {
        let language = Language::with_region(ISO639::En, "US");
        assert_eq!(query_value(Mode::Live, &[language]), Some("en".to_string()));
    }
}
