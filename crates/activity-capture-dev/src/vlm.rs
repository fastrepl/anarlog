use std::{
    io,
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{Duration, SystemTime},
};

use axum::http::StatusCode;
use hypr_activity_capture::ActivityScreenshotCapture;
use hypr_llm_cactus::{CompleteService, ModelManagerBuilder};
use reqwest::Client;
use serde_json::json;
use tokio::sync::{mpsc as tokio_mpsc, oneshot};
use url::Url;

pub(crate) const DEFAULT_VLM_MODEL_NAME: &str = "cactus";
pub(crate) const DEFAULT_VLM_PROMPT: &str =
    "Describe what is visible in this screenshot and infer the active user task in 2-4 sentences.";
pub(crate) const DEFAULT_VLM_TIMEOUT_SECS: u64 = 120;

#[derive(Debug, Clone)]
pub(crate) struct VlmSettings {
    pub(crate) model_path: PathBuf,
    pub(crate) model_name: String,
    pub(crate) prompt: String,
    pub(crate) timeout: Duration,
}

#[derive(Debug, Clone)]
pub(crate) struct InferenceResult {
    pub(crate) screenshot: ActivityScreenshotCapture,
    pub(crate) screenshot_path: PathBuf,
    pub(crate) model_name: String,
    pub(crate) prompt: String,
    pub(crate) response: Result<String, String>,
    pub(crate) started_at: SystemTime,
    pub(crate) finished_at: SystemTime,
}

impl InferenceResult {
    pub(crate) fn latency(&self) -> Duration {
        self.finished_at
            .duration_since(self.started_at)
            .unwrap_or_default()
    }
}

enum Command {
    Infer {
        screenshot: ActivityScreenshotCapture,
        screenshot_path: PathBuf,
    },
    Shutdown,
}

pub(crate) struct VlmRuntime {
    model_name: String,
    command_tx: tokio_mpsc::UnboundedSender<Command>,
    result_rx: Receiver<InferenceResult>,
    handle: Option<thread::JoinHandle<()>>,
}

impl VlmRuntime {
    pub(crate) fn spawn(settings: VlmSettings) -> io::Result<Self> {
        let (command_tx, command_rx) = tokio_mpsc::unbounded_channel();
        let (result_tx, result_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let model_name = settings.model_name.clone();

        let handle = thread::Builder::new()
            .name("activity-capture-dev-vlm".to_string())
            .spawn(move || run_worker(settings, command_rx, result_tx, ready_tx))
            .map_err(io::Error::other)?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                model_name,
                command_tx,
                result_rx,
                handle: Some(handle),
            }),
            Ok(Err(error)) => {
                let _ = handle.join();
                Err(error)
            }
            Err(error) => {
                let _ = handle.join();
                Err(io::Error::other(error))
            }
        }
    }

    pub(crate) fn model_name(&self) -> &str {
        &self.model_name
    }

    pub(crate) fn enqueue(
        &self,
        screenshot: ActivityScreenshotCapture,
        screenshot_path: PathBuf,
    ) -> io::Result<()> {
        self.command_tx
            .send(Command::Infer {
                screenshot,
                screenshot_path,
            })
            .map_err(|error| io::Error::new(io::ErrorKind::BrokenPipe, error.to_string()))
    }

    pub(crate) fn drain_results(&mut self) -> Vec<InferenceResult> {
        let mut results = Vec::new();
        loop {
            match self.result_rx.try_recv() {
                Ok(result) => results.push(result),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        results
    }
}

impl Drop for VlmRuntime {
    fn drop(&mut self) {
        let _ = self.command_tx.send(Command::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn run_worker(
    settings: VlmSettings,
    mut command_rx: tokio_mpsc::UnboundedReceiver<Command>,
    result_tx: mpsc::Sender<InferenceResult>,
    ready_tx: mpsc::Sender<io::Result<()>>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = ready_tx.send(Err(io::Error::other(error)));
            return;
        }
    };

    runtime.block_on(async move {
        let server =
            match LocalServer::spawn(&settings.model_path, settings.model_name.clone()).await {
                Ok(server) => server,
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                    return;
                }
            };

        let client = match Client::builder().timeout(settings.timeout).build() {
            Ok(client) => client,
            Err(error) => {
                let _ = ready_tx.send(Err(io::Error::other(error)));
                server.stop().await;
                return;
            }
        };

        let _ = ready_tx.send(Ok(()));

        while let Some(command) = command_rx.recv().await {
            match command {
                Command::Infer {
                    screenshot,
                    screenshot_path,
                } => {
                    let started_at = SystemTime::now();
                    let response = run_inference(
                        &client,
                        server.completions_url(),
                        &settings,
                        &screenshot,
                        &screenshot_path,
                    )
                    .await;
                    let finished_at = SystemTime::now();

                    let _ = result_tx.send(InferenceResult {
                        screenshot,
                        screenshot_path,
                        model_name: settings.model_name.clone(),
                        prompt: settings.prompt.clone(),
                        response,
                        started_at,
                        finished_at,
                    });
                }
                Command::Shutdown => break,
            }
        }

        server.stop().await;
    });
}

struct LocalServer {
    addr: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
    handle: tokio::task::JoinHandle<()>,
}

impl LocalServer {
    async fn spawn(model_path: &Path, model_name: String) -> io::Result<Self> {
        let _metadata = std::fs::metadata(model_path).map_err(|error| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("model path is not accessible: {error}"),
            )
        })?;

        let manager = ModelManagerBuilder::default()
            .register(model_name.clone(), model_path.to_path_buf())
            .default_model(model_name)
            .build();

        let router = CompleteService::new(manager).into_router(|error| async move {
            (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
        });

        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0u16))
            .await
            .map_err(io::Error::other)?;
        let addr = listener.local_addr().map_err(io::Error::other)?;
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await;
        });

        Ok(Self {
            addr,
            shutdown_tx: Some(shutdown_tx),
            handle,
        })
    }

    fn completions_url(&self) -> String {
        format!("http://{}/v1/chat/completions", self.addr)
    }

    async fn stop(mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        let _ = self.handle.await;
    }
}

async fn run_inference(
    client: &Client,
    url: String,
    settings: &VlmSettings,
    screenshot: &ActivityScreenshotCapture,
    screenshot_path: &Path,
) -> Result<String, String> {
    let image_url = Url::from_file_path(screenshot_path)
        .map_err(|_| {
            format!(
                "failed to convert screenshot path to file URL: {}",
                screenshot_path.display()
            )
        })?
        .to_string();
    let response = client
        .post(url)
        .json(&json!({
            "model": settings.model_name,
            "stream": false,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": build_prompt(&settings.prompt, screenshot),
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": image_url,
                            }
                        }
                    ]
                }
            ]
        }))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    let status = response.status();
    let body = response.text().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(format!("HTTP {status}: {body}"));
    }

    parse_response_text(&body)
}

fn build_prompt(prompt: &str, screenshot: &ActivityScreenshotCapture) -> String {
    let title = screenshot.target.title.as_deref().unwrap_or("-");

    format!(
        "{prompt}\n\nContext:\n- app: {}\n- window: {}\n- reason: {:?}\n- fingerprint: {}",
        screenshot.target.app_name, title, screenshot.reason, screenshot.fingerprint
    )
}

fn parse_response_text(body: &str) -> Result<String, String> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|error| format!("invalid JSON response: {error}"))?;
    value["choices"][0]["message"]["content"]
        .as_str()
        .map(str::trim)
        .filter(|content| !content.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| "response did not include assistant content".to_string())
}
