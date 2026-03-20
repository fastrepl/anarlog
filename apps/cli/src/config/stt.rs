use std::path::PathBuf;

use hypr_listener2_core::{BatchEvent, BatchParams, BatchProvider, BatchRuntime};
use hypr_local_model::{CactusSttModel, LocalModel};
#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
use hypr_local_stt_server::LocalSttServer;
use tokio::sync::mpsc;

use crate::cli::Provider;
use crate::config::paths;
use crate::error::{CliError, CliResult, did_you_mean};

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
pub(crate) type ServerGuard = Option<hypr_local_stt_server::LocalSttServer>;

#[cfg(not(any(target_arch = "arm", target_arch = "aarch64")))]
pub(crate) type ServerGuard = ();

pub struct SttGlobalArgs {
    pub provider: Provider,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub language: String,
}

pub struct ChannelBatchRuntime {
    pub tx: mpsc::UnboundedSender<BatchEvent>,
}

impl BatchRuntime for ChannelBatchRuntime {
    fn emit(&self, event: BatchEvent) {
        let _ = self.tx.send(event);
    }
}

impl Provider {
    pub fn is_local(&self) -> bool {
        match self {
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            Provider::Cactus => true,
            _ => false,
        }
    }

    fn cloud_provider(&self) -> Option<owhisper_client::Provider> {
        match self {
            Provider::Deepgram => Some(owhisper_client::Provider::Deepgram),
            Provider::Soniox => Some(owhisper_client::Provider::Soniox),
            Provider::Assemblyai => Some(owhisper_client::Provider::AssemblyAI),
            Provider::Fireworks => Some(owhisper_client::Provider::Fireworks),
            Provider::Openai => Some(owhisper_client::Provider::OpenAI),
            Provider::Gladia => Some(owhisper_client::Provider::Gladia),
            Provider::Elevenlabs => Some(owhisper_client::Provider::ElevenLabs),
            Provider::Mistral => Some(owhisper_client::Provider::Mistral),
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            Provider::Cactus => None,
        }
    }

    fn to_batch_provider(&self) -> BatchProvider {
        match self {
            Provider::Deepgram => BatchProvider::Deepgram,
            Provider::Soniox => BatchProvider::Soniox,
            Provider::Assemblyai => BatchProvider::AssemblyAI,
            Provider::Fireworks => BatchProvider::Fireworks,
            Provider::Openai => BatchProvider::OpenAI,
            Provider::Gladia => BatchProvider::Gladia,
            Provider::Elevenlabs => BatchProvider::ElevenLabs,
            Provider::Mistral => BatchProvider::Mistral,
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            Provider::Cactus => BatchProvider::Cactus,
        }
    }
}

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
pub struct CactusServerInfo {
    pub server: LocalSttServer,
    pub base_url: String,
    pub model_name: String,
}

pub struct ResolvedSttConfig {
    pub provider: BatchProvider,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub language: hypr_language::Language,
    pub server: ServerGuard,
}

impl ResolvedSttConfig {
    pub fn model_option(&self) -> Option<String> {
        if self.model.is_empty() {
            None
        } else {
            Some(self.model.clone())
        }
    }

    pub fn to_batch_params(
        &self,
        session_id: String,
        file_path: String,
        keywords: Vec<String>,
    ) -> BatchParams {
        BatchParams {
            session_id,
            provider: self.provider.clone(),
            file_path,
            model: self.model_option(),
            base_url: self.base_url.clone(),
            api_key: self.api_key.clone(),
            languages: vec![self.language.clone()],
            keywords,
        }
    }
}

pub async fn resolve_config(
    provider: Provider,
    base_url: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
    language_code: impl Into<String>,
) -> CliResult<ResolvedSttConfig> {
    let language_code = language_code.into();
    let language = language_code
        .parse::<hypr_language::Language>()
        .map_err(|e| CliError::invalid_argument("--language", language_code, e.to_string()))?;

    let batch_provider = provider.to_batch_provider();

    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    if provider.is_local() {
        let info = resolve_and_spawn_cactus(model.as_deref()).await?;
        return Ok(ResolvedSttConfig {
            provider: batch_provider,
            base_url: info.base_url,
            api_key: api_key.unwrap_or_default(),
            model: info.model_name,
            language,
            server: Some(info.server),
        });
    }

    if let Some(cloud) = provider.cloud_provider() {
        let base_url = base_url.unwrap_or_else(|| cloud.default_api_base().to_string());
        let api_key = api_key
            .or_else(|| std::env::var(cloud.env_key_name()).ok())
            .ok_or_else(|| {
                CliError::required_argument_with_hint(
                    "STT API key",
                    "Run `char connect` to configure your STT provider",
                )
            })?;
        return Ok(ResolvedSttConfig {
            provider: batch_provider,
            base_url,
            api_key,
            model: model.unwrap_or_default(),
            language,
            server: ServerGuard::default(),
        });
    }

    let base_url =
        base_url.ok_or_else(|| CliError::required_argument("--base-url (or CHAR_BASE_URL)"))?;
    let api_key =
        api_key.ok_or_else(|| CliError::required_argument("--api-key (or CHAR_API_KEY)"))?;

    Ok(ResolvedSttConfig {
        provider: batch_provider,
        base_url,
        api_key,
        model: model.unwrap_or_default(),
        language,
        server: ServerGuard::default(),
    })
}

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
pub async fn resolve_and_spawn_cactus(model_name: Option<&str>) -> CliResult<CactusServerInfo> {
    let (model, model_path) = resolve_cactus_model(model_name)?;

    let server = LocalSttServer::start(model_path)
        .await
        .map_err(|e| CliError::operation_failed("start local cactus server", e.to_string()))?;

    Ok(CactusServerInfo {
        base_url: server.base_url().to_string(),
        model_name: model.to_string(),
        server,
    })
}

#[cfg_attr(not(feature = "dev"), allow(dead_code))]
pub fn resolve_local_model_path(
    model_id: Option<&str>,
    model_path: Option<PathBuf>,
) -> CliResult<PathBuf> {
    if let Some(path) = model_path {
        if !path.exists() {
            return Err(CliError::not_found(
                format!("model path '{}'", path.display()),
                None,
            ));
        }
        return Ok(path);
    }

    if let Some(name) = model_id {
        return resolve_cactus_model_path(name);
    }

    Err(CliError::required_argument_with_hint(
        "--model or --model-path",
        suggest_cactus_models(),
    ))
}

#[cfg_attr(not(feature = "dev"), allow(dead_code))]
fn resolve_cactus_model_path(name: &str) -> CliResult<PathBuf> {
    if !cactus_enabled() {
        return Err(unsupported_cactus_error());
    }

    let models_base = paths::resolve_paths().models_base;
    let canonical = canonical_cactus_name(name);

    let model = LocalModel::all()
        .into_iter()
        .find(|model| {
            matches!(model, LocalModel::Cactus(_))
                && (model.cli_name() == name || model.cli_name() == canonical)
        })
        .ok_or_else(|| not_found_cactus_model(name, true))?;

    let path = model.install_path(&models_base);
    if !path.exists() {
        return Err(CliError::not_found(
            format!("cactus model '{name}' (not downloaded)"),
            Some(format!(
                "Download it first: char model cactus download {name}"
            )),
        ));
    }

    Ok(path)
}

// --- Cactus helpers (inlined from config/cactus.rs) ---

fn cactus_enabled() -> bool {
    cfg!(any(target_arch = "arm", target_arch = "aarch64"))
}

fn unsupported_cactus_error() -> CliError {
    CliError::msg("cactus local models are only available on ARM devices")
}

fn canonical_cactus_name(name: &str) -> String {
    if name.starts_with("cactus-") {
        name.to_string()
    } else {
        format!("cactus-{name}")
    }
}

fn default_cactus_model() -> CactusSttModel {
    if cfg!(target_arch = "aarch64") && cfg!(target_os = "macos") {
        CactusSttModel::WhisperSmallInt8Apple
    } else {
        CactusSttModel::WhisperSmallInt8
    }
}

fn resolve_cactus_model(name: Option<&str>) -> CliResult<(CactusSttModel, PathBuf)> {
    if !cactus_enabled() {
        return Err(unsupported_cactus_error());
    }

    let models_base = paths::resolve_paths().models_base;

    let model = match name {
        Some(name) => {
            let canonical = canonical_cactus_name(name);
            LocalModel::all()
                .into_iter()
                .find_map(|model| match model {
                    LocalModel::Cactus(cactus)
                        if model.cli_name() == name || model.cli_name() == canonical =>
                    {
                        Some(cactus)
                    }
                    _ => None,
                })
                .ok_or_else(|| not_found_cactus_model(name, false))?
        }
        None => default_cactus_model(),
    };

    let model_path = LocalModel::Cactus(model.clone()).install_path(&models_base);
    if !model_path.exists() {
        return Err(CliError::not_found(
            format!("cactus model files at '{}'", model_path.display()),
            Some(format!(
                "Download it first: char model cactus download {}",
                LocalModel::Cactus(model.clone()).cli_name()
            )),
        ));
    }

    Ok((model, model_path))
}

fn not_found_cactus_model(name: &str, include_downloaded_hint: bool) -> CliError {
    if !cactus_enabled() {
        return unsupported_cactus_error();
    }

    let names: Vec<&str> = LocalModel::all()
        .iter()
        .filter_map(|model| {
            if matches!(model, LocalModel::Cactus(_)) {
                Some(model.cli_name())
            } else {
                None
            }
        })
        .collect();

    let mut hint = String::new();
    if let Some(suggestion) = did_you_mean(name, &names) {
        hint.push_str(&format!("Did you mean '{suggestion}'?\n\n"));
    }
    if include_downloaded_hint {
        hint.push_str(&suggest_cactus_models());
    } else {
        hint.push_str("Run `char model cactus list` to see available models.");
    }

    CliError::not_found(format!("cactus model '{name}'"), Some(hint))
}

fn suggest_cactus_models() -> String {
    if !cactus_enabled() {
        return "Cactus local models are only available on ARM devices.".to_string();
    }

    let models_base = paths::resolve_paths().models_base;
    let mut downloaded = Vec::new();
    let mut available = Vec::new();

    for model in LocalModel::all() {
        let LocalModel::Cactus(_) = &model else {
            continue;
        };

        if model.install_path(&models_base).exists() {
            downloaded.push(model.cli_name());
        } else {
            available.push(model.cli_name());
        }
    }

    let mut hint = String::new();
    if !downloaded.is_empty() {
        hint.push_str("Downloaded models:\n");
        for name in &downloaded {
            hint.push_str(&format!("  {name}\n"));
        }
    }
    if !available.is_empty() {
        if !downloaded.is_empty() {
            hint.push_str("Other models (not downloaded):\n");
        } else {
            hint.push_str("No models downloaded. Available models:\n");
        }
        for name in &available {
            hint.push_str(&format!("  {name}\n"));
        }
        hint.push_str("Download with: char model cactus download <name>");
    }
    if hint.is_empty() {
        hint.push_str("No cactus models found. Run `char model cactus list` to check.");
    }
    hint
}
