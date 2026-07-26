use owhisper_interface::ListenParams;
use url::UrlQuery;
use url::form_urlencoded::Serializer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptionMode {
    Live,
    Batch,
}

pub trait LanguageQueryStrategy {
    fn append_language_query<'a>(
        &self,
        query_pairs: &mut Serializer<'a, UrlQuery>,
        params: &ListenParams,
        mode: TranscriptionMode,
    );
}
