use owhisper_client::{
    AdapterKind, ArgmaxAdapter, AssemblyAIAdapter, BatchSttAdapter, DeepgramAdapter,
    ElevenLabsAdapter, FireworksAdapter, GladiaAdapter, HyprnoteAdapter, MistralAdapter,
    OpenAIAdapter, PyannoteAdapter, SonioxAdapter,
};
use tracing::Instrument;

use super::{
    BatchParams, BatchRunMode, BatchRunOutput, adapter_kind_label, format_user_friendly_error,
    session_span,
};

pub(super) async fn run_direct_batch_for_adapter_kind(
    adapter_kind: AdapterKind,
    params: BatchParams,
    listen_params: owhisper_interface::ListenParams,
) -> crate::Result<BatchRunOutput> {
    match adapter_kind {
        AdapterKind::Argmax => {
            run_direct_batch::<ArgmaxAdapter>(
                adapter_kind_label(adapter_kind),
                params,
                listen_params,
            )
            .await
        }
        AdapterKind::Deepgram => {
            run_direct_batch::<DeepgramAdapter>(
                adapter_kind_label(adapter_kind),
                params,
                listen_params,
            )
            .await
        }
        AdapterKind::Soniox => {
            run_direct_batch::<SonioxAdapter>(
                adapter_kind_label(adapter_kind),
                params,
                listen_params,
            )
            .await
        }
        AdapterKind::AssemblyAI => {
            run_direct_batch::<AssemblyAIAdapter>(
                adapter_kind_label(adapter_kind),
                params,
                listen_params,
            )
            .await
        }
        AdapterKind::Fireworks => {
            run_direct_batch::<FireworksAdapter>(
                adapter_kind_label(adapter_kind),
                params,
                listen_params,
            )
            .await
        }
        AdapterKind::OpenAI => {
            run_direct_batch::<OpenAIAdapter>(
                adapter_kind_label(adapter_kind),
                params,
                listen_params,
            )
            .await
        }
        AdapterKind::Gladia => {
            run_direct_batch::<GladiaAdapter>(
                adapter_kind_label(adapter_kind),
                params,
                listen_params,
            )
            .await
        }
        AdapterKind::ElevenLabs => {
            run_direct_batch::<ElevenLabsAdapter>(
                adapter_kind_label(adapter_kind),
                params,
                listen_params,
            )
            .await
        }
        AdapterKind::Pyannote => {
            run_direct_batch::<PyannoteAdapter>(
                adapter_kind_label(adapter_kind),
                params,
                listen_params,
            )
            .await
        }
        AdapterKind::DashScope => Err(crate::BatchFailure::BatchCapabilityUnsupported {
            provider: adapter_kind_label(adapter_kind).to_string(),
        }
        .into()),
        AdapterKind::Mistral => {
            run_direct_batch::<MistralAdapter>(
                adapter_kind_label(adapter_kind),
                params,
                listen_params,
            )
            .await
        }
        AdapterKind::Hyprnote => {
            run_direct_batch::<HyprnoteAdapter>(
                adapter_kind_label(adapter_kind),
                params,
                listen_params,
            )
            .await
        }
        AdapterKind::Cactus => Err(crate::BatchFailure::DirectBatchUnsupported {
            provider: adapter_kind_label(adapter_kind).to_string(),
        }
        .into()),
    }
}

async fn run_direct_batch<A: BatchSttAdapter>(
    provider: &str,
    params: BatchParams,
    listen_params: owhisper_interface::ListenParams,
) -> crate::Result<BatchRunOutput> {
    let span = session_span(&params.session_id);

    async {
        let client = owhisper_client::BatchClient::<A>::builder()
            .api_base(params.base_url.clone())
            .api_key(params.api_key.clone())
            .params(listen_params)
            .build();

        tracing::debug!("transcribing file: {}", params.file_path);
        let response = match client.transcribe_file(&params.file_path).await {
            Ok(response) => response,
            Err(err) => {
                let raw_error = format!("{err:?}");
                let message = format_user_friendly_error(&raw_error);
                tracing::error!(
                    error = %raw_error,
                    hyprnote.error.user_message = %message,
                    "batch transcription failed"
                );
                return Err(crate::BatchFailure::DirectRequestFailed {
                    provider: provider.to_string(),
                    message,
                }
                .into());
            }
        };
        tracing::info!("batch transcription completed");

        Ok(BatchRunOutput {
            session_id: params.session_id,
            mode: BatchRunMode::Direct,
            response,
        })
    }
    .instrument(span)
    .await
}
