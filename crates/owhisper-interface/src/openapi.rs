use utoipa::OpenApi;

#[derive(utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CommonListenParams {
    /// STT provider. Use 'hyprnote' for automatic routing (default), or specify:
    /// deepgram, soniox, assemblyai, gladia, elevenlabs, fireworks, openai, dashscope, mistral
    #[allow(dead_code)]
    provider: Option<String>,
    /// Model to use for transcription (provider-specific, e.g. 'nova-3')
    #[allow(dead_code)]
    model: Option<String>,
    /// BCP-47 language hint (e.g. 'en', 'ko', 'ja'). Multiple values or comma-separated supported
    #[allow(dead_code)]
    language: Option<String>,
    /// Keyword boosting. Comma-separated or repeated query params
    #[allow(dead_code)]
    keywords: Option<String>,
}

#[derive(utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct StreamListenParams {
    /// Audio sample rate in Hz (default: 16000)
    #[allow(dead_code)]
    sample_rate: Option<u32>,
    /// Number of audio channels (default: 1)
    #[allow(dead_code)]
    channels: Option<u8>,
    /// Audio encoding: linear16, flac, mulaw, opus, ogg-opus, etc.
    #[allow(dead_code)]
    encoding: Option<String>,
}

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::batch::Response,
    crate::batch::Results,
    crate::batch::Channel,
    crate::batch::Alternatives,
    crate::batch::Word,
    crate::stream::StreamResponse,
    crate::stream::Channel,
    crate::stream::Alternatives,
    crate::stream::Word,
    crate::stream::Metadata,
    crate::stream::ModelInfo,
)))]
struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
