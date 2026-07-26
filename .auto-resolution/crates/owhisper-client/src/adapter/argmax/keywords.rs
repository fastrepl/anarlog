use owhisper_interface::ListenParams;

use crate::adapter::deepgram_compat::{KeywordQueryStrategy, Serializer, UrlQuery};

pub struct ArgmaxKeywordStrategy;

impl KeywordQueryStrategy for ArgmaxKeywordStrategy {
    fn append_keyword_query<'a>(
        &self,
        query_pairs: &mut Serializer<'a, UrlQuery>,
        params: &ListenParams,
    ) {
        if params.keywords.is_empty() {
            return;
        }

        for keyword in &params.keywords {
            query_pairs.append_pair("keyterm", keyword);
        }
    }
}
