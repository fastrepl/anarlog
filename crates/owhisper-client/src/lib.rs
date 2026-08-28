mod adapter;
mod batch;
mod error;
mod error_detection;
mod http_client;
mod live;
#[cfg(feature = "local")]
mod local_apple_speech_live;
#[cfg(feature = "local")]
mod local_soniqo_live;
pub(crate) mod polling;
mod providers;

#[cfg(test)]
pub(crate) mod test_utils;

pub use error_detection::ProviderError;
use owhisper_interface::ListenParams;
pub use providers::{Auth, Provider, is_meta_model};

#[cfg(feature = "local")]
pub use adapter::StreamingBatchConfig;
pub use adapter::deepgram::DeepgramModel;
pub use adapter::{
    AdapterKind, AnarlogAdapter, AquaVoiceAdapter, ArgmaxAdapter, AssemblyAIAdapter,
    AwsTranscribeAdapter, AzureSpeechAdapter, BatchSttAdapter, BatchUploadLimit, CallbackResult,
    CallbackSttAdapter, CartesiaAdapter, CohereAdapter, DashScopeAdapter, DeepgramAdapter,
    DeepgramFluxAdapter, ElevenLabsAdapter, FireworksAdapter, GladiaAdapter, GoogleCloudAdapter,
    GoogleGenerativeAiAdapter, GroqAdapter, LanguageQuality, LanguageSupport, MistralAdapter,
    OpenAIAdapter, OpenRouterAdapter, PyannoteAdapter, RealtimeSttAdapter, RevAiAdapter,
    SiliconFlowAdapter, SmallestAIAdapter, SonioxAdapter, SpeechmaticsAdapter, TogetherAdapter,
    WhisperCppAdapter, XaiAdapter, ZaiAdapter, append_provider_param,
    documented_language_codes_batch, documented_language_codes_live, is_anarlog_proxy,
    is_local_host, normalize_languages,
};
pub use adapter::{StreamingBatchEvent, StreamingBatchStream};

pub use anlg_ws_client;
pub use batch::{BatchClient, BatchClientBuilder};
pub use error::Error;
pub use live::{
    DualHandle, FinalizeHandle, ListenClient, ListenClientBuilder, ListenClientDual,
    ListenClientDualInput, ListenClientInput,
};
#[cfg(feature = "local")]
pub use local_apple_speech_live::{
    LocalAppleSpeechLiveClient, LocalAppleSpeechLiveError, LocalAppleSpeechLiveHandle,
    LocalAppleSpeechLiveStream,
};
#[cfg(feature = "local")]
pub use local_soniqo_live::{
    LocalSoniqoLiveClient, LocalSoniqoLiveError, LocalSoniqoLiveHandle, LocalSoniqoLiveStream,
};

pub fn normalize_listen_params(mut params: ListenParams) -> ListenParams {
    params.languages = adapter::normalize_languages(&params.languages);
    params
}
