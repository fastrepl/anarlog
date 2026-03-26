use std::path::Path;
use std::time::Duration;

use hypr_local_model::LocalModel;
use tokio::sync::mpsc;

use super::runtime;
use crate::error::{CliError, CliResult};
use crate::tui::{InlineViewport, SPINNER};

pub(super) async fn download(
    model: LocalModel,
    models_base: &Path,
    trace_buffer: crate::OptTraceBuffer,
) -> CliResult<()> {
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

    let mut viewport = if trace_buffer.is_some() {
        InlineViewport::stderr(5, trace_buffer).ok()
    } else {
        None
    };

    if let Err(e) = manager.download(&model).await {
        drop(viewport);
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

        draw_download(&mut viewport, &model, spinner_idx, pct);

        if done {
            break;
        }
    }

    while manager.is_downloading(&model).await {
        tokio::time::sleep(Duration::from_millis(120)).await;
    }

    if let Some(ref mut vp) = viewport {
        vp.clear()
            .map_err(|e| CliError::operation_failed("clear viewport", e.to_string()))?;
    }
    drop(viewport);

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
    viewport: &mut Option<InlineViewport>,
    model: &LocalModel,
    spinner_idx: usize,
    pct: u8,
) {
    if let Some(vp) = viewport {
        vp.poll_toggle();
        let name = model.display_name();
        let pct_str = format!("{}%", pct);
        vp.draw(&[
            format!(
                "{} Downloading {}... {}",
                SPINNER[spinner_idx], name, pct_str
            ),
            format_gauge(pct),
        ]);
    }
}

fn format_gauge(pct: u8) -> String {
    let width = 40;
    let filled = (pct as usize * width) / 100;
    let empty = width - filled;
    format!("  [{}{}]", "█".repeat(filled), "░".repeat(empty),)
}
