mod keywords;
mod language;

pub use keywords::KeywordQueryStrategy;
pub use language::{LanguageQueryStrategy, TranscriptionMode};

pub use url::UrlQuery;
pub use url::form_urlencoded::Serializer;

use owhisper_interface::ListenParams;

use super::url_builder::{QueryParamBuilder, resolve_model_for_languages};

pub fn listen_endpoint_url(api_base: &str) -> (url::Url, Vec<(String, String)>) {
    let mut url: url::Url = match api_base.parse() {
        Ok(url) => url,
        Err(_error) => {
            tracing::warn!(
                error.type = "invalid_api_base",
                "deepgram_invalid_api_base_using_default"
            );
            crate::providers::Provider::Deepgram
                .default_api_base()
                .parse()
                .expect("invalid_default_api_base")
        }
    };
    let existing_params = super::extract_query_params(&url);
    url.set_query(None);
    super::append_path_if_missing(&mut url, "/listen");
    (url, existing_params)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_listen_endpoint_url() {
        let cases = [
            (
                "https://api.deepgram.com/v1",
                "https://api.deepgram.com/v1/listen",
                vec![],
            ),
            (
                "https://api.anarlog.so/v1?provider=deepgram",
                "https://api.anarlog.so/v1/listen",
                vec![("provider", "deepgram")],
            ),
            (
                "https://api.anarlog.so/listen?provider=deepgram",
                "https://api.anarlog.so/listen",
                vec![("provider", "deepgram")],
            ),
            (
                "https://api.anarlog.so/listen/",
                "https://api.anarlog.so/listen/",
                vec![],
            ),
            ("12", "https://api.deepgram.com/v1/listen", vec![]),
        ];

        for (input, expected_url, expected_params) in cases {
            let (url, params) = listen_endpoint_url(input);
            assert_eq!(url.as_str(), expected_url, "input: {input}");
            assert_eq!(
                params,
                expected_params
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect::<Vec<_>>(),
                "input: {input}"
            );
        }
    }
}

pub fn build_listen_ws_url<L, K>(
    api_base: &str,
    params: &ListenParams,
    channels: u8,
    lang_strategy: &L,
    keyword_strategy: &K,
) -> url::Url
where
    L: LanguageQueryStrategy,
    K: KeywordQueryStrategy,
{
    let (mut url, existing_params) = listen_endpoint_url(api_base);

    let mut builder = QueryParamBuilder::new();
    for (key, value) in &existing_params {
        builder.add(key, value);
    }

    builder
        .add_common_listen_params(params, channels)
        .add_bool("interim_results", true)
        .add_bool("multichannel", channels > 1)
        .add_bool("vad_events", false);

    if let Some(custom) = &params.custom_query {
        for (key, value) in custom {
            builder.add(key, value);
        }
    }

    builder.apply_to(&mut url);

    {
        let mut query_pairs = url.query_pairs_mut();
        lang_strategy.append_language_query(&mut query_pairs, params, TranscriptionMode::Live);
        keyword_strategy.append_keyword_query(&mut query_pairs, params);
    }

    super::set_scheme_from_host(&mut url);

    url
}

pub fn build_batch_url<L, K>(
    api_base: &str,
    params: &ListenParams,
    lang_strategy: &L,
    keyword_strategy: &K,
) -> url::Url
where
    L: LanguageQueryStrategy,
    K: KeywordQueryStrategy,
{
    use crate::providers::Provider;

    let (mut url, existing_params) = listen_endpoint_url(api_base);

    let mut builder = QueryParamBuilder::new();
    for (key, value) in &existing_params {
        builder.add(key, value);
    }

    let model = resolve_model_for_languages(
        params.model.as_deref(),
        &params.languages,
        Provider::Deepgram.default_batch_model(),
    );
    builder
        .add("model", model)
        .add_bool("diarize", true)
        .add_bool("multichannel", params.channels > 1)
        .add_bool("punctuate", true)
        .add_bool("smart_format", true)
        .add_bool("utterances", true)
        .add_bool("numerals", true)
        .add_bool("filler_words", false)
        .add_bool("dictation", false)
        .add_bool("paragraphs", false)
        .add_bool("profanity_filter", false)
        .add_bool("measurements", false)
        .add_bool("topics", false)
        .add_bool("sentiment", false)
        .add_bool("intents", false)
        .add_bool("detect_entities", false)
        .add_bool("mip_opt_out", true);

    builder.apply_to(&mut url);

    {
        let mut query_pairs = url.query_pairs_mut();
        lang_strategy.append_language_query(&mut query_pairs, params, TranscriptionMode::Batch);
        keyword_strategy.append_keyword_query(&mut query_pairs, params);
    }

    url
}
