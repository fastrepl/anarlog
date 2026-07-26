use hypr_exa::ExaClient;
use hypr_jina::JinaClient;

use crate::config::ResearchConfig;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) exa: ExaClient,
    pub(crate) jina: JinaClient,
}

impl AppState {
    pub(crate) fn new(config: ResearchConfig) -> Self {
        let exa = ExaClient::builder()
            .api_key(config.exa_api_key)
            .build()
            .expect("failed to build Exa client");

        let jina = JinaClient::builder()
            .api_key(config.jina_api_key)
            .build()
            .expect("failed to build Jina client");

        Self { exa, jina }
    }
}
