use std::sync::Arc;

use clap::{Parser, ValueEnum};
use hypr_transcribe_proxy::{HyprnoteRoutingConfig, SttProxyConfig};
use transcribe_cli::{
    ActualAudio, AudioArgs, AudioProvider, DEFAULT_SAMPLE_RATE, DEFAULT_TIMEOUT_SECS,
    build_dual_client, build_single_client, default_listen_params, run_dual_client,
    run_single_client, spawn_router,
};

#[derive(Clone, ValueEnum)]
enum ProviderArg {
    Hyprnote,
    Deepgram,
    Soniox,
}

#[derive(Parser)]
struct Args {
    #[command(flatten)]
    audio: AudioArgs,

    #[arg(long)]
    provider: ProviderArg,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let mut env = hypr_transcribe_proxy::Env::default();
    let provider_name = match args.provider {
        ProviderArg::Hyprnote => {
            env.stt.deepgram_api_key = std::env::var("DEEPGRAM_API_KEY").ok();
            env.stt.soniox_api_key = std::env::var("SONIOX_API_KEY").ok();
            "hyprnote"
        }
        ProviderArg::Deepgram => {
            env.stt.deepgram_api_key =
                Some(std::env::var("DEEPGRAM_API_KEY").expect("DEEPGRAM_API_KEY not set"));
            "deepgram"
        }
        ProviderArg::Soniox => {
            env.stt.soniox_api_key =
                Some(std::env::var("SONIOX_API_KEY").expect("SONIOX_API_KEY not set"));
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
    let server = spawn_router(app).await;

    eprintln!("proxy: {} -> {}", server.addr(), provider_name);
    eprintln!();

    let audio: Arc<dyn AudioProvider> = Arc::new(ActualAudio);

    match args.provider {
        ProviderArg::Hyprnote => {
            run_with_adapter::<owhisper_client::HyprnoteAdapter>(
                audio,
                &args.audio.audio,
                server.api_base(""),
            )
            .await;
        }
        ProviderArg::Deepgram => {
            run_with_adapter::<owhisper_client::DeepgramAdapter>(
                audio,
                &args.audio.audio,
                server.api_base(""),
            )
            .await;
        }
        ProviderArg::Soniox => {
            run_with_adapter::<owhisper_client::SonioxAdapter>(
                audio,
                &args.audio.audio,
                server.api_base(""),
            )
            .await;
        }
    }
}

async fn run_with_adapter<A: owhisper_client::RealtimeSttAdapter>(
    audio: Arc<dyn AudioProvider>,
    source: &transcribe_cli::AudioSource,
    api_base: String,
) {
    if source.is_dual() {
        let client = build_dual_client::<A>(api_base, None, default_listen_params()).await;
        run_dual_client(
            audio,
            source.clone(),
            client,
            DEFAULT_SAMPLE_RATE,
            DEFAULT_TIMEOUT_SECS,
        )
        .await;
    } else {
        let client = build_single_client::<A>(api_base, None, default_listen_params()).await;
        run_single_client(
            audio,
            source.clone(),
            client,
            DEFAULT_SAMPLE_RATE,
            DEFAULT_TIMEOUT_SECS,
        )
        .await;
    }
}
