use std::path::PathBuf;
use std::sync::Arc;

use futures_util::StreamExt;
use owhisper_client::{ListenClient, RealtimeSttAdapter};
use owhisper_interface::stream::StreamResponse;
use tokio::sync::mpsc;

use super::audio::{
    ActualAudio, AudioProvider, AudioSource, ChannelKind, DEFAULT_SAMPLE_RATE,
    DEFAULT_TIMEOUT_SECS, DisplayMode, create_dual_audio_stream, create_single_audio_stream,
};
use super::server::spawn_router;
use super::tracing::{TracingCapture, init_capture};
use crate::cli::Provider;
pub use crate::cli::{DebugProvider, TranscribeArgs};
#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
use crate::config::stt::resolve_local_model_path;
use crate::config::stt::{ResolvedSttConfig, resolve_config};
use crate::error::{CliError, CliResult};

pub(crate) enum RuntimeEvent {
    StreamResponse {
        response: StreamResponse,
        display_mode: DisplayMode,
    },
    StreamEnded,
    Failed(String),
}

pub(crate) struct Runtime {
    task: tokio::task::JoinHandle<()>,
    tracing: Arc<TracingCapture>,
}

impl Runtime {
    pub(crate) async fn start(
        args: TranscribeArgs,
        tx: mpsc::UnboundedSender<RuntimeEvent>,
    ) -> CliResult<Self> {
        let tracing = TracingCapture::new();
        init_capture(Arc::clone(&tracing));

        validate_args(&args)?;

        let task = tokio::spawn(async move {
            if let Err(error) = run(args, tx.clone()).await {
                let _ = tx.send(RuntimeEvent::Failed(error.to_string()));
            }
        });

        Ok(Self { task, tracing })
    }

    pub(crate) fn tracing_capture(&self) -> Arc<TracingCapture> {
        Arc::clone(&self.tracing)
    }

    pub(crate) fn abort(&self) {
        self.task.abort();
    }
}

fn validate_args(args: &TranscribeArgs) -> CliResult<()> {
    if let Some(ref model_path) = args.model_path
        && !is_local_provider(&args.provider)
    {
        return Err(CliError::invalid_argument_with_hint(
            "--model-path",
            model_path.display().to_string(),
            "only valid with local providers (cactus)",
            "Use --provider cactus for local model files, or remove --model-path for cloud providers.",
        ));
    }

    Ok(())
}

fn is_local_provider(provider: &DebugProvider) -> bool {
    match provider {
        #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
        DebugProvider::Cactus => true,
        _ => false,
    }
}

async fn run(args: TranscribeArgs, tx: mpsc::UnboundedSender<RuntimeEvent>) -> CliResult<()> {
    match args.provider.clone() {
        DebugProvider::Deepgram => {
            let model = require_model_name(args.model.as_deref(), &args.provider)?;
            let resolved = resolve_standard_provider(
                args.provider.clone(),
                args.deepgram_api_key,
                Some(model),
            )
            .await?;
            run_resolved_provider::<owhisper_client::DeepgramAdapter>(
                &resolved,
                args.audio.audio,
                tx,
            )
            .await?;
        }
        DebugProvider::Soniox => {
            let model = require_model_name(args.model.as_deref(), &args.provider)?;
            let resolved =
                resolve_standard_provider(args.provider.clone(), args.soniox_api_key, Some(model))
                    .await?;
            run_resolved_provider::<owhisper_client::SonioxAdapter>(
                &resolved,
                args.audio.audio,
                tx,
            )
            .await?;
        }
        #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
        DebugProvider::Cactus => {
            if args.model_path.is_some() {
                let model_path = resolve_local_model_path(args.model.as_deref(), args.model_path)?;
                run_cactus_from_path(model_path, args.audio.audio, tx).await?;
            } else {
                let resolved =
                    resolve_standard_provider(args.provider.clone(), None, args.model).await?;
                run_resolved_provider::<owhisper_client::CactusAdapter>(
                    &resolved,
                    args.audio.audio,
                    tx,
                )
                .await?;
            }
        }
        DebugProvider::ProxyHyprnote => {
            run_proxy(
                ProxyKind::Hyprnote,
                args.deepgram_api_key,
                args.soniox_api_key,
                args.audio.audio,
                tx,
            )
            .await?;
        }
        DebugProvider::ProxyDeepgram => {
            let api_key = require_key(args.deepgram_api_key, "DEEPGRAM_API_KEY")?;
            run_proxy(
                ProxyKind::Deepgram,
                Some(api_key),
                None,
                args.audio.audio,
                tx,
            )
            .await?;
        }
        DebugProvider::ProxySoniox => {
            let api_key = require_key(args.soniox_api_key, "SONIOX_API_KEY")?;
            run_proxy(ProxyKind::Soniox, None, Some(api_key), args.audio.audio, tx).await?;
        }
    }

    Ok(())
}

fn default_listen_params() -> owhisper_interface::ListenParams {
    owhisper_interface::ListenParams {
        sample_rate: DEFAULT_SAMPLE_RATE,
        languages: vec![hypr_language::ISO639::En.into()],
        ..Default::default()
    }
}

fn build_client_builder<A: RealtimeSttAdapter>(
    api_base: impl Into<String>,
    api_key: Option<String>,
    params: owhisper_interface::ListenParams,
) -> owhisper_client::ListenClientBuilder<A> {
    let mut builder = ListenClient::builder()
        .adapter::<A>()
        .api_base(api_base.into())
        .params(params);

    if let Some(api_key) = api_key {
        builder = builder.api_key(api_key);
    }

    builder
}

async fn run_resolved_provider<A: RealtimeSttAdapter>(
    resolved: &ResolvedSttConfig,
    source: AudioSource,
    tx: mpsc::UnboundedSender<RuntimeEvent>,
) -> CliResult<()> {
    // keep local server alive for the duration of this scope
    let _ = resolved.server.as_ref();
    let audio: Arc<dyn AudioProvider> = create_audio_provider(&source);
    let mut params = default_listen_params();
    params.languages = vec![resolved.language.clone()];
    params.model = resolved.model_option();
    let api_key = if resolved.api_key.is_empty() {
        None
    } else {
        Some(resolved.api_key.clone())
    };

    run_for_source::<A>(audio, source, &resolved.base_url, api_key, params, tx).await
}

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
async fn run_cactus_from_path(
    model_path: PathBuf,
    source: AudioSource,
    tx: mpsc::UnboundedSender<RuntimeEvent>,
) -> CliResult<()> {
    let server = hypr_local_stt_server::LocalSttServer::start(model_path)
        .await
        .map_err(|e| CliError::operation_failed("start local cactus server", e.to_string()))?;
    let base_url = server.base_url().to_string();
    let audio: Arc<dyn AudioProvider> = create_audio_provider(&source);

    run_for_source::<owhisper_client::CactusAdapter>(
        audio,
        source,
        &base_url,
        None,
        default_listen_params(),
        tx,
    )
    .await?;

    drop(server);
    Ok(())
}

async fn run_for_source<A: RealtimeSttAdapter>(
    audio: Arc<dyn AudioProvider>,
    source: AudioSource,
    api_base: impl Into<String>,
    api_key: Option<String>,
    params: owhisper_interface::ListenParams,
    tx: mpsc::UnboundedSender<RuntimeEvent>,
) -> CliResult<()> {
    let builder = build_client_builder::<A>(api_base, api_key, params);

    if source.is_dual() {
        let client = builder.build_dual().await;
        let audio_stream = create_dual_audio_stream(&audio, &source, DEFAULT_SAMPLE_RATE)?;
        let (response_stream, handle) =
            client
                .from_realtime_audio(audio_stream)
                .await
                .map_err(|e| {
                    CliError::operation_failed("connect realtime transcription", e.to_string())
                })?;
        forward_stream(
            response_stream,
            handle,
            DEFAULT_TIMEOUT_SECS,
            DisplayMode::Dual,
            tx,
        )
        .await
    } else {
        let kind = match source {
            AudioSource::Input => ChannelKind::Mic,
            AudioSource::Output => ChannelKind::Speaker,
            AudioSource::Mock => ChannelKind::Mic,
            AudioSource::RawDual | AudioSource::AecDual => unreachable!(),
        };
        let client = builder.build_single().await;
        let audio_stream = create_single_audio_stream(&audio, &source, DEFAULT_SAMPLE_RATE)?;
        let (response_stream, handle) =
            client
                .from_realtime_audio(audio_stream)
                .await
                .map_err(|e| {
                    CliError::operation_failed("connect realtime transcription", e.to_string())
                })?;
        forward_stream(
            response_stream,
            handle,
            DEFAULT_TIMEOUT_SECS,
            DisplayMode::Single(kind),
            tx,
        )
        .await
    }
}

async fn forward_stream<S, H>(
    response_stream: S,
    handle: H,
    timeout_secs: u64,
    display_mode: DisplayMode,
    tx: mpsc::UnboundedSender<RuntimeEvent>,
) -> CliResult<()>
where
    S: futures_util::Stream<Item = Result<StreamResponse, owhisper_client::hypr_ws_client::Error>>
        + Send,
    H: owhisper_client::FinalizeHandle + Send,
{
    futures_util::pin_mut!(response_stream);
    let read_loop = async {
        while let Some(result) = response_stream.next().await {
            match result {
                Ok(response) => {
                    let done = matches!(
                        &response,
                        StreamResponse::TerminalResponse { .. }
                            | StreamResponse::ErrorResponse { .. }
                    );
                    let _ = tx.send(RuntimeEvent::StreamResponse {
                        response,
                        display_mode,
                    });
                    if done {
                        break;
                    }
                }
                Err(error) => {
                    let _ = tx.send(RuntimeEvent::Failed(error.to_string()));
                    break;
                }
            }
        }
    };

    let _ = tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), read_loop).await;
    let _ = tx.send(RuntimeEvent::StreamEnded);
    handle.finalize().await;
    Ok(())
}

fn create_audio_provider(source: &AudioSource) -> Arc<dyn AudioProvider> {
    #[cfg(feature = "dev")]
    if source.is_mock() {
        return Arc::new(hypr_audio_mock::MockAudio::new(1));
    }

    let _ = source;
    Arc::new(ActualAudio)
}

fn shared_provider(provider: DebugProvider) -> Option<Provider> {
    match provider {
        DebugProvider::Deepgram => Some(Provider::Deepgram),
        DebugProvider::Soniox => Some(Provider::Soniox),
        #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
        DebugProvider::Cactus => Some(Provider::Cactus),
        DebugProvider::ProxyHyprnote
        | DebugProvider::ProxyDeepgram
        | DebugProvider::ProxySoniox => None,
    }
}

async fn resolve_standard_provider(
    provider: DebugProvider,
    api_key: Option<String>,
    model: Option<String>,
) -> CliResult<ResolvedSttConfig> {
    let shared = shared_provider(provider).ok_or_else(|| {
        CliError::operation_failed("resolve debug provider", "provider is not shared")
    })?;
    resolve_config(shared, None, api_key, model, "en").await
}

fn require_model_name(model: Option<&str>, provider: &DebugProvider) -> CliResult<String> {
    if let Some(model) = model {
        return Ok(model.to_string());
    }

    let hint = match provider {
        DebugProvider::Deepgram => "Available models: nova-3, nova-2, nova, enhanced, base",
        DebugProvider::Soniox => "Available models: stt_rt_preview",
        _ => "Pass a model name for the upstream provider.",
    };

    Err(CliError::required_argument_with_hint("--model", hint))
}

fn require_key(key: Option<String>, env_name: &str) -> CliResult<String> {
    key.ok_or_else(|| {
        CliError::required_argument(format!(
            "--{} (or {env_name})",
            env_name.to_lowercase().replace('_', "-")
        ))
    })
}

enum ProxyKind {
    Hyprnote,
    Deepgram,
    Soniox,
}

async fn run_proxy(
    kind: ProxyKind,
    deepgram_api_key: Option<String>,
    soniox_api_key: Option<String>,
    source: AudioSource,
    tx: mpsc::UnboundedSender<RuntimeEvent>,
) -> CliResult<()> {
    use hypr_transcribe_proxy::{HyprnoteRoutingConfig, SttProxyConfig};

    let mut env = hypr_transcribe_proxy::Env::default();
    let provider_name = match kind {
        ProxyKind::Hyprnote => {
            env.stt.deepgram_api_key = deepgram_api_key;
            env.stt.soniox_api_key = soniox_api_key;
            "hyprnote"
        }
        ProxyKind::Deepgram => {
            env.stt.deepgram_api_key = deepgram_api_key;
            "deepgram"
        }
        ProxyKind::Soniox => {
            env.stt.soniox_api_key = soniox_api_key;
            "soniox"
        }
    };

    let supabase_env = hypr_api_env::SupabaseEnv {
        supabase_url: String::new(),
        supabase_anon_key: String::new(),
        supabase_service_role_key: String::new(),
    };

    let config = SttProxyConfig::new(&env, &supabase_env)
        .with_hyprnote_routing(HyprnoteRoutingConfig::default());
    let app = hypr_transcribe_proxy::router(config);
    let server = spawn_router(app).await?;

    tracing::info!("proxy: {} -> {}", server.addr(), provider_name);

    let audio: Arc<dyn AudioProvider> = Arc::new(ActualAudio);
    let api_base = server.api_base("");

    match kind {
        ProxyKind::Hyprnote => {
            run_for_source::<owhisper_client::HyprnoteAdapter>(
                audio,
                source,
                api_base,
                None,
                default_listen_params(),
                tx,
            )
            .await?;
        }
        ProxyKind::Deepgram => {
            run_for_source::<owhisper_client::DeepgramAdapter>(
                audio,
                source,
                api_base,
                None,
                default_listen_params(),
                tx,
            )
            .await?;
        }
        ProxyKind::Soniox => {
            run_for_source::<owhisper_client::SonioxAdapter>(
                audio,
                source,
                api_base,
                None,
                default_listen_params(),
                tx,
            )
            .await?;
        }
    }

    Ok(())
}
