use clap::{Parser, ValueEnum};
use hypr_transcribe_proxy::{HyprnoteRoutingConfig, SttProxyConfig};
use transcribe_cli::{
    AudioArgs, DEFAULT_SAMPLE_RATE, DEFAULT_TIMEOUT_SECS, build_single_client,
    default_listen_params, run_single_client, spawn_router,
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

    match args.provider {
        ProviderArg::Hyprnote => {
            let client = build_single_client::<owhisper_client::HyprnoteAdapter>(
                server.api_base(""),
                None,
                default_listen_params(),
            )
            .await;

            run_single_client(
                args.audio.audio,
                client,
                DEFAULT_SAMPLE_RATE,
                DEFAULT_TIMEOUT_SECS,
            )
            .await;
        }
        ProviderArg::Deepgram => {
            let client = build_single_client::<owhisper_client::DeepgramAdapter>(
                server.api_base(""),
                None,
                default_listen_params(),
            )
            .await;

            run_single_client(
                args.audio.audio,
                client,
                DEFAULT_SAMPLE_RATE,
                DEFAULT_TIMEOUT_SECS,
            )
            .await;
        }
        ProviderArg::Soniox => {
            let client = build_single_client::<owhisper_client::SonioxAdapter>(
                server.api_base(""),
                None,
                default_listen_params(),
            )
            .await;

            run_single_client(
                args.audio.audio,
                client,
                DEFAULT_SAMPLE_RATE,
                DEFAULT_TIMEOUT_SECS,
            )
            .await;
        }
    }
}
