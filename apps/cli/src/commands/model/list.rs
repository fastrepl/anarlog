use std::io::IsTerminal;
use std::path::Path;

use comfy_table::{Cell, Color, ContentArrangement, Table, presets::UTF8_FULL_CONDENSED};
use hypr_local_model::LocalModel;
use hypr_model_downloader::{DownloadableModel, ModelDownloadManager};

use crate::commands::OutputFormat;
use crate::config::desktop as settings;
use crate::error::CliResult;

#[derive(Clone, Debug, serde::Serialize)]
pub(super) struct ModelRow {
    name: String,
    kind: String,
    status: String,
    display_name: String,
    description: String,
    active: bool,
    install_path: String,
}

pub(super) async fn collect_model_rows(
    models: &[LocalModel],
    models_base: &Path,
    current: &Option<settings::DesktopSettings>,
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

        let active = current
            .as_ref()
            .is_some_and(|value| super::is_current_model(model, value));

        rows.push(ModelRow {
            name: model.cli_name().to_string(),
            kind: model.kind().to_string(),
            status: status.to_string(),
            display_name: model.display_name().to_string(),
            description: model.description().to_string(),
            active,
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
    if matches!(format, OutputFormat::Json) {
        crate::output::write_json(None, &rows).await?;
        return Ok(());
    }

    print_model_rows_table(models_base, rows)
}

fn print_model_rows_table(models_base: &Path, rows: &[ModelRow]) -> CliResult<()> {
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
