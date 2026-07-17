use owhisper_interface::ListenParams;
use url::UrlQuery;
use url::form_urlencoded::Serializer;

pub trait KeywordQueryStrategy {
    fn append_keyword_query<'a>(
        &self,
        query_pairs: &mut Serializer<'a, UrlQuery>,
        params: &ListenParams,
    );
}
