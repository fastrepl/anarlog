use std::io::{IsTerminal, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use clap::{Subcommand, ValueEnum};
use comfy_table::{Cell, Color, ContentArrangement, Table, presets::UTF8_FULL_CONDENSED};
use hypr_local_model::{LocalModel, LocalModelKind};
use hypr_local_stt_core::SUPPORTED_MODELS as SUPPORTED_STT_MODELS;
use hypr_model_downloader::{DownloadableModel, ModelDownloadManager};
use indicatif::{ProgressBar, ProgressStyle};

use crate::commands::OutputFormat;
use crate::error::{CliError, CliResult};

mod runtime;
pub(crate) mod settings;

use runtime::CliModelRuntime;

#[derive(Subcommand, Debug)]
pub enum ModelCommands {
    Paths,
    Current,
    List {
        #[arg(long, value_enum)]
        kind: Option<ModelKind>,
        #[arg(long)]
        supported: bool,
        #[arg(long, value_enum, default_value = "text")]
        format: OutputFormat,
        #[arg(long, hide = true, conflicts_with = "format")]
        json: bool,
    },
    #[command(about = "Manage downloadable Cactus models")]
    Cactus {
        #[command(subcommand)]
        command: CactusCommands,
    },
    Download {
        name: String,
    },
    Delete {
        name: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum CactusCommands {
    List {
        #[arg(long, value_enum, default_value = "text")]
        format: OutputFormat,
        #[arg(long, hide = true, conflicts_with = "format")]
        json: bool,
    },
    Download {
        name: String,
    },
    Delete {
        name: String,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ModelKind {
    Stt,
    Llm,
}

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
            json,
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

            let mut rows = Vec::new();
            for model in &models {
                let status = match manager.is_downloaded(model).await {
                    Ok(true) => "downloaded",
                    Ok(false) => {
                        if model.download_url().is_some() {
                            "not-downloaded"
                        } else {
                            "unavailable"
                        }
                    }
                    Err(_) => "error",
                };

                let active = current
                    .as_ref()
                    .is_some_and(|value| is_current_model(model, value));

                rows.push(ModelRow {
                    active,
                    name: model.cli_name().to_string(),
                    kind: model.kind().to_string(),
                    status: status.to_string(),
                    display_name: model.display_name().to_string(),
                    description: model.description().to_string(),
                    install_path: model.install_path(&models_base).display().to_string(),
                });
            }

            let format = if json { OutputFormat::Json } else { format };

            if matches!(format, OutputFormat::Json) {
                return print_model_rows_json(&models_base, &rows);
            }

            print_model_rows(&models_base, &rows)?;
            Ok(())
        }
        ModelCommands::Cactus { command } => {
            run_cactus(command, &paths.settings_path, &models_base).await
        }
        ModelCommands::Download { name } => {
            let Some(model) = find_model(&name) else {
                return Err(CliError::not_found(
                    format!("model '{name}'"),
                    Some("Run `char model list` to see available models.".to_string()),
                ));
            };

            let progress = make_download_progress_bar(&model);
            let runtime = Arc::new(CliModelRuntime {
                models_base: models_base.clone(),
                progress: progress.clone(),
            });
            let manager = ModelDownloadManager::new(runtime);

            if manager.is_downloaded(&model).await.unwrap_or(false) {
                println!(
                    "Model already downloaded: {} ({})",
                    model.display_name(),
                    model.install_path(&models_base).display()
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
                    model.install_path(&models_base).display()
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
        ModelCommands::Delete { name } => {
            let runtime = Arc::new(CliModelRuntime {
                models_base: models_base.clone(),
                progress: None,
            });
            let manager = ModelDownloadManager::new(runtime);

            let Some(model) = find_model(&name) else {
                return Err(CliError::not_found(
                    format!("model '{name}'"),
                    Some("Run `char model list` to see available models.".to_string()),
                ));
            };

            if let Err(e) = manager.delete(&model).await {
                return Err(CliError::operation_failed(
                    "delete model",
                    format!("{}: {e}", model.cli_name()),
                ));
            }

            println!("Deleted {}", model.display_name());
            Ok(())
        }
    }
}

async fn run_cactus(
    command: CactusCommands,
    settings_path: &std::path::Path,
    models_base: &std::path::Path,
) -> CliResult<()> {
    match command {
        CactusCommands::List { format, json } => {
            let runtime = Arc::new(CliModelRuntime {
                models_base: models_base.to_path_buf(),
                progress: None,
            });
            let manager = ModelDownloadManager::new(runtime);

            let current = settings::load_settings(settings_path);
            let models = all_cactus_models();

            let format = if json { OutputFormat::Json } else { format };

            if matches!(format, OutputFormat::Json) {
                #[derive(serde::Serialize)]
                struct Item {
                    name: String,
                    kind: String,
                    status: String,
                    display_name: String,
                    description: String,
                    active: bool,
                    install_path: String,
                }

                let mut items = Vec::with_capacity(models.len());
                for model in models {
                    let status = match manager.is_downloaded(&model).await {
                        Ok(true) => "downloaded",
                        Ok(false) => {
                            if model.download_url().is_some() {
                                "not-downloaded"
                            } else {
                                "unavailable"
                            }
                        }
                        Err(_) => "error",
                    };

                    let active = current
                        .as_ref()
                        .is_some_and(|value| is_current_model(&model, value));

                    items.push(Item {
                        name: model.cli_name().to_string(),
                        kind: model.kind().to_string(),
                        status: status.to_string(),
                        display_name: model.display_name().to_string(),
                        description: model.description().to_string(),
                        active,
                        install_path: model.install_path(models_base).display().to_string(),
                    });
                }

                let bytes = if std::io::stdout().is_terminal() {
                    serde_json::to_vec_pretty(&items)
                } else {
                    serde_json::to_vec(&items)
                }
                .map_err(|e| CliError::operation_failed("serialize model list", e.to_string()))?;

                std::io::stdout()
                    .write_all(&bytes)
                    .map_err(|e| CliError::operation_failed("write output", e.to_string()))?;
                std::io::stdout()
                    .write_all(b"\n")
                    .map_err(|e| CliError::operation_failed("write output", e.to_string()))?;
                return Ok(());
            }

            let mut rows = Vec::new();
            for model in models {
                let status = match manager.is_downloaded(&model).await {
                    Ok(true) => "downloaded",
                    Ok(false) => {
                        if model.download_url().is_some() {
                            "not-downloaded"
                        } else {
                            "unavailable"
                        }
                    }
                    Err(_) => "error",
                };

                let active = current
                    .as_ref()
                    .is_some_and(|value| is_current_model(&model, value));

                rows.push(ModelRow {
                    active,
                    name: model.cli_name().to_string(),
                    kind: model.kind().to_string(),
                    status: status.to_string(),
                    display_name: model.display_name().to_string(),
                    description: model.description().to_string(),
                    install_path: model.install_path(models_base).display().to_string(),
                });
            }

            print_model_rows(models_base, &rows)?;
            Ok(())
        }
        CactusCommands::Download { name } => {
            let Some(model) = find_cactus_model(&name) else {
                return Err(CliError::not_found(
                    format!("cactus model '{name}'"),
                    Some("Run `char model cactus list` to see available models.".to_string()),
                ));
            };

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
        CactusCommands::Delete { name } => {
            let runtime = Arc::new(CliModelRuntime {
                models_base: models_base.to_path_buf(),
                progress: None,
            });
            let manager = ModelDownloadManager::new(runtime);

            let Some(model) = find_cactus_model(&name) else {
                return Err(CliError::not_found(
                    format!("cactus model '{name}'"),
                    Some("Run `char model cactus list` to see available models.".to_string()),
                ));
            };

            if let Err(e) = manager.delete(&model).await {
                return Err(CliError::operation_failed(
                    "delete model",
                    format!("{}: {e}", model.cli_name()),
                ));
            }

            println!("Deleted {}", model.display_name());
            Ok(())
        }
    }
}

#[derive(Clone, Debug)]
struct ModelRow {
    active: bool,
    name: String,
    kind: String,
    status: String,
    display_name: String,
    description: String,
    install_path: String,
}

fn print_model_rows_json(_models_base: &Path, rows: &[ModelRow]) -> CliResult<()> {
    #[derive(serde::Serialize)]
    struct Item {
        name: String,
        kind: String,
        status: String,
        display_name: String,
        description: String,
        active: bool,
        install_path: String,
    }

    let items: Vec<Item> = rows
        .iter()
        .map(|row| Item {
            name: row.name.clone(),
            kind: row.kind.clone(),
            status: row.status.clone(),
            display_name: row.display_name.clone(),
            description: row.description.clone(),
            active: row.active,
            install_path: row.install_path.clone(),
        })
        .collect();

    let bytes = if std::io::stdout().is_terminal() {
        serde_json::to_vec_pretty(&items)
    } else {
        serde_json::to_vec(&items)
    }
    .map_err(|e| CliError::operation_failed("serialize model list", e.to_string()))?;

    std::io::stdout()
        .write_all(&bytes)
        .map_err(|e| CliError::operation_failed("write output", e.to_string()))?;
    std::io::stdout()
        .write_all(b"\n")
        .map_err(|e| CliError::operation_failed("write output", e.to_string()))?;
    Ok(())
}

fn print_model_rows(models_base: &Path, rows: &[ModelRow]) -> CliResult<()> {
    println!("models_base={}", models_base.display());

    if !std::io::stdout().is_terminal() {
        for row in rows {
            let active = if row.active { "*" } else { "" };
            if row.description.is_empty() {
                println!(
                    "{}\t{}\t{}\t{}\t{}",
                    active, row.name, row.kind, row.status, row.display_name,
                );
            } else {
                println!(
                    "{}\t{}\t{}\t{}\t{} ({})",
                    active, row.name, row.kind, row.status, row.display_name, row.description,
                );
            }
        }
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(["", "Name", "Kind", "Status", "Model", "Description"]);

    for row in rows {
        let active = if row.active {
            Cell::new("*")
        } else {
            Cell::new("")
        };

        let status_cell = match row.status.as_str() {
            "downloaded" => Cell::new(&row.status).fg(Color::Green),
            "not-downloaded" => Cell::new(&row.status).fg(Color::Yellow),
            "unavailable" => Cell::new(&row.status).fg(Color::DarkGrey),
            "error" => Cell::new(&row.status).fg(Color::Red),
            _ => Cell::new(&row.status),
        };

        table.add_row([
            active,
            Cell::new(&row.name),
            Cell::new(&row.kind),
            status_cell,
            Cell::new(&row.display_name),
            Cell::new(&row.description),
        ]);
    }

    println!();
    println!("{table}");
    Ok(())
}

fn make_download_progress_bar(model: &LocalModel) -> Option<ProgressBar> {
    if !std::io::stderr().is_terminal() {
        return None;
    }

    let bar = ProgressBar::new(100);
    bar.set_style(
        ProgressStyle::with_template("{spinner} {msg} [{wide_bar}] {pos:>3}%")
            .unwrap()
            .progress_chars("=>-"),
    );
    bar.set_message(format!("Downloading {}", model.cli_name()));
    bar.enable_steady_tick(Duration::from_millis(120));
    Some(bar)
}

fn find_model(name: &str) -> Option<LocalModel> {
    all_models(None)
        .into_iter()
        .find(|model| model.cli_name() == name)
}

fn all_cactus_models() -> Vec<LocalModel> {
    LocalModel::all()
        .into_iter()
        .filter(|model| model.cli_name().starts_with("cactus-"))
        .collect()
}

fn find_cactus_model(name: &str) -> Option<LocalModel> {
    let canonical = if name.starts_with("cactus-") {
        name.to_string()
    } else {
        format!("cactus-{name}")
    };

    all_cactus_models()
        .into_iter()
        .find(|model| model.cli_name() == name || model.cli_name() == canonical)
}

fn all_models(kind: Option<ModelKind>) -> Vec<LocalModel> {
    LocalModel::all()
        .into_iter()
        .filter(|model| matches_kind(model, kind))
        .collect()
}

fn supported_models(kind: Option<ModelKind>) -> CliResult<Vec<LocalModel>> {
    match kind {
        Some(ModelKind::Stt) => Ok(SUPPORTED_STT_MODELS.iter().cloned().collect()),
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
