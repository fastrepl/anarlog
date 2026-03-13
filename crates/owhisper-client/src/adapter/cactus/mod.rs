#[cfg(feature = "argmax")]
mod batch;
mod live;

#[derive(Clone, Default)]
pub struct CactusAdapter;

impl CactusAdapter {
    pub fn is_supported_languages_live(
        _languages: &[hypr_language::Language],
        _model: Option<&str>,
    ) -> bool {
        true
    }
}
