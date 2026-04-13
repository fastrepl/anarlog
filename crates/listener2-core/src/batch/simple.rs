use owhisper_client::{
    AdapterKind, AquaVoiceAdapter, ArgmaxAdapter, AssemblyAIAdapter, BatchSttAdapter,
    DeepgramAdapter, ElevenLabsAdapter, FireworksAdapter, GladiaAdapter, HyprnoteAdapter,
    MistralAdapter, OpenAIAdapter, PyannoteAdapter, SonioxAdapter,
};
use tracing::Instrument;

use super::{BatchParams, BatchRunMode, BatchRunOutput, format_user_friendly_error, session_span};

macro_rules! dispatch_batch {
    ($ak:expr, $params:expr, $lp:expr,
     { $($var:ident => $adapter:ty),+ $(,)? },
     unsupported: [$($unsup:ident),* $(,)?]
    ) => {
        match $ak {
            $(AdapterKind::$var => {
                run_direct_batch::<$adapter>(&AdapterKind::$var.to_string(), $params, $lp).await
            })+
            $(AdapterKind::$unsup => {
                Err(crate::BatchFailure::DirectBatchUnsupported {
                    provider: AdapterKind::$unsup.to_string(),
                }.into())
            })*
        }
    };
}

pub(super) async fn run_direct_batch_for_adapter_kind(
    adapter_kind: AdapterKind,
    params: BatchParams,
    listen_params: owhisper_interface::ListenParams,
) -> crate::Result<BatchRunOutput> {
    dispatch_batch!(adapter_kind, params, listen_params, {
        Argmax => ArgmaxAdapter,
        Deepgram => DeepgramAdapter,
        Soniox => SonioxAdapter,
        AssemblyAI => AssemblyAIAdapter,
        Fireworks => FireworksAdapter,
        OpenAI => OpenAIAdapter,
        Gladia => GladiaAdapter,
        ElevenLabs => ElevenLabsAdapter,
        Pyannote => PyannoteAdapter,
        Mistral => MistralAdapter,
        Hyprnote => HyprnoteAdapter,
        AquaVoice => AquaVoiceAdapter,
    }, unsupported: [DashScope, Cactus])
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
