mod delete;
mod download;
pub(crate) mod list;
pub(crate) mod runtime;

use std::sync::Arc;

use hypr_local_model::{LocalModel, LocalModelKind};
use hypr_model_downloader::ModelDownloadManager;
use tokio::sync::mpsc;

use clap::Subcommand;

use crate::app::AppContext;
use crate::cli::OutputFormat;
use crate::error::{CliError, CliResult, did_you_mean};
use runtime::CliModelRuntime;

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long, env = "CHAR_BASE", hide_env_values = true, value_name = "DIR")]
    pub base: Option<std::path::PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// List local models and their status
    List {
        #[arg(short = 'f', long, value_enum, default_value = "pretty")]
        format: OutputFormat,
    },
    /// Download a model by name
    Download { name: String },
    /// Delete a downloaded model
    Delete {
        name: String,
        #[arg(long)]
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
                .filter(|m| m.model_kind() == LocalModelKind::Stt)
                .filter(model_is_enabled)
                .collect(),
            label: "model",
            list_cmd: "char models list",
        }
    }

    fn resolve(&self, name: &str) -> CliResult<LocalModel> {
        self.models
            .iter()
            .find(|m| m.cli_name() == name)
            .or_else(|| {
                if name.starts_with("cactus-") {
                    None
                } else {
                    let cactus_name = format!("cactus-{name}");
                    self.models.iter().find(|m| m.cli_name() == cactus_name)
                }
            })
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

pub async fn run(ctx: &AppContext, args: Args) -> CliResult<()> {
    let resolved = ctx.paths();
    let models_base = resolved.models_base.clone();

    match args.command {
        Commands::List { format } => list_models(&ModelScope::all(), &models_base, format).await,
        Commands::Download { name } => {
            let model = ModelScope::all().resolve(&name)?;
            download::download(model, &models_base, ctx.trace_buffer()).await
        }
        Commands::Delete { name, force } => {
            let model = ModelScope::all().resolve(&name)?;
            delete::delete(model, &models_base, force).await
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

pub(crate) fn model_is_enabled(model: &LocalModel) -> bool {
    if matches!(model, LocalModel::Am(_)) {
        return false;
    }
    cfg!(all(
        target_os = "macos",
        any(target_arch = "arm", target_arch = "aarch64")
    )) || !matches!(model, LocalModel::Cactus(_) | LocalModel::CactusLlm(_))
}
