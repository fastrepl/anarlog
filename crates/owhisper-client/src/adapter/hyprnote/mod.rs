mod batch;
mod live;

#[derive(Clone, Default)]
pub struct HyprnoteAdapter;

impl HyprnoteAdapter {
    pub fn is_supported_languages_live(
        _languages: &[hypr_language::Language],
        _model: Option<&str>,
    ) -> bool {
        true
    }

    pub fn is_supported_languages_batch(
        _languages: &[hypr_language::Language],
        _model: Option<&str>,
    ) -> bool {
        true
    }
}
