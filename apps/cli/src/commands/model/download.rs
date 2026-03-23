use std::io::{IsTerminal, Stderr};
use std::path::Path;
use std::time::Duration;

use hypr_local_model::LocalModel;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{LineGauge, Paragraph};
use ratatui::{Terminal, TerminalOptions, Viewport};
use tokio::sync::mpsc;

use super::runtime;
use crate::error::{CliError, CliResult};

const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub(super) async fn download(model: LocalModel, models_base: &Path) -> CliResult<()> {
    let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();

    let manager = super::make_manager(models_base, Some(progress_tx));

    if manager.is_downloaded(&model).await.unwrap_or(false) {
        eprintln!(
            "Model already downloaded: {} ({})",
            model.display_name(),
            model.install_path(models_base).display()
        );
        return Ok(());
    }

    let mut terminal = if std::io::stderr().is_terminal() {
        Some(
            Terminal::with_options(
                CrosstermBackend::new(std::io::stderr()),
                TerminalOptions {
                    viewport: Viewport::Inline(2),
                },
            )
            .map_err(|e| CliError::operation_failed("init download viewport", e.to_string()))?,
        )
    } else {
        None
    };

    if let Err(e) = manager.download(&model).await {
        drop(terminal);
        return Err(CliError::operation_failed(
            "start model download",
            format!("{}: {e}", model.cli_name()),
        ));
    }

    let mut pct: u8 = 0;
    let mut spinner_idx: usize = 0;
    let mut tick = tokio::time::interval(Duration::from_millis(80));

    loop {
        let done = tokio::select! {
            event = progress_rx.recv() => {
                match event {
                    Some(runtime::DownloadEvent::Progress(p)) => { pct = p; false }
                    Some(runtime::DownloadEvent::Completed | runtime::DownloadEvent::Failed) | None => true,
                }
            }
            _ = tick.tick() => {
                spinner_idx = (spinner_idx + 1) % SPINNER.len();
                false
            }
        };

        draw_download(&mut terminal, &model, spinner_idx, pct);

        if done {
            break;
        }
    }

    while manager.is_downloading(&model).await {
        tokio::time::sleep(Duration::from_millis(120)).await;
    }

    drop(terminal);

    if manager.is_downloaded(&model).await.unwrap_or(false) {
        eprintln!(
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

fn draw_download(
    terminal: &mut Option<Terminal<CrosstermBackend<Stderr>>>,
    model: &LocalModel,
    spinner_idx: usize,
    pct: u8,
) {
    if let Some(term) = terminal {
        let name = model.display_name();
        let _ = term.draw(|frame| {
            let chunks = Layout::vertical([Constraint::Length(1), Constraint::Length(1)])
                .split(frame.area());

            let status = Paragraph::new(Line::from(format!(
                "{} Downloading {}...",
                SPINNER[spinner_idx], name
            )))
            .style(Style::default().fg(Color::Cyan));
            frame.render_widget(status, chunks[0]);

            let gauge = LineGauge::default()
                .ratio(pct as f64 / 100.0)
                .label(format!("{}%", pct))
                .filled_style(Style::default().fg(Color::Cyan));
            frame.render_widget(gauge, chunks[1]);
        });
    }
}
