use owhisper_interface::ListenParams;

use crate::adapter::deepgram::{DeepgramAdapter, DeepgramModel};
use crate::adapter::deepgram_compat::{
    LanguageQueryStrategy, Serializer, TranscriptionMode, UrlQuery,
};

const NOVA2_MULTI_LANGS: &[&str] = &["en", "es"];
const NOVA3_MULTI_LANGS: &[&str] = &["en", "es", "fr", "de", "hi", "ru", "pt", "ja", "it", "nl"];
const LANGUAGE_DETECTION_LANGS: &[&str] = &[
    "bg", "ca", "cs", "da", "de", "el", "en", "es", "et", "fi", "fr", "hi", "hu", "id", "it", "ja",
    "ko", "lt", "lv", "ms", "nl", "no", "pl", "pt", "ro", "ru", "sk", "sv", "th", "tr", "uk", "vi",
    "zh",
];

pub fn can_use_multi(model: &str, languages: &[hypr_language::Language]) -> bool {
    if languages.len() < 2 {
        return false;
    }

    let multi_langs: &[&str] = if model.contains("nova-3") {
        NOVA3_MULTI_LANGS
    } else if model.contains("nova-2") {
        NOVA2_MULTI_LANGS
    } else {
        return false;
    };

    languages
        .iter()
        .all(|lang| multi_langs.contains(&lang.iso639().code()))
}

pub struct DeepgramLanguageStrategy;

impl LanguageQueryStrategy for DeepgramLanguageStrategy {
    fn append_language_query<'a>(
        &self,
        query_pairs: &mut Serializer<'a, UrlQuery>,
        params: &ListenParams,
        mode: TranscriptionMode,
    ) {
        let model = params.model.as_deref().unwrap_or("");

        match params.languages.len() {
            0 => {
                if mode == TranscriptionMode::Batch {
                    query_pairs.append_pair("detect_language", "true");
                } else {
                    query_pairs.append_pair("language", "en");
                }
            }
            1 => {
                if let Some(language) = params.languages.first() {
                    let code = single_language_query_code(params, language);
                    query_pairs.append_pair("language", &code);
                }
            }
            _ => {
                if can_use_multi(model, &params.languages) {
                    query_pairs.append_pair("language", "multi");
                } else if mode == TranscriptionMode::Batch {
                    append_detect_language_query(query_pairs, &params.languages);
                } else if let Some(language) = params.languages.first() {
                    let code = single_language_query_code(params, language);
                    query_pairs.append_pair("language", &code);
                }
            }
        }
    }
}

fn single_language_query_code(params: &ListenParams, language: &hypr_language::Language) -> String {
    let Some(region) = language.region() else {
        return language.iso639().code().to_string();
    };

    let Some(model) = effective_model(params) else {
        return language.iso639().code().to_string();
    };

    let regional = format!("{}-{region}", language.iso639().code());
    if model.supported_languages().contains(&regional.as_str()) {
        regional
    } else {
        language.iso639().code().to_string()
    }
}

fn append_detect_language_query<'a>(
    query_pairs: &mut Serializer<'a, UrlQuery>,
    languages: &[hypr_language::Language],
) {
    if languages.iter().all(supports_language_detection) {
        for language in languages {
            query_pairs.append_pair("detect_language", language.iso639().code());
        }
    } else {
        query_pairs.append_pair("detect_language", "true");
    }
}

pub(super) fn supports_language_detection(language: &hypr_language::Language) -> bool {
    LANGUAGE_DETECTION_LANGS.contains(&language.iso639().code())
}

fn effective_model(params: &ListenParams) -> Option<DeepgramModel> {
    params
        .model
        .as_deref()
        .and_then(|model| model.parse::<DeepgramModel>().ok())
        .or_else(|| DeepgramAdapter::find_model(&params.languages))
}
