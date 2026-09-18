#[cfg(feature = "local")]
mod batch;
mod live;

use std::sync::{Arc, Mutex};

use super::{LanguageQuality, LanguageSupport};

#[derive(Clone, Default)]
pub struct WisprFlowAdapter {
    live: Arc<Mutex<live::Session>>,
}

impl WisprFlowAdapter {
    pub fn language_support(_languages: &[anlg_language::Language]) -> LanguageSupport {
        LanguageSupport::Supported {
            quality: LanguageQuality::NoData,
        }
    }

    fn endpoint(base: &str, path: &str) -> Result<url::Url, crate::Error> {
        let base = if base.is_empty() {
            "https://platform-api.wisprflow.ai"
        } else {
            base
        };
        let mut url = url::Url::parse(base)
            .map_err(|_| crate::Error::provider_configuration("wisprflow", "Invalid API URL"))?;
        if !matches!(url.scheme(), "http" | "https" | "ws" | "wss") || url.host_str().is_none() {
            return Err(crate::Error::provider_configuration(
                "wisprflow",
                "Invalid API URL",
            ));
        }
        url.set_path(path);
        url.set_query(None);
        url.set_fragment(None);
        Ok(url)
    }
}
