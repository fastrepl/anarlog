use std::io::IsTerminal;
use std::path::Path;

use hypr_local_model::LocalModel;
use hypr_model_downloader::{DownloadableModel, ModelDownloadManager};
use ratatui::layout::Constraint;
use ratatui::style::{Modifier, Style};
use ratatui::text::Text;
use ratatui::widgets::{Cell, Row, Table};

use crate::cli::OutputFormat;
use crate::config::paths as settings;
use crate::error::CliResult;
use crate::widgets::InlineBox;

#[derive(Clone, Debug, serde::Serialize)]
pub(crate) struct ModelRow {
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) status: String,
    pub(crate) display_name: String,
    pub(crate) description: String,
    pub(crate) active: bool,
    pub(crate) install_path: String,
}

pub(crate) async fn collect_model_rows(
    models: &[LocalModel],
    models_base: &Path,
    current: &Option<settings::Settings>,
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
    use crate::theme::Theme;
    let theme = Theme::DEFAULT;

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

    let dim = Style::default().add_modifier(Modifier::DIM);

    let header = Row::new(["", "Name", "Kind", "Status", "Model", "Description"]).style(dim);

    let table_rows: Vec<Row> = rows
        .iter()
        .map(|row| {
            let active = if row.active {
                Cell::from(Text::raw("*"))
            } else {
                Cell::from("")
            };

            let status_cell = match row.status.as_str() {
                "downloaded" => Cell::from(row.status.as_str()).style(theme.status_active),
                "not-downloaded" => Cell::from(row.status.as_str()).style(theme.status_degraded),
                "unavailable" => Cell::from(row.status.as_str()).style(theme.muted),
                "error" => Cell::from(row.status.as_str()).style(theme.error),
                _ => Cell::from(row.status.as_str()),
            };

            Row::new([
                active,
                Cell::from(row.name.as_str()),
                Cell::from(row.kind.as_str()),
                status_cell,
                Cell::from(row.display_name.as_str()),
                Cell::from(row.description.as_str()),
            ])
        })
        .collect();

    let content_lines = (table_rows.len() + 1) as u16;
    let height = InlineBox::viewport_height(content_lines);

    let widths = [
        Constraint::Length(1),
        Constraint::Length(20),
        Constraint::Length(8),
        Constraint::Length(14),
        Constraint::Min(16),
        Constraint::Min(12),
    ];
    let table = Table::new(table_rows, widths).header(header);

    println!();
    let _ = hypr_cli_tui::render_inline(height, |frame| {
        let inner = InlineBox::render(frame);
        frame.render_widget(table, inner);
    });
    Ok(())
}
