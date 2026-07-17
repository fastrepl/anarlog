use owhisper_interface::ListenParams;

use crate::adapter::deepgram_compat::{KeywordQueryStrategy, Serializer, UrlQuery};

pub struct DeepgramKeywordStrategy;

impl KeywordQueryStrategy for DeepgramKeywordStrategy {
    fn append_keyword_query<'a>(
        &self,
        query_pairs: &mut Serializer<'a, UrlQuery>,
        params: &ListenParams,
    ) {
        if params.keywords.is_empty() {
            return;
        }

        let use_keywords = params
            .model
            .as_ref()
            .map(|model| model.contains("nova-2"))
            .unwrap_or(false);

        let param_name = if use_keywords { "keywords" } else { "keyterm" };
        let max_keywords = if use_keywords {
            // https://developers.deepgram.com/docs/keywords#keyword-limits
            99
        } else {
            // https://github.com/deepgram/deepgram-python-sdk/issues/503
            50
        };

        for keyword in params.keywords.iter().take(max_keywords) {
            query_pairs.append_pair(param_name, keyword);
        }
    }
}
