use std::path::PathBuf;

use axum::Router;
use axum::error_handling::HandleError;
use axum::http::StatusCode;
use clap::Parser;
use hypr_transcribe_cactus::TranscribeService;
use transcribe_cli::{
    AudioArgs, DEFAULT_SAMPLE_RATE, DEFAULT_TIMEOUT_SECS, build_single_client,
    default_listen_params, run_single_client, spawn_router,
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
    let client = build_single_client::<owhisper_client::CactusAdapter>(
        server.api_base("/v1"),
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
