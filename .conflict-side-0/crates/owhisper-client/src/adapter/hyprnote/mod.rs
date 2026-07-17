mod batch;
mod live;

use super::{LanguageQuality, LanguageSupport};

#[derive(Clone, Default)]
pub struct HyprnoteAdapter;

impl HyprnoteAdapter {
    pub fn language_support_live(
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> LanguageSupport {
        match soniqo_language_support(languages, model, true) {
            Some(support) => support,
            None => LanguageSupport::Supported {
                quality: LanguageQuality::NoData,
            },
        }
    }

    pub fn is_supported_languages_live(
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> bool {
        Self::language_support_live(languages, model).is_supported()
    }

    pub fn language_support_batch(
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> LanguageSupport {
        match soniqo_language_support(languages, model, false) {
            Some(support) => support,
            None => LanguageSupport::Supported {
                quality: LanguageQuality::NoData,
            },
        }
    }

    pub fn is_supported_languages_batch(
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> bool {
        Self::language_support_batch(languages, model).is_supported()
    }
}

fn soniqo_language_support(
    languages: &[hypr_language::Language],
    model: Option<&str>,
    live: bool,
) -> Option<LanguageSupport> {
    let model = model?;

    match model {
        "soniqo-parakeet-streaming" => Some(parakeet_language_support(languages)),
        "soniqo-parakeet-batch" => Some(if live {
            LanguageSupport::NotSupported
        } else {
            parakeet_language_support(languages)
        }),
        model if model.starts_with("soniqo-") => Some(if live {
            LanguageSupport::NotSupported
        } else {
            LanguageSupport::Supported {
                quality: LanguageQuality::NoData,
            }
        }),
        _ => None,
    }
}

fn parakeet_language_support(languages: &[hypr_language::Language]) -> LanguageSupport {
    if languages
        .iter()
        .all(hypr_language::is_parakeet_tdt_v3_language)
    {
        LanguageSupport::Supported {
            quality: LanguageQuality::NoData,
        }
    } else {
        LanguageSupport::NotSupported
    }
}
