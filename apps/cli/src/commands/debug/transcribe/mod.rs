pub mod audio;
pub mod client;
pub mod display;
pub mod server;

use std::sync::Arc;

use owhisper_client::RealtimeSttAdapter;

use self::audio::*;
use self::client::*;
use self::server::spawn_router;
pub use crate::cli::{DebugProvider, TranscribeArgs};
use crate::commands::Provider as SharedProvider;
#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
use crate::config::stt::resolve_local_model_path;
use crate::config::stt::{ResolvedSttConfig, resolve_config};
use crate::error::{CliError, CliResult};
#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
use std::path::PathBuf;

impl DebugProvider {
    fn is_local(&self) -> bool {
        match self {
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            DebugProvider::Cactus => true,
            _ => false,
        }
    }

    fn shared_provider(&self) -> Option<SharedProvider> {
        match self {
            DebugProvider::Deepgram => Some(SharedProvider::Deepgram),
            DebugProvider::Soniox => Some(SharedProvider::Soniox),
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            DebugProvider::Cactus => Some(SharedProvider::Cactus),
            DebugProvider::ProxyHyprnote
            | DebugProvider::ProxyDeepgram
            | DebugProvider::ProxySoniox => None,
        }
    }
}

pub async fn run(args: TranscribeArgs) -> CliResult<()> {
    if let Some(ref model_path) = args.model_path {
        if !args.provider.is_local() {
            return Err(CliError::invalid_argument_with_hint(
                "--model-path",
                model_path.display().to_string(),
                "only valid with local providers (cactus)",
                "Use --provider cactus for local model files, or remove --model-path for cloud providers.",
            ));
        }
    }

    match args.provider {
        DebugProvider::Deepgram => {
            let model = require_model_name(args.model.as_deref(), &args.provider)?;
            let resolved =
                resolve_standard_provider(&args.provider, args.deepgram_api_key, Some(model))
                    .await?;
            run_resolved_provider::<owhisper_client::DeepgramAdapter>(&resolved, args.audio.audio)
                .await?;
        }
        DebugProvider::Soniox => {
            let model = require_model_name(args.model.as_deref(), &args.provider)?;
            let resolved =
                resolve_standard_provider(&args.provider, args.soniox_api_key, Some(model)).await?;
            run_resolved_provider::<owhisper_client::SonioxAdapter>(&resolved, args.audio.audio)
                .await?;
        }
        #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
        DebugProvider::Cactus => {
            if args.model_path.is_some() {
                let model_path = resolve_local_model_path(args.model.as_deref(), args.model_path)?;
                run_cactus_from_path(model_path, args.audio.audio).await?;
            } else {
                let resolved = resolve_standard_provider(&args.provider, None, args.model).await?;
                run_resolved_provider::<owhisper_client::CactusAdapter>(
                    &resolved,
                    args.audio.audio,
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
            )
            .await?;
        }
        DebugProvider::ProxyDeepgram => {
            let api_key = require_key(args.deepgram_api_key, "DEEPGRAM_API_KEY")?;
            run_proxy(ProxyKind::Deepgram, Some(api_key), None, args.audio.audio).await?;
        }
        DebugProvider::ProxySoniox => {
            let api_key = require_key(args.soniox_api_key, "SONIOX_API_KEY")?;
            run_proxy(ProxyKind::Soniox, None, Some(api_key), args.audio.audio).await?;
        }
    }
    Ok(())
}

fn require_model_name(model: Option<&str>, provider: &DebugProvider) -> CliResult<String> {
    if let Some(m) = model {
        return Ok(m.to_string());
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

async fn resolve_standard_provider(
    provider: &DebugProvider,
    api_key: Option<String>,
    model: Option<String>,
) -> CliResult<ResolvedSttConfig> {
    let shared = provider.shared_provider().ok_or_else(|| {
        CliError::operation_failed("resolve debug provider", "provider is not shared")
    })?;
    resolve_config(shared, None, api_key, model, "en").await
}

fn create_audio_provider(source: &AudioSource) -> Arc<dyn AudioProvider> {
    #[cfg(feature = "mock-audio")]
    if source.is_mock() {
        return Arc::new(hypr_audio_mock::MockAudio::new(1));
    }
    let _ = source;
    Arc::new(ActualAudio)
}

async fn run_resolved_provider<A: RealtimeSttAdapter>(
    resolved: &ResolvedSttConfig,
    source: AudioSource,
) -> CliResult<()> {
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

    run_for_source::<A>(audio, source, &resolved.base_url, api_key, params).await
}

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
async fn run_cactus_from_path(model_path: PathBuf, source: AudioSource) -> CliResult<()> {
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
    )
    .await?;

    // keep server alive until transcription ends
    drop(server);
    Ok(())
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

    eprintln!("proxy: {} -> {}", server.addr(), provider_name);
    eprintln!();

    let audio: Arc<dyn AudioProvider> = Arc::new(ActualAudio);
    let api_base = server.api_base("");

    match kind {
        ProxyKind::Hyprnote => {
            run_with_adapter::<owhisper_client::HyprnoteAdapter>(audio, &source, api_base).await?;
        }
        ProxyKind::Deepgram => {
            run_with_adapter::<owhisper_client::DeepgramAdapter>(audio, &source, api_base).await?;
        }
        ProxyKind::Soniox => {
            run_with_adapter::<owhisper_client::SonioxAdapter>(audio, &source, api_base).await?;
        }
    }

    Ok(())
}

async fn run_with_adapter<A: RealtimeSttAdapter>(
    audio: Arc<dyn AudioProvider>,
    source: &AudioSource,
    api_base: String,
) -> CliResult<()> {
    run_for_source::<A>(
        audio,
        source.clone(),
        api_base,
        None,
        default_listen_params(),
    )
    .await
}
