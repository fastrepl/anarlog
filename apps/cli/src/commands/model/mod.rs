mod delete;
mod download;
pub(crate) mod list;
mod paths;
pub(crate) mod runtime;

use std::sync::Arc;

use hypr_local_model::{LocalModel, LocalModelKind};
use hypr_local_stt_core::SUPPORTED_MODELS as SUPPORTED_STT_MODELS;

use hypr_model_downloader::ModelDownloadManager;
use tokio::sync::mpsc;

use clap::Subcommand;

use crate::cli::OutputFormat;
use crate::config::paths as config_paths;
use crate::error::{CliError, CliResult, did_you_mean};
use runtime::CliModelRuntime;

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Show resolved paths for settings and model storage
    Paths,
    /// List available models and their download status
    List {
        #[arg(long)]
        supported: bool,
        #[arg(short = 'f', long, value_enum, default_value = "pretty")]
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
    Delete {
        name: String,
        #[arg(short = 'f', long)]
        force: bool,
    },
}

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
#[derive(Subcommand, Debug)]
pub enum CactusCommands {
    /// List available Cactus models
    List {
        #[arg(short = 'f', long, value_enum, default_value = "pretty")]
        format: OutputFormat,
    },
    /// Download a Cactus model by name
    Download { name: String },
    /// Delete a downloaded Cactus model
    Delete {
        name: String,
        #[arg(short = 'f', long)]
        force: bool,
    },
}

struct ModelScope {
    models: Vec<LocalModel>,
    label: &'static str,
    list_cmd: &'static str,
}

impl ModelScope {
    fn all() -> Self {
        Self {
            models: LocalModel::all()
                .into_iter()
                .filter(|m| model_is_enabled(m) && m.model_kind() == LocalModelKind::Stt)
                .collect(),
            label: "model",
            list_cmd: "char models list",
        }
    }

    fn supported() -> Self {
        Self {
            models: SUPPORTED_STT_MODELS
                .iter()
                .filter(|m| model_is_enabled(m))
                .cloned()
                .collect(),
            label: "model",
            list_cmd: "char models list",
        }
    }

    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    fn cactus() -> Self {
        Self {
            models: LocalModel::all()
                .into_iter()
                .filter(|m| {
                    m.cli_name().starts_with("cactus-") && m.model_kind() == LocalModelKind::Stt
                })
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

pub async fn run(command: Commands, trace_buffer: crate::OptTraceBuffer) -> CliResult<()> {
    let resolved = config_paths::resolve_paths();
    let models_base = resolved.models_base.clone();
    let db_path = resolved.base.join("app.db");

    match command {
        Commands::Paths => paths::paths(&resolved.base, &db_path, &models_base),
        Commands::List { supported, format } => {
            let scope = if supported {
                ModelScope::supported()
            } else {
                ModelScope::all()
            };
            list_models(&scope, &models_base, format).await
        }
        #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
        Commands::Cactus { command } => run_cactus(command, &models_base, trace_buffer).await,
        Commands::Download { name } => {
            let model = ModelScope::all().resolve(&name)?;
            download::download(model, &models_base, trace_buffer).await
        }
        Commands::Delete { name, force } => {
            let model = ModelScope::all().resolve(&name)?;
            delete::delete(model, &models_base, force).await
        }
    }
}

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
async fn run_cactus(
    command: CactusCommands,
    models_base: &std::path::Path,
    trace_buffer: crate::OptTraceBuffer,
) -> CliResult<()> {
    let scope = ModelScope::cactus();

    match command {
        CactusCommands::List { format } => list_models(&scope, models_base, format).await,
        CactusCommands::Download { name } => {
            let name = normalize_cactus_name(&name);
            download::download(scope.resolve(&name)?, models_base, trace_buffer).await
        }
        CactusCommands::Delete { name, force } => {
            let name = normalize_cactus_name(&name);
            delete::delete(scope.resolve(&name)?, models_base, force).await
        }
    }
}

async fn list_models(
    scope: &ModelScope,
    models_base: &std::path::Path,
    format: OutputFormat,
) -> CliResult<()> {
    let manager = make_manager(models_base, None);
    let rows = list::collect_model_rows(&scope.models, models_base, &manager).await;
    list::write_model_output(&rows, models_base, format).await
}

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
