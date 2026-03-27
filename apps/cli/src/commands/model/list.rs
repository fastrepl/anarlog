use std::path::Path;

use comfy_table::{ContentArrangement, Table, presets::UTF8_FULL_CONDENSED};
use hypr_local_model::LocalModel;
use hypr_model_downloader::{DownloadableModel, ModelDownloadManager};

use crate::cli::OutputFormat;
use crate::error::CliResult;

#[derive(Clone, Debug, serde::Serialize)]
pub(crate) struct ModelRow {
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) status: String,
    pub(crate) downloaded: bool,
    pub(crate) downloadable: bool,
    pub(crate) available_on_current_platform: bool,
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
        let downloadable = model.download_url().is_some();
        let available_on_current_platform = model.is_available_on_current_platform();

        let status = match manager.is_downloaded(model).await {
            Ok(true) => "downloaded",
            Ok(false) if downloadable && available_on_current_platform => "available",
            Ok(false) if downloadable => "unsupported",
            Ok(false) => continue,
            Err(_) => "error",
        };

        rows.push(ModelRow {
            name: model.cli_name().to_string(),
            kind: model.kind().to_string(),
            status: status.to_string(),
            downloaded: status == "downloaded",
            downloadable,
            available_on_current_platform,
            display_name: model.display_name().to_string(),
            description: model.description().to_string(),
            install_path: model.install_path(models_base).display().to_string(),
        });
    }
    rows.sort_by(|a, b| {
        status_rank(&a.status)
            .cmp(&status_rank(&b.status))
            .then_with(|| a.name.cmp(&b.name))
    });
    rows
}

pub(super) async fn write_model_output(
    rows: &[ModelRow],
    _models_base: &Path,
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

            let home = dirs::home_dir();

            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL_CONDENSED)
                .set_content_arrangement(ContentArrangement::Dynamic);

            table.set_header(vec!["Name", "Status", "Path"]);

            for row in rows {
                let path = match &home {
                    Some(h) => row
                        .install_path
                        .strip_prefix(&h.display().to_string())
                        .map(|rest| format!("~{rest}"))
                        .unwrap_or_else(|| row.install_path.clone()),
                    None => row.install_path.clone(),
                };
                table.add_row(vec![row.name.clone(), row.status.clone(), path]);
            }

            println!("{table}");
        }
    }
    Ok(())
}

fn status_rank(status: &str) -> usize {
    match status {
        "downloaded" => 0,
        "available" => 1,
        "unsupported" => 2,
        _ => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, kind: &str, status: &str) -> ModelRow {
        ModelRow {
            name: name.to_string(),
            kind: kind.to_string(),
            status: status.to_string(),
            downloaded: status == "downloaded",
            downloadable: status != "unavailable",
            available_on_current_platform: status != "unsupported",
            display_name: name.to_string(),
            description: String::new(),
            install_path: format!("/tmp/{name}"),
        }
    }

    #[test]
    fn sorts_downloaded_before_available_and_unsupported() {
        let mut rows = vec![
            row("model-b", "llm", "unsupported"),
            row("model-c", "llm", "available"),
            row("model-a", "stt-whisper", "downloaded"),
        ];

        rows.sort_by(|a, b| {
            status_rank(&a.status)
                .cmp(&status_rank(&b.status))
                .then_with(|| a.name.cmp(&b.name))
        });

        assert_eq!(
            rows.iter().map(|row| row.name.as_str()).collect::<Vec<_>>(),
            vec!["model-a", "model-c", "model-b"]
        );
    }
}
