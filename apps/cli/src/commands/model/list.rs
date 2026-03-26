use std::path::Path;

use hypr_local_model::LocalModel;
use hypr_model_downloader::{DownloadableModel, ModelDownloadManager};

use crate::cli::OutputFormat;
use crate::error::CliResult;

#[derive(Clone, Debug, serde::Serialize)]
pub(crate) struct ModelRow {
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) display_name: String,
    pub(crate) description: String,
    pub(crate) install_path: String,
}

pub(crate) async fn collect_model_rows(
    models: &[LocalModel],
    models_base: &Path,
    manager: &ModelDownloadManager<LocalModel>,
) -> Vec<ModelRow> {
    let mut rows = Vec::new();
    for model in models {
        let status = match manager.is_downloaded(model).await {
            Ok(true) => "downloaded",
            Ok(false) if model.download_url().is_some() => "not-downloaded",
            Ok(false) => "unavailable",
            Err(_) => "error",
        };

        rows.push(ModelRow {
            name: model.cli_name().to_string(),
            status: status.to_string(),
            display_name: model.display_name().to_string(),
            description: model.description().to_string(),
            install_path: model.install_path(models_base).display().to_string(),
        });
    }
    rows
}

pub(super) async fn write_model_output(
    rows: &[ModelRow],
    models_base: &Path,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => {
            crate::output::write_json(None, &rows).await?;
        }
        OutputFormat::Pretty => {
            if rows.is_empty() {
                eprintln!("No models found.");
                return Ok(());
            }

            let name_w = rows.iter().map(|r| r.name.len()).max().unwrap_or(4).max(4);
            let status_w = rows
                .iter()
                .map(|r| r.status.len())
                .max()
                .unwrap_or(6)
                .max(6);

            println!("{:<name_w$}  {:<status_w$}  DISPLAY NAME", "NAME", "STATUS",);
            for row in rows {
                let label = if row.description.is_empty() {
                    row.display_name.clone()
                } else {
                    format!("{} ({})", row.display_name, row.description)
                };
                println!(
                    "{:<name_w$}  {:<status_w$}  {}",
                    row.name, row.status, label,
                );
            }
        }
    }
    Ok(())
}
