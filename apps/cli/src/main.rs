mod app;
mod cli;
mod commands;
mod config;
mod error;
mod output;
mod stt;
#[cfg(feature = "standalone")]
pub(crate) mod tui;

use crate::cli::{Cli, Commands};
use crate::error::CliResult;
use clap::Parser;

#[allow(clippy::let_unit_value)]
fn main() {
    let cli = Cli::parse();

    if cli.no_color || std::env::var_os("NO_COLOR").is_some() {
        colored::control::set_override(false);
    }

    let trace_buffer = init_tracing(&cli);

    #[cfg(all(feature = "standalone", target_os = "macos"))]
    let result = if matches!(&cli.command, Some(Commands::ShortcutDaemon)) {
        crate::commands::shortcut::daemon::run_blocking()
    } else {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime");
        runtime.block_on(run(cli, trace_buffer))
    };

    #[cfg(not(all(feature = "standalone", target_os = "macos")))]
    let result = {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime");
        runtime.block_on(run(cli, trace_buffer))
    };

    if let Err(error) = result {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

#[cfg(feature = "standalone")]
type OptTraceBuffer = Option<tui::TraceBuffer>;
#[cfg(not(feature = "standalone"))]
type OptTraceBuffer = ();

fn init_tracing(cli: &Cli) -> OptTraceBuffer {
    let level = cli.verbose.tracing_level_filter();

    let wants_json = matches!(
        cli.command,
        Some(Commands::Transcribe {
            args: commands::transcribe::Args {
                format: cli::OutputFormat::Json,
                ..
            },
        })
    );

    #[cfg(feature = "standalone")]
    let wants_json = wants_json
        || matches!(
            cli.command,
            Some(Commands::Record {
                args: commands::record::Args {
                    format: cli::OutputFormat::Json,
                    ..
                },
            })
        );

    #[cfg(feature = "standalone")]
    let wants_capture = !wants_json
        && std::io::IsTerminal::is_terminal(&std::io::stderr())
        && matches!(
            cli.command,
            Some(
                Commands::Transcribe { .. }
                    | Commands::Models { .. }
                    | Commands::Record { .. }
                    | Commands::Play { .. },
            )
        );

    #[cfg(feature = "standalone")]
    if wants_capture {
        let buf = tui::new_trace_buffer();
        init_tracing_capture(level, buf.clone());
        return Some(buf);
    }

    if wants_json {
        init_tracing_json(level);
    } else {
        init_tracing_stderr(level);
    }

    #[cfg(feature = "standalone")]
    return None;
    #[cfg(not(feature = "standalone"))]
    return;
}

fn init_tracing_stderr(level: tracing_subscriber::filter::LevelFilter) {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

fn init_tracing_json(level: tracing_subscriber::filter::LevelFilter) {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env_lossy();
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

#[cfg(feature = "standalone")]
fn init_tracing_capture(level: tracing_subscriber::filter::LevelFilter, buffer: tui::TraceBuffer) {
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let filter = EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env_lossy();
    let capture = tui::CaptureLayer::new(buffer);
    tracing_subscriber::registry()
        .with(filter)
        .with(capture)
        .init();
}

async fn run(cli: Cli, trace_buffer: OptTraceBuffer) -> CliResult<()> {
    let base = cli
        .command
        .as_ref()
        .and_then(Commands::base_override)
        .map(std::path::Path::to_path_buf);
    let tracked = cli.command.as_ref().map(Into::into);
    let Cli {
        command, verbose, ..
    } = cli;
    let ctx = app::AppContext::new(base.as_deref(), verbose.is_silent(), trace_buffer);

    if let Some(subcommand) = tracked {
        ctx.track_command(subcommand);
    }

    commands::run(&ctx, command).await
}
