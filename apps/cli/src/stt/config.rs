use std::path::{Path, PathBuf};

use hypr_listener2_core::{BatchEvent, BatchParams, BatchProvider, BatchRuntime};
use hypr_local_model::{CactusSttModel, LocalModel, WhisperModel};
#[cfg(target_os = "macos")]
use hypr_local_stt_server::LocalSttServer;
use tokio::sync::mpsc;

use crate::error::{CliError, CliResult, did_you_mean};

use super::SttProvider;

#[cfg(target_os = "macos")]
pub type ServerGuard = Option<hypr_local_stt_server::LocalSttServer>;

#[cfg(not(target_os = "macos"))]
pub type ServerGuard = ();

pub struct SttOverrides {
    pub provider: Option<SttProvider>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub language: String,
    pub models_base: PathBuf,
}

pub struct ChannelBatchRuntime {
    pub tx: mpsc::UnboundedSender<BatchEvent>,
}

impl BatchRuntime for ChannelBatchRuntime {
    fn emit(&self, event: BatchEvent) {
        let _ = self.tx.send(event);
    }
}

#[cfg(target_os = "macos")]
pub struct LocalServerInfo {
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
    #[cfg(feature = "desktop")] _pool: Option<&sqlx::SqlitePool>,
    #[cfg(not(feature = "desktop"))] _pool: Option<()>,
    overrides: SttOverrides,
) -> CliResult<ResolvedSttConfig> {
    let language_code = overrides.language;
    let language = language_code
        .parse::<hypr_language::Language>()
        .map_err(|e| CliError::invalid_argument("--language", language_code, e.to_string()))?;

    let provider = overrides.provider.ok_or_else(|| {
        CliError::required_argument_with_hint("--provider", "Pass --provider explicitly")
    })?;
    let base_url = overrides.base_url;
    let api_key = overrides.api_key;
    let model = overrides.model;

    #[cfg(target_os = "macos")]
    if provider.is_local() {
        let info = match provider {
            SttProvider::Whispercpp => {
                resolve_and_spawn_whisper(&overrides.models_base, model.as_deref()).await?
            }
            #[cfg(all(target_os = "macos", any(target_arch = "arm", target_arch = "aarch64")))]
            SttProvider::Cactus => {
                resolve_and_spawn_cactus(&overrides.models_base, model.as_deref()).await?
            }
            _ => unreachable!("cloud providers are handled below"),
        };

        return Ok(ResolvedSttConfig {
            provider: provider.to_batch_provider(),
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
                    format!("Pass --api-key or set {} env var", cloud.env_key_name()),
                )
            })?;
        return Ok(ResolvedSttConfig {
            provider: provider.to_batch_provider(),
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
        provider: provider.to_batch_provider(),
        base_url,
        api_key,
        model: model.unwrap_or_default(),
        language,
        server: ServerGuard::default(),
    })
}

#[cfg(target_os = "macos")]
pub async fn resolve_and_spawn_whisper(
    models_base: &Path,
    model_name: Option<&str>,
) -> CliResult<LocalServerInfo> {
    let (model, model_path) = resolve_whisper_model(models_base, model_name)?;

    let server = LocalSttServer::start_whisper(model_path)
        .await
        .map_err(|e| CliError::operation_failed("start local whisper server", e.to_string()))?;

    Ok(LocalServerInfo {
        base_url: server.base_url().to_string(),
        model_name: LocalModel::Whisper(model.clone()).cli_name().to_string(),
        server,
    })
}

#[cfg(all(target_os = "macos", any(target_arch = "arm", target_arch = "aarch64")))]
pub async fn resolve_and_spawn_cactus(
    models_base: &Path,
    model_name: Option<&str>,
) -> CliResult<LocalServerInfo> {
    let (model, model_path) = resolve_cactus_model(models_base, model_name)?;

    let server = LocalSttServer::start_cactus(model_path)
        .await
        .map_err(|e| CliError::operation_failed("start local cactus server", e.to_string()))?;

    Ok(LocalServerInfo {
        base_url: server.base_url().to_string(),
        model_name: model.to_string(),
        server,
    })
}

fn whisper_enabled() -> bool {
    cfg!(target_os = "macos")
}

fn unsupported_whisper_error() -> CliError {
    CliError::msg("whisper local models are only available on macOS")
}

fn missing_whisper_model_error() -> CliError {
    CliError::required_argument_with_hint(
        "--model",
        format!(
            "Pass --model explicitly for --provider whispercpp. Valid models: {}",
            whisper_model_names().join(", ")
        ),
    )
}

fn resolve_whisper_model(
    models_base: &Path,
    name: Option<&str>,
) -> CliResult<(WhisperModel, PathBuf)> {
    if !whisper_enabled() {
        return Err(unsupported_whisper_error());
    }

    let name = name.ok_or_else(missing_whisper_model_error)?;
    let model = LocalModel::all()
        .into_iter()
        .find_map(|model| match model {
            LocalModel::Whisper(whisper) if model.cli_name() == name => Some(whisper),
            _ => None,
        })
        .ok_or_else(|| not_found_whisper_model(models_base, name, false))?;

    let model_path = LocalModel::Whisper(model.clone()).install_path(models_base);
    if !model_path.exists() {
        return Err(CliError::not_found(
            format!("whisper model file at '{}'", model_path.display()),
            Some(format!(
                "Download it first: char models download {}",
                LocalModel::Whisper(model.clone()).cli_name()
            )),
        ));
    }

    Ok((model, model_path))
}

fn not_found_whisper_model(
    models_base: &Path,
    name: &str,
    include_downloaded_hint: bool,
) -> CliError {
    if !whisper_enabled() {
        return unsupported_whisper_error();
    }

    let names = whisper_model_names();
    let mut hint = String::new();
    if let Some(suggestion) = did_you_mean(name, &names) {
        hint.push_str(&format!("Did you mean '{suggestion}'?\n\n"));
    }
    if include_downloaded_hint {
        hint.push_str(&suggest_whisper_models(models_base));
    } else {
        hint.push_str("Run `char models list` to see available models.");
    }

    CliError::not_found(format!("whisper model '{name}'"), Some(hint))
}

fn whisper_model_names() -> Vec<&'static str> {
    LocalModel::all()
        .iter()
        .filter_map(|model| match model {
            LocalModel::Whisper(_) => Some(model.cli_name()),
            _ => None,
        })
        .collect()
}

fn suggest_whisper_models(models_base: &Path) -> String {
    if !whisper_enabled() {
        return "Whisper local models are only available on macOS.".to_string();
    }

    let mut downloaded = Vec::new();
    let mut available = Vec::new();

    for model in LocalModel::all() {
        let LocalModel::Whisper(_) = &model else {
            continue;
        };

        if model.install_path(models_base).exists() {
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
        hint.push_str("Download with: char models download <name>");
    }
    if hint.is_empty() {
        hint.push_str("No whisper models found. Run `char models list` to check.");
    }
    hint
}

fn cactus_enabled() -> bool {
    cfg!(target_os = "macos") && cfg!(any(target_arch = "arm", target_arch = "aarch64"))
}

fn unsupported_cactus_error() -> CliError {
    CliError::msg("cactus local models are only available on Apple Silicon Macs")
}

fn canonical_cactus_name(name: &str) -> String {
    if name.starts_with("cactus-") {
        name.to_string()
    } else {
        format!("cactus-{name}")
    }
}

fn missing_cactus_model_error() -> CliError {
    CliError::required_argument_with_hint(
        "--model",
        format!(
            "Pass --model explicitly for --provider cactus. Valid models: {}",
            cactus_model_names().join(", ")
        ),
    )
}

fn cactus_model_names() -> Vec<&'static str> {
    LocalModel::all()
        .iter()
        .filter_map(|model| match model {
            LocalModel::Cactus(_) => Some(model.cli_name()),
            _ => None,
        })
        .collect()
}

fn resolve_cactus_model(
    models_base: &Path,
    name: Option<&str>,
) -> CliResult<(CactusSttModel, PathBuf)> {
    if !cactus_enabled() {
        return Err(unsupported_cactus_error());
    }

    let name = name.ok_or_else(missing_cactus_model_error)?;
    let canonical = canonical_cactus_name(name);
    let model = LocalModel::all()
        .into_iter()
        .find_map(|model| match model {
            LocalModel::Cactus(cactus)
                if model.cli_name() == name || model.cli_name() == canonical =>
            {
                Some(cactus)
            }
            _ => None,
        })
        .ok_or_else(|| not_found_cactus_model(models_base, name, false))?;

    let model_path = LocalModel::Cactus(model.clone()).install_path(models_base);
    if !model_path.exists() {
        return Err(CliError::not_found(
            format!("cactus model files at '{}'", model_path.display()),
            Some(format!(
                "Download it first: char models download {}",
                LocalModel::Cactus(model.clone()).cli_name()
            )),
        ));
    }

    Ok((model, model_path))
}

fn not_found_cactus_model(
    models_base: &Path,
    name: &str,
    include_downloaded_hint: bool,
) -> CliError {
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
        hint.push_str(&suggest_cactus_models(models_base));
    } else {
        hint.push_str("Run `char models list` to see available models.");
    }

    CliError::not_found(format!("cactus model '{name}'"), Some(hint))
}

fn suggest_cactus_models(models_base: &Path) -> String {
    if !cactus_enabled() {
        return "Cactus local models are only available on Apple Silicon Macs.".to_string();
    }
    let mut downloaded = Vec::new();
    let mut available = Vec::new();

    for model in LocalModel::all() {
        let LocalModel::Cactus(_) = &model else {
            continue;
        };

        if model.install_path(models_base).exists() {
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
        hint.push_str("Download with: char models download <name>");
    }
    if hint.is_empty() {
        hint.push_str("No cactus models found. Run `char models list` to check.");
    }
    hint
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_whisper_model_error_mentions_model_flag() {
        let error = missing_whisper_model_error();
        let rendered = error.to_string();

        assert!(rendered.contains("--model"));
        assert!(rendered.contains("whisper-small"));
    }

    #[test]
    fn whisper_not_found_suggests_close_match() {
        let error = not_found_whisper_model(Path::new("/tmp"), "whisper-smal", false);
        let rendered = error.to_string();

        assert!(rendered.contains("Did you mean 'whisper-small'?"));
    }
}
