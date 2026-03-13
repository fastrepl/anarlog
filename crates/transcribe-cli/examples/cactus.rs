use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::error_handling::HandleError;
use axum::http::StatusCode;
use clap::Parser;
use hypr_transcribe_cactus::TranscribeService;
use transcribe_cli::{
    ActualAudio, AudioArgs, AudioProvider, DEFAULT_SAMPLE_RATE, DEFAULT_TIMEOUT_SECS,
    build_dual_client, build_single_client, default_listen_params, run_dual_client,
    run_single_client, spawn_router,
};

#[derive(Parser)]
struct Args {
    #[command(flatten)]
    audio: AudioArgs,

    #[arg(long)]
    model: PathBuf,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let audio: Arc<dyn AudioProvider> = Arc::new(ActualAudio);

    assert!(
        args.model.exists(),
        "model not found: {}",
        args.model.display()
    );

    let app = Router::new().route_service(
        "/v1/listen",
        HandleError::new(
            TranscribeService::builder().model_path(args.model).build(),
            |err: String| async move { (StatusCode::INTERNAL_SERVER_ERROR, err) },
        ),
    );
    let server = spawn_router(app).await;
    if args.audio.audio.is_dual() {
        let client = build_dual_client::<owhisper_client::CactusAdapter>(
            server.api_base("/v1"),
            None,
            default_listen_params(),
        )
        .await;

        run_dual_client(
            audio,
            args.audio.audio,
            client,
            DEFAULT_SAMPLE_RATE,
            DEFAULT_TIMEOUT_SECS,
        )
        .await;
    } else {
        let client = build_single_client::<owhisper_client::CactusAdapter>(
            server.api_base("/v1"),
            None,
            default_listen_params(),
        )
        .await;

        run_single_client(
            audio,
            args.audio.audio,
            client,
            DEFAULT_SAMPLE_RATE,
            DEFAULT_TIMEOUT_SECS,
        )
        .await;
    }
}
