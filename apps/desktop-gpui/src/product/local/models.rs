use std::{
    collections::HashMap,
    path::PathBuf,
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};

use anlg_local_model::LocalModel;
use anlg_model_downloader::{DownloadStatus, ModelDownloadManager, ModelDownloaderRuntime};
use desktop_runtime::{CancellationToken, Result, ServiceError};
use tokio::process::{Child, Command};

use super::failure;

pub struct EngineResources {
    pub argmax: PathBuf,
    pub llama_server: PathBuf,
    pub argmax_api_key: String,
}

struct DownloadRuntime {
    directory: PathBuf,
    progress: Mutex<HashMap<LocalModel, DownloadStatus>>,
}

impl ModelDownloaderRuntime<LocalModel> for DownloadRuntime {
    fn models_base(&self) -> std::result::Result<PathBuf, anlg_model_downloader::Error> {
        Ok(self.directory.clone())
    }
    fn emit_progress(&self, model: &LocalModel, status: DownloadStatus) {
        if let Ok(mut progress) = self.progress.lock() {
            progress.insert(model.clone(), status);
        }
    }
}

pub struct ModelService {
    downloader: ModelDownloadManager<LocalModel>,
    runtime: Arc<DownloadRuntime>,
    resources: EngineResources,
    running: tokio::sync::Mutex<[Option<RunningModel>; 2]>,
}

struct RunningModel {
    model: LocalModel,
    url: String,
    child: Option<Child>,
    native: Option<NativeEngine>,
}

pub enum NativeEngine {
    Soniqo(anlg_transcribe_soniqo::LiveTranscriptionSession),
    Apple(anlg_transcribe_speechanalyzer::LiveTranscriptionSession),
}

impl NativeEngine {
    pub fn stop(self) -> Result<()> {
        match self {
            Self::Soniqo(session) => session.stop().map_err(failure),
            Self::Apple(session) => session.stop().map_err(failure),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ModelState {
    pub model: LocalModel,
    pub installed: bool,
    pub running: bool,
    pub progress: Option<DownloadStatus>,
}

impl ModelService {
    pub fn new(directory: PathBuf, resources: EngineResources) -> Self {
        let runtime = Arc::new(DownloadRuntime {
            directory,
            progress: Mutex::new(HashMap::new()),
        });
        Self {
            downloader: ModelDownloadManager::new(runtime.clone()),
            runtime,
            resources,
            running: tokio::sync::Mutex::new([None, None]),
        }
    }

    pub fn registry() -> Vec<LocalModel> {
        anlg_local_stt_core::SUPPORTED_MODELS
            .iter()
            .cloned()
            .chain(
                anlg_local_llm_core::SUPPORTED_MODELS
                    .iter()
                    .cloned()
                    .map(LocalModel::GgufLlm),
            )
            .collect()
    }

    pub fn resolve(key: &str) -> Result<LocalModel> {
        Self::registry()
            .into_iter()
            .find(|m| m.cli_name() == key)
            .ok_or_else(|| failure("Unknown or unsupported model"))
    }

    pub async fn state(&self, model: LocalModel) -> Result<ModelState> {
        if !model.is_available_on_current_platform() {
            return Ok(ModelState {
                model,
                installed: false,
                running: false,
                progress: None,
            });
        }
        let mut native_progress = None;
        let installed = match model.clone() {
            LocalModel::Soniqo(native) if model.is_available_on_current_platform() => {
                let state = tokio::task::spawn_blocking(move || {
                    anlg_transcribe_soniqo::model_download_state(native)
                })
                .await
                .map_err(failure)?
                .map_err(failure)?;
                native_progress =
                    download_status(&state.status, state.progress_percent, state.error);
                state.status == "ready"
            }
            LocalModel::AppleSpeech(_) if model.is_available_on_current_platform() => {
                let state = tokio::task::spawn_blocking(|| {
                    anlg_transcribe_speechanalyzer::settings_locale().and_then(|locale| {
                        anlg_transcribe_speechanalyzer::model_download_state(&locale)
                    })
                })
                .await
                .map_err(failure)?
                .map_err(failure)?;
                native_progress =
                    download_status(&state.status, state.progress_percent, state.error);
                state.status == "ready"
            }
            _ => self
                .downloader
                .is_downloaded(&model)
                .await
                .map_err(failure)?,
        };
        let mut running = self.running.lock().await;
        let running = &mut running[model_slot(&model)];
        if let Some(current) = running.as_mut()
            && let Some(child) = current.child.as_mut()
            && child.try_wait().map_err(failure)?.is_some()
        {
            *running = None;
        }
        Ok(ModelState {
            running: running.as_ref().is_some_and(|r| r.model == model),
            progress: native_progress.or(self
                .runtime
                .progress
                .lock()
                .map_err(|_| failure("Model state lock poisoned"))?
                .get(&model)
                .cloned()),
            model,
            installed,
        })
    }

    pub async fn download(&self, model: LocalModel) -> Result<()> {
        if !model.is_available_on_current_platform() {
            return Err(failure(
                "This model requires Apple Silicon; choose a cloud transcription provider on this platform",
            ));
        }
        match model {
            LocalModel::Soniqo(native) => tokio::task::spawn_blocking(move || {
                anlg_transcribe_soniqo::start_model_download(native)
            })
            .await
            .map_err(failure)?
            .map_err(failure),
            LocalModel::AppleSpeech(_) => tokio::task::spawn_blocking(|| {
                anlg_transcribe_speechanalyzer::settings_locale().and_then(|locale| {
                    anlg_transcribe_speechanalyzer::start_model_download(&locale)
                })
            })
            .await
            .map_err(failure)?
            .map_err(failure),
            _ => self.downloader.download(&model).await.map_err(failure),
        }
    }

    pub async fn cancel(&self, model: LocalModel) -> Result<()> {
        match model {
            LocalModel::Soniqo(native) => {
                tokio::task::spawn_blocking(move || anlg_transcribe_soniqo::reset_model(native))
                    .await
                    .map_err(failure)?
                    .map_err(failure)
            }
            LocalModel::AppleSpeech(_) => tokio::task::spawn_blocking(|| {
                anlg_transcribe_speechanalyzer::settings_locale()
                    .and_then(|locale| anlg_transcribe_speechanalyzer::release_locale(&locale))
            })
            .await
            .map_err(failure)?
            .map_err(failure),
            _ => self
                .downloader
                .cancel_download(&model)
                .await
                .map(|_| ())
                .map_err(failure),
        }
    }

    pub async fn delete(&self, model: LocalModel) -> Result<()> {
        if self.running.lock().await[model_slot(&model)]
            .as_ref()
            .is_some_and(|r| r.model == model)
        {
            return Err(failure("Stop this model before deleting it"));
        }
        self.cancel(model.clone()).await?;
        match model {
            LocalModel::Soniqo(native) => {
                tokio::task::spawn_blocking(move || anlg_transcribe_soniqo::delete_model(native))
                    .await
                    .map_err(failure)?
                    .map_err(failure)
            }
            LocalModel::AppleSpeech(_) => Ok(()),
            _ => self.downloader.delete(&model).await.map_err(failure),
        }
    }

    pub async fn start(&self, model: LocalModel, cancel: CancellationToken) -> Result<String> {
        if !self.state(model.clone()).await?.installed {
            return Err(failure("Download the model before starting it"));
        }
        let mut running = self.running.lock().await;
        let running = &mut running[model_slot(&model)];
        if let Some(current) = running.as_ref() {
            if current.model == model {
                return Ok(current.url.clone());
            }
            return Err(failure("Stop the active model before starting another"));
        }
        let mut native = None;
        let (url, child) = match &model {
            LocalModel::Soniqo(model) => {
                let model = *model;
                native = Some(NativeEngine::Soniqo(
                    tokio::task::spawn_blocking(move || {
                        anlg_transcribe_soniqo::LiveTranscriptionSession::start(model)
                    })
                    .await
                    .map_err(failure)?
                    .map_err(failure)?,
                ));
                (anlg_transcribe_soniqo::LOCAL_BASE_URL.into(), None)
            }
            LocalModel::AppleSpeech(_) => {
                native = Some(NativeEngine::Apple(
                    tokio::task::spawn_blocking(|| {
                        anlg_transcribe_speechanalyzer::settings_locale().and_then(|locale| {
                            anlg_transcribe_speechanalyzer::LiveTranscriptionSession::start(&locale)
                        })
                    })
                    .await
                    .map_err(failure)?
                    .map_err(failure)?,
                ));
                (anlg_transcribe_speechanalyzer::LOCAL_BASE_URL.into(), None)
            }
            LocalModel::Am(am) => {
                if self.resources.argmax_api_key.is_empty() {
                    return Err(failure("Argmax engine credential is missing"));
                }
                let port = free_port()?;
                let mut child = Command::new(&self.resources.argmax)
                    .args(["--port", &port.to_string()])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .kill_on_drop(true)
                    .spawn()
                    .map_err(failure)?;
                let url = format!("http://127.0.0.1:{port}/v1");
                let client = anlg_am::Client::new(&url);
                let mut ready = false;
                for _ in 0..40 {
                    if cancel.is_cancelled() {
                        return Err(ServiceError::Cancelled);
                    }
                    if child.try_wait().map_err(failure)?.is_some() {
                        return Err(failure("Argmax engine exited during startup"));
                    }
                    if let Ok(response) = client
                        .init(
                            anlg_am::InitRequest::new(self.resources.argmax_api_key.clone())
                                .with_model(am.clone(), self.runtime.directory.join("stt")),
                        )
                        .await
                        && response.is_success()
                    {
                        ready = true;
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                if !ready {
                    return Err(failure("Argmax engine did not become ready"));
                }
                (url, Some(child))
            }
            LocalModel::GgufLlm(_) => {
                let port = free_port()?;
                let mut child = Command::new(&self.resources.llama_server)
                    .arg("--model")
                    .arg(model.install_path(&self.runtime.directory))
                    .args(["--host", "127.0.0.1", "--port", &port.to_string()])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .kill_on_drop(true)
                    .spawn()
                    .map_err(failure)?;
                let base = format!("http://127.0.0.1:{port}");
                let client = reqwest::Client::builder()
                    .timeout(Duration::from_secs(2))
                    .build()
                    .map_err(failure)?;
                let mut ready = false;
                for _ in 0..120 {
                    if cancel.is_cancelled() {
                        return Err(ServiceError::Cancelled);
                    }
                    if child.try_wait().map_err(failure)?.is_some() {
                        return Err(failure("LLM engine exited during startup"));
                    }
                    if let Ok(response) = client.get(format!("{base}/health")).send().await
                        && response.status().is_success()
                    {
                        ready = true;
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                if !ready {
                    return Err(failure("LLM engine did not become ready"));
                }
                (format!("{base}/v1"), Some(child))
            }
            LocalModel::Whisper(_) => {
                return Err(failure(
                    "The shipping selectable registry does not include Whisper CPP",
                ));
            }
        };
        if cancel.is_cancelled() {
            if let Some(native) = native {
                tokio::task::spawn_blocking(move || native.stop())
                    .await
                    .map_err(failure)??;
            }
            return Err(ServiceError::Cancelled);
        }
        *running = Some(RunningModel {
            model,
            url: url.clone(),
            child,
            native,
        });
        Ok(url)
    }

    pub async fn stop(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        let mut errors = Vec::new();
        for current in running.iter_mut() {
            if let Err(error) = stop_running(current).await {
                errors.push(error.to_string());
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(failure(errors.join("; ")))
        }
    }

    pub async fn stop_kind(&self, model: &LocalModel) -> Result<()> {
        stop_running(&mut self.running.lock().await[model_slot(model)]).await
    }

    pub async fn take_native_session(&self) -> Option<NativeEngine> {
        self.running.lock().await[0]
            .as_mut()
            .and_then(|running| running.native.take())
    }
}

fn model_slot(model: &LocalModel) -> usize {
    usize::from(matches!(model, LocalModel::GgufLlm(_)))
}

async fn stop_running(running: &mut Option<RunningModel>) -> Result<()> {
    if let Some(current) = running.as_mut()
        && let Some(child) = current.child.as_mut()
        && child.try_wait().map_err(failure)?.is_none()
    {
        child.kill().await.map_err(failure)?;
        child.wait().await.map_err(failure)?;
    }
    if let Some(native) = running.as_mut().and_then(|current| current.native.take()) {
        tokio::task::spawn_blocking(move || native.stop())
            .await
            .map_err(failure)??;
    }
    *running = None;
    Ok(())
}

fn download_status(
    status: &str,
    percent: Option<u8>,
    error: Option<String>,
) -> Option<DownloadStatus> {
    match status {
        "ready" => Some(DownloadStatus::Completed),
        "downloading" => Some(DownloadStatus::Downloading(percent.unwrap_or(0))),
        _ => error.map(DownloadStatus::Failed),
    }
}

fn free_port() -> Result<u16> {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|socket| socket.local_addr())
        .map(|address| address.port())
        .map_err(failure)
}
