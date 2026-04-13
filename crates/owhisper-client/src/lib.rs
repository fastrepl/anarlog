mod adapter;
mod batch;
mod error;
mod error_detection;
mod http_client;
mod live;
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
    AdapterKind, AquaVoiceAdapter, ArgmaxAdapter, AssemblyAIAdapter, BatchSttAdapter,
    CactusAdapter, CallbackResult, CallbackSttAdapter, DashScopeAdapter, DeepgramAdapter,
    ElevenLabsAdapter, FireworksAdapter, GladiaAdapter, HyprnoteAdapter, LanguageQuality,
    LanguageSupport, MistralAdapter, OpenAIAdapter, PyannoteAdapter, RealtimeSttAdapter,
    SonioxAdapter, WhisperCppAdapter, append_provider_param, documented_language_codes_batch,
    documented_language_codes_live, is_hyprnote_proxy, is_local_host, normalize_languages,
};
pub use adapter::{StreamingBatchEvent, StreamingBatchStream};

pub use batch::{BatchClient, BatchClientBuilder};
pub use error::Error;
pub use hypr_ws_client;
pub use live::{DualHandle, FinalizeHandle, ListenClient, ListenClientBuilder, ListenClientDual};

pub fn normalize_listen_params(mut params: ListenParams) -> ListenParams {
    params.languages = adapter::normalize_languages(&params.languages);
    params
}
