mod actor;
mod bootstrap;

use std::sync::Arc;

use owhisper_client::{AdapterKind, OpenAIAdapter};

use crate::BatchRuntime;

use super::{BatchParams, BatchRunOutput};

#[derive(Debug, Clone, Copy, strum::IntoStaticStr)]
pub(super) enum ProgressiveProvider {
    #[strum(serialize = "argmax")]
    Argmax,
    #[strum(serialize = "cactus")]
    Cactus,
    #[strum(serialize = "openai")]
    OpenAI,
    #[strum(serialize = "whispercpp")]
    WhisperCpp,
}

impl ProgressiveProvider {
    pub(super) fn label(self) -> &'static str {
        self.into()
    }
}

pub(super) async fn run_progressive_batch_session(
    runtime: Arc<dyn BatchRuntime>,
    params: BatchParams,
    listen_params: owhisper_interface::ListenParams,
) -> crate::Result<BatchRunOutput> {
    let progressive_provider = resolve_progressive_provider(&params, &listen_params)?;
    actor::run_progressive_batch(runtime, params, listen_params, progressive_provider).await
}

fn resolve_progressive_provider(
    params: &BatchParams,
    listen_params: &owhisper_interface::ListenParams,
) -> crate::Result<ProgressiveProvider> {
    match params.provider {
        super::BatchProvider::WhisperLocal => return Ok(ProgressiveProvider::WhisperCpp),
        super::BatchProvider::Cactus => return Ok(ProgressiveProvider::Cactus),
        super::BatchProvider::OpenAI
            if OpenAIAdapter::supports_progressive_batch_model(listen_params.model.as_deref()) =>
        {
            return Ok(ProgressiveProvider::OpenAI);
        }
        _ => {}
    }

    let adapter_kind = AdapterKind::from_url_and_languages(
        &params.base_url,
        &listen_params.languages,
        listen_params.model.as_deref(),
    );

    match adapter_kind {
        AdapterKind::Argmax => Ok(ProgressiveProvider::Argmax),
        AdapterKind::Cactus => Ok(ProgressiveProvider::Cactus),
        AdapterKind::OpenAI
            if OpenAIAdapter::supports_progressive_batch_model(listen_params.model.as_deref()) =>
        {
            Ok(ProgressiveProvider::OpenAI)
        }
        AdapterKind::DashScope => Err(crate::BatchFailure::BatchCapabilityUnsupported {
            provider: adapter_kind.to_string(),
        }
        .into()),
        _ => Err(crate::BatchFailure::ProgressiveBatchUnsupported {
            provider: adapter_kind.to_string(),
        }
        .into()),
    }
}
