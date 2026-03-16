mod list;
mod runtime;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use hypr_local_model::{LocalModel, LocalModelKind};
use hypr_local_stt_core::SUPPORTED_MODELS as SUPPORTED_STT_MODELS;
use hypr_model_downloader::ModelDownloadManager;

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
pub use crate::cli::CactusCommands;
pub use crate::cli::{ModelCommands, ModelKind};
use crate::config::cactus;
use crate::config::desktop as settings;
use crate::error::{CliError, CliResult, did_you_mean};
use runtime::CliModelRuntime;

pub async fn run(command: ModelCommands) -> CliResult<()> {
    let paths = settings::resolve_paths();
    let models_base = paths.models_base.clone();

    match command {
        ModelCommands::Paths => {
            println!("global_base={}", paths.global_base.display());
            println!("vault_base={}", paths.vault_base.display());
            println!("settings_path={}", paths.settings_path.display());
            println!("models_base={}", models_base.display());
            Ok(())
        }
        ModelCommands::Current => {
            println!("settings_path={}", paths.settings_path.display());

            let Some(current) = settings::load_settings(&paths.settings_path) else {
                println!("stt\tprovider=unset\tmodel=unset\tconfig=unavailable");
                println!("llm\tprovider=unset\tmodel=unset\tconfig=unavailable");
                return Ok(());
            };

            let stt_provider = current.current_stt_provider.as_deref().unwrap_or("unset");
            let stt_model = current.current_stt_model.as_deref().unwrap_or("unset");
            let llm_provider = current.current_llm_provider.as_deref().unwrap_or("unset");
            let llm_model = current.current_llm_model.as_deref().unwrap_or("unset");

            let stt_config = current
                .current_stt_provider
                .as_deref()
                .and_then(|id| current.stt_providers.get(id));
            let llm_config = current
                .current_llm_provider
                .as_deref()
                .and_then(|id| current.llm_providers.get(id));

            println!(
                "stt\tprovider={}\tmodel={}\t{}",
                stt_provider,
                stt_model,
                format_provider_config_status(stt_config)
            );
            println!(
                "llm\tprovider={}\tmodel={}\t{}",
                llm_provider,
                llm_model,
                format_provider_config_status(llm_config)
            );
            Ok(())
        }
        ModelCommands::List {
            kind,
            supported,
            format,
        } => {
            let runtime = Arc::new(CliModelRuntime {
                models_base: models_base.clone(),
                progress: None,
            });
            let manager = ModelDownloadManager::new(runtime);
            let current = settings::load_settings(&paths.settings_path);

            let models = if supported {
                supported_models(kind)?
            } else {
                all_models(kind)
            };

            let rows = list::collect_model_rows(&models, &models_base, &current, &manager).await;
            list::write_model_output(&rows, &models_base, format).await
        }
        #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
        ModelCommands::Cactus { command } => {
            run_cactus(command, &paths.settings_path, &models_base).await
        }
        ModelCommands::Download { name } => {
            let Some(model) = find_model(&name) else {
                return Err(not_found_model(&name));
            };
            download_model(model, &models_base).await
        }
        ModelCommands::Delete { name } => {
            let Some(model) = find_model(&name) else {
                return Err(not_found_model(&name));
            };
            delete_model(model, &models_base).await
        }
    }
}

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
async fn run_cactus(
    command: CactusCommands,
    settings_path: &std::path::Path,
    models_base: &std::path::Path,
) -> CliResult<()> {
    match command {
        CactusCommands::List { format } => {
            let runtime = Arc::new(CliModelRuntime {
                models_base: models_base.to_path_buf(),
                progress: None,
            });
            let manager = ModelDownloadManager::new(runtime);
            let current = settings::load_settings(settings_path);
            let models = cactus::all_cactus_models();

            let rows = list::collect_model_rows(&models, models_base, &current, &manager).await;
            list::write_model_output(&rows, models_base, format).await
        }
        CactusCommands::Download { name } => {
            let Some(model) = cactus::find_cactus_model(&name) else {
                return Err(cactus::not_found_cactus_model(&name, false));
            };
            download_model(model, models_base).await
        }
        CactusCommands::Delete { name } => {
            let Some(model) = cactus::find_cactus_model(&name) else {
                return Err(cactus::not_found_cactus_model(&name, false));
            };
            delete_model(model, models_base).await
        }
    }
}

async fn download_model(model: LocalModel, models_base: &Path) -> CliResult<()> {
    let progress = make_download_progress_bar(&model);
    let runtime = Arc::new(CliModelRuntime {
        models_base: models_base.to_path_buf(),
        progress: progress.clone(),
    });
    let manager = ModelDownloadManager::new(runtime);

    if manager.is_downloaded(&model).await.unwrap_or(false) {
        println!(
            "Model already downloaded: {} ({})",
            model.display_name(),
            model.install_path(models_base).display()
        );
        return Ok(());
    }

    if let Err(e) = manager.download(&model).await {
        if let Some(progress) = progress {
            progress.abandon_with_message("Failed");
        }
        return Err(CliError::operation_failed(
            "start model download",
            format!("{}: {e}", model.cli_name()),
        ));
    }

    while manager.is_downloading(&model).await {
        tokio::time::sleep(Duration::from_millis(120)).await;
    }

    if manager.is_downloaded(&model).await.unwrap_or(false) {
        if let Some(progress) = progress {
            progress.finish_and_clear();
        }
        println!(
            "Downloaded {} -> {}",
            model.display_name(),
            model.install_path(models_base).display()
        );
        Ok(())
    } else {
        if let Some(progress) = progress {
            progress.abandon_with_message("Failed");
        }
        Err(CliError::operation_failed(
            "download model",
            model.cli_name().to_string(),
        ))
    }
}

async fn delete_model(model: LocalModel, models_base: &Path) -> CliResult<()> {
    let runtime = Arc::new(CliModelRuntime {
        models_base: models_base.to_path_buf(),
        progress: None,
    });
    let manager = ModelDownloadManager::new(runtime);

    if let Err(e) = manager.delete(&model).await {
        return Err(CliError::operation_failed(
            "delete model",
            format!("{}: {e}", model.cli_name()),
        ));
    }

    println!("Deleted {}", model.display_name());
    Ok(())
}

fn make_download_progress_bar(model: &LocalModel) -> Option<indicatif::ProgressBar> {
    crate::output::create_progress_bar(
        &format!("Downloading {}", model.cli_name()),
        "{spinner} {msg} [{wide_bar}] {pos:>3}%",
        "=>-",
    )
}

fn find_model(name: &str) -> Option<LocalModel> {
    all_models(None)
        .into_iter()
        .find(|model| model.cli_name() == name)
}

fn all_models(kind: Option<ModelKind>) -> Vec<LocalModel> {
    LocalModel::all()
        .into_iter()
        .filter(|model| model_is_enabled(model) && matches_kind(model, kind))
        .collect()
}

fn supported_models(kind: Option<ModelKind>) -> CliResult<Vec<LocalModel>> {
    match kind {
        Some(ModelKind::Stt) => Ok(SUPPORTED_STT_MODELS
            .iter()
            .filter(|model| model_is_enabled(model))
            .cloned()
            .collect()),
        Some(ModelKind::Llm) => Err(CliError::invalid_argument(
            "--supported",
            "true",
            "Only STT has a shared supported model list right now; use `--kind stt`.",
        )),
        None => Err(CliError::invalid_argument(
            "--supported",
            "true",
            "Pass `--kind stt` (supported list is STT-only right now).",
        )),
    }
}

fn model_is_enabled(model: &LocalModel) -> bool {
    cactus::CACTUS_ENABLED || !cactus::is_cactus_local_model(model)
}

fn matches_kind(model: &LocalModel, kind: Option<ModelKind>) -> bool {
    match kind {
        None => true,
        Some(ModelKind::Stt) => model.model_kind() == LocalModelKind::Stt,
        Some(ModelKind::Llm) => model.model_kind() == LocalModelKind::Llm,
    }
}

fn format_provider_config_status(config: Option<&settings::ProviderConfig>) -> String {
    let Some(config) = config else {
        return "config=missing".to_string();
    };

    let base_url = if config.base_url.is_some() {
        "set"
    } else {
        "missing"
    };
    let api_key = if config.has_api_key { "set" } else { "missing" };

    format!("config=base_url:{} api_key:{}", base_url, api_key)
}

fn not_found_model(name: &str) -> CliError {
    let names: Vec<&str> = all_models(None).iter().map(|m| m.cli_name()).collect();
    let mut hint = String::new();
    if let Some(suggestion) = did_you_mean(name, &names) {
        hint.push_str(&format!("Did you mean '{suggestion}'?\n\n"));
    }
    hint.push_str("Run `char model list` to see available models.");
    CliError::not_found(format!("model '{name}'"), Some(hint))
}

fn is_current_model(model: &LocalModel, current: &settings::DesktopSettings) -> bool {
    match model.model_kind() {
        LocalModelKind::Llm => {
            current.current_llm_model.as_deref() == model.settings_name().as_deref()
        }
        LocalModelKind::Stt => {
            current.current_stt_provider.as_deref() == Some("hyprnote")
                && current.current_stt_model.as_deref() != Some("cloud")
                && current.current_stt_model.as_deref() == model.settings_name().as_deref()
        }
    }
}

trait SettingsName {
    fn settings_name(&self) -> Option<String>;
}

impl SettingsName for LocalModel {
    fn settings_name(&self) -> Option<String> {
        serde_json::to_value(self)
            .ok()?
            .as_str()
            .map(ToString::to_string)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn empty_settings() -> settings::DesktopSettings {
        settings::DesktopSettings {
            current_stt_provider: None,
            current_stt_model: None,
            current_llm_provider: None,
            current_llm_model: None,
            stt_providers: HashMap::new(),
            llm_providers: HashMap::new(),
        }
    }

    #[test]
    fn stt_current_model_uses_serialized_name() {
        let model = LocalModel::Whisper(hypr_local_model::WhisperModel::QuantizedTiny);
        let mut current = empty_settings();
        current.current_stt_provider = Some("hyprnote".to_string());
        current.current_stt_model = Some("QuantizedTiny".to_string());

        assert!(is_current_model(&model, &current));
    }

    #[test]
    fn llm_current_model_uses_serialized_name() {
        let model = LocalModel::GgufLlm(hypr_local_model::GgufLlmModel::Llama3p2_3bQ4);
        let mut current = empty_settings();
        current.current_llm_model = Some("Llama3p2_3bQ4".to_string());

        assert!(is_current_model(&model, &current));
    }
}
