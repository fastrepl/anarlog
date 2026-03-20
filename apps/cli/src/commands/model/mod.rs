pub(crate) mod action;
pub(crate) mod app;
pub(crate) mod effect;
pub(crate) mod list;
pub(crate) mod runtime;
mod screen;
pub(crate) mod ui;

use std::io::IsTerminal;
use std::sync::Arc;
use std::time::Duration;

use hypr_cli_tui::run_screen_inline;
use hypr_local_model::{LocalModel, LocalModelKind};
use hypr_local_stt_core::SUPPORTED_MODELS as SUPPORTED_STT_MODELS;
use hypr_model_downloader::ModelDownloadManager;
use sqlx::SqlitePool;
use tokio::sync::mpsc;

use clap::{Subcommand, ValueEnum};

use crate::cli::OutputFormat;
use crate::config::paths as settings;
use crate::error::{CliError, CliResult, did_you_mean};
use runtime::CliModelRuntime;

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Show resolved paths for settings and model storage
    Paths,
    /// List available models and their download status
    List {
        #[arg(long, value_enum)]
        kind: Option<ModelKind>,
        #[arg(long)]
        supported: bool,
        #[arg(short = 'f', long, value_enum, default_value = "text")]
        format: OutputFormat,
    },
    /// Manage downloadable Cactus models
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    Cactus {
        #[command(subcommand)]
        command: CactusCommands,
    },
    /// Download a model by name
    Download { name: String },
    /// Delete a downloaded model
    Delete { name: String },
}

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
#[derive(Subcommand, Debug)]
pub enum CactusCommands {
    /// List available Cactus models
    List {
        #[arg(short = 'f', long, value_enum, default_value = "text")]
        format: OutputFormat,
    },
    /// Download a Cactus model by name
    Download { name: String },
    /// Delete a downloaded Cactus model
    Delete { name: String },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ModelKind {
    Stt,
    Llm,
}

// ---------------------------------------------------------------------------
// ModelScope – a filtered set of models with metadata for error messages
// ---------------------------------------------------------------------------

struct ModelScope {
    models: Vec<LocalModel>,
    label: &'static str,
    list_cmd: &'static str,
}

impl ModelScope {
    fn all(kind: Option<ModelKind>) -> Self {
        Self {
            models: LocalModel::all()
                .into_iter()
                .filter(|m| model_is_enabled(m) && matches_kind(m, kind))
                .collect(),
            label: "model",
            list_cmd: "char models list",
        }
    }

    fn supported(kind: Option<ModelKind>) -> CliResult<Self> {
        match kind {
            Some(ModelKind::Stt) => Ok(Self {
                models: SUPPORTED_STT_MODELS
                    .iter()
                    .filter(|m| model_is_enabled(m))
                    .cloned()
                    .collect(),
                label: "model",
                list_cmd: "char models list",
            }),
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

    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    fn cactus() -> Self {
        Self {
            models: LocalModel::all()
                .into_iter()
                .filter(|m| m.cli_name().starts_with("cactus-"))
                .collect(),
            label: "cactus model",
            list_cmd: "char models cactus list",
        }
    }

    fn resolve(&self, name: &str) -> CliResult<LocalModel> {
        self.models
            .iter()
            .find(|m| m.cli_name() == name)
            .cloned()
            .ok_or_else(|| {
                let names: Vec<&str> = self.models.iter().map(|m| m.cli_name()).collect();
                let mut hint = String::new();
                if let Some(suggestion) = did_you_mean(name, &names) {
                    hint.push_str(&format!("Did you mean '{suggestion}'?\n\n"));
                }
                hint.push_str(&format!("Run `{}` to see available models.", self.list_cmd));
                CliError::not_found(format!("{} '{name}'", self.label), Some(hint))
            })
    }
}

// ---------------------------------------------------------------------------
// Command dispatch
// ---------------------------------------------------------------------------

pub async fn run(command: Commands, pool: &SqlitePool) -> CliResult<()> {
    let paths = settings::resolve_paths();
    let models_base = paths.models_base.clone();
    let db_path = paths.base.join("app.db");

    match command {
        Commands::Paths => {
            println!("base={}", paths.base.display());
            println!("db_path={}", db_path.display());
            println!("models_base={}", models_base.display());
            Ok(())
        }
        Commands::List {
            kind,
            supported,
            format,
        } => {
            let scope = if supported {
                ModelScope::supported(kind)?
            } else {
                ModelScope::all(kind)
            };
            list_models(&scope, pool, &models_base, format).await
        }
        #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
        Commands::Cactus { command } => run_cactus(command, pool, &models_base).await,
        Commands::Download { name } => {
            let model = ModelScope::all(None).resolve(&name)?;
            download_model(model, &models_base).await
        }
        Commands::Delete { name } => {
            let model = ModelScope::all(None).resolve(&name)?;
            delete_model(model, &models_base).await
        }
    }
}

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
async fn run_cactus(
    command: CactusCommands,
    pool: &SqlitePool,
    models_base: &std::path::Path,
) -> CliResult<()> {
    let scope = ModelScope::cactus();

    match command {
        CactusCommands::List { format } => list_models(&scope, pool, models_base, format).await,
        CactusCommands::Download { name } => {
            let name = normalize_cactus_name(&name);
            download_model(scope.resolve(&name)?, models_base).await
        }
        CactusCommands::Delete { name } => {
            let name = normalize_cactus_name(&name);
            delete_model(scope.resolve(&name)?, models_base).await
        }
    }
}

// ---------------------------------------------------------------------------
// Operations
// ---------------------------------------------------------------------------

async fn list_models(
    scope: &ModelScope,
    pool: &SqlitePool,
    models_base: &std::path::Path,
    format: OutputFormat,
) -> CliResult<()> {
    let manager = make_manager(models_base, None);
    let current = settings::load_settings_from_db(pool).await;
    let rows = list::collect_model_rows(&scope.models, models_base, &current, &manager).await;
    list::write_model_output(&rows, models_base, format).await
}

async fn download_model(model: LocalModel, models_base: &std::path::Path) -> CliResult<()> {
    let show_progress = std::io::stderr().is_terminal();

    let (progress_tx, progress_rx) = if show_progress {
        let (tx, rx) = mpsc::unbounded_channel();
        (Some(tx), Some(rx))
    } else {
        (None, None)
    };

    let manager = make_manager(models_base, progress_tx);

    if manager.is_downloaded(&model).await.unwrap_or(false) {
        println!(
            "Model already downloaded: {} ({})",
            model.display_name(),
            model.install_path(models_base).display()
        );
        return Ok(());
    }

    if let Err(e) = manager.download(&model).await {
        return Err(CliError::operation_failed(
            "start model download",
            format!("{}: {e}", model.cli_name()),
        ));
    }

    if let Some(progress_rx) = progress_rx {
        let screen = screen::DownloadScreen::new(model.cli_name().to_string());
        let height = screen.viewport_height();
        let success = run_screen_inline(screen, height, Some(progress_rx))
            .await
            .map_err(|e| CliError::operation_failed("download tui", e.to_string()))?;

        if success {
            println!(
                "Downloaded {} -> {}",
                model.display_name(),
                model.install_path(models_base).display()
            );
            Ok(())
        } else {
            Err(CliError::operation_failed(
                "download model",
                model.cli_name().to_string(),
            ))
        }
    } else {
        while manager.is_downloading(&model).await {
            tokio::time::sleep(Duration::from_millis(120)).await;
        }

        if manager.is_downloaded(&model).await.unwrap_or(false) {
            println!(
                "Downloaded {} -> {}",
                model.display_name(),
                model.install_path(models_base).display()
            );
            Ok(())
        } else {
            Err(CliError::operation_failed(
                "download model",
                model.cli_name().to_string(),
            ))
        }
    }
}

async fn delete_model(model: LocalModel, models_base: &std::path::Path) -> CliResult<()> {
    let manager = make_manager(models_base, None);

    if let Err(e) = manager.delete(&model).await {
        return Err(CliError::operation_failed(
            "delete model",
            format!("{}: {e}", model.cli_name()),
        ));
    }

    println!("Deleted {}", model.display_name());
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_manager(
    models_base: &std::path::Path,
    progress_tx: Option<mpsc::UnboundedSender<runtime::DownloadEvent>>,
) -> ModelDownloadManager<LocalModel> {
    let runtime = Arc::new(CliModelRuntime {
        models_base: models_base.to_path_buf(),
        progress_tx,
    });
    ModelDownloadManager::new(runtime)
}

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
fn normalize_cactus_name(name: &str) -> String {
    if name.starts_with("cactus-") {
        name.to_string()
    } else {
        format!("cactus-{name}")
    }
}

pub(crate) fn model_is_enabled(model: &LocalModel) -> bool {
    cfg!(any(target_arch = "arm", target_arch = "aarch64"))
        || !matches!(model, LocalModel::Cactus(_) | LocalModel::CactusLlm(_))
}

fn matches_kind(model: &LocalModel, kind: Option<ModelKind>) -> bool {
    match kind {
        None => true,
        Some(ModelKind::Stt) => model.model_kind() == LocalModelKind::Stt,
        Some(ModelKind::Llm) => model.model_kind() == LocalModelKind::Llm,
    }
}

fn is_current_model(model: &LocalModel, current: &settings::Settings) -> bool {
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

    fn empty_settings() -> settings::Settings {
        settings::Settings {
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
