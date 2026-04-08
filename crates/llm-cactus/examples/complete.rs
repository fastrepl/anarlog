use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use axum::http::StatusCode;
use llm_cactus::{CompleteService, ModelManagerBuilder};

struct Args {
    model: PathBuf,
    prompt: String,
    system: Option<String>,
    model_name: String,
    temperature: Option<f32>,
}

impl Args {
    fn parse() -> Self {
        let mut args = pico_args::Arguments::from_env();

        let model = args.value_from_str("--model").unwrap_or_else(|_| {
            eprintln!("error: --model <PATH> is required");
            std::process::exit(1);
        });

        let prompt = args.value_from_str("--prompt").unwrap_or_else(|_| {
            eprintln!("error: --prompt <TEXT> is required");
            std::process::exit(1);
        });

        let system = args.opt_value_from_str("--system").unwrap_or_else(|e| {
            eprintln!("error: {e}");
            std::process::exit(1);
        });

        let model_name = args
            .opt_value_from_str("--model-name")
            .unwrap_or_else(|e| {
                eprintln!("error: {e}");
                std::process::exit(1);
            })
            .unwrap_or_else(|| "cactus".to_string());

        let temperature = args
            .opt_value_from_str("--temperature")
            .unwrap_or_else(|e| {
                eprintln!("error: {e}");
                std::process::exit(1);
            });

        let _ = args.finish();

        Self {
            model,
            prompt,
            system,
            model_name,
            temperature,
        }
    }
}

struct LocalServer {
    addr: SocketAddr,
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

impl LocalServer {
    async fn spawn(model_path: PathBuf, model_name: String) -> Self {
        let manager = ModelManagerBuilder::default()
            .register(model_name.clone(), model_path)
            .default_model(model_name)
            .build();

        let app = CompleteService::new(manager)
            .into_router(|err| async move { (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()) });

        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0u16))
            .await
            .expect("failed to bind local server");
        let addr = listener.local_addr().expect("failed to read local addr");
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .expect("local server crashed");
        });

        Self { addr, shutdown_tx }
    }
}

impl Drop for LocalServer {
    fn drop(&mut self) {
        let shutdown_tx =
            std::mem::replace(&mut self.shutdown_tx, tokio::sync::oneshot::channel().0);
        let _ = shutdown_tx.send(());
    }
}

/// cargo run -p llm-cactus --example complete -- --model ~/Library/Application\ Support/hyprnote/models/cactus/qwen2.5-3b-instruct-q4km --prompt "Write a haiku about note taking"
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = Args::parse();

    assert!(
        args.model.exists(),
        "model not found: {}",
        args.model.display()
    );

    let server = LocalServer::spawn(args.model.clone(), args.model_name.clone()).await;
    let client = reqwest::Client::new();
    let url = format!("http://{}/v1/chat/completions", server.addr);

    let mut messages = Vec::new();
    if let Some(system) = args.system {
        messages.push(serde_json::json!({
            "role": "system",
            "content": system,
        }));
    }
    messages.push(serde_json::json!({
        "role": "user",
        "content": args.prompt,
    }));

    let response = client
        .post(url)
        .json(&serde_json::json!({
            "model": args.model_name,
            "stream": false,
            "temperature": args.temperature,
            "messages": messages,
        }))
        .send()
        .await
        .expect("request failed");

    let status = response.status();
    let body = response.text().await.expect("failed to read response body");

    if !status.is_success() {
        eprintln!("request failed: HTTP {status}");
        eprintln!("{body}");
        std::process::exit(1);
    }

    let json: serde_json::Value = serde_json::from_str(&body).expect("invalid JSON response");
    println!(
        "{}",
        serde_json::to_string_pretty(&json).expect("failed to format response")
    );
}
