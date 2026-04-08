pub mod transcribe;
pub(crate) mod update_check;

#[cfg(feature = "desktop")]
pub mod export;
#[cfg(feature = "desktop")]
pub mod humans;
#[cfg(feature = "todo")]
pub mod integration;
#[cfg(feature = "desktop")]
pub mod meetings;
#[cfg(feature = "desktop")]
pub mod orgs;
#[cfg(feature = "todo")]
pub mod todo;

#[cfg(feature = "standalone")]
pub mod bug;
#[cfg(feature = "standalone")]
pub mod desktop;
#[cfg(feature = "standalone")]
pub mod hello;
#[cfg(feature = "standalone")]
pub mod model;
#[cfg(feature = "standalone")]
pub mod play;
#[cfg(feature = "standalone")]
pub mod record;
#[cfg(all(feature = "standalone", target_os = "macos"))]
pub mod shortcut;
#[cfg(feature = "standalone")]
pub mod skill;
#[cfg(feature = "standalone")]
pub mod update;

use std::path::{Path, PathBuf};

use crate::app::AppContext;
use crate::cli::{Cli, Commands as CliCommand};
use crate::error::{CliError, CliResult};

pub(crate) fn resolve_session_dir(base: Option<&Path>, timestamp: &str) -> CliResult<PathBuf> {
    let base = base.map(Path::to_path_buf).unwrap_or_else(|| {
        dirs::data_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("char")
    });

    let mut dir = base.join(timestamp);
    if !dir.exists() {
        std::fs::create_dir_all(&dir)
            .map_err(|e| CliError::operation_failed("create session directory", e.to_string()))?;
        return Ok(dir);
    }

    for i in 1.. {
        dir = base.join(format!("{timestamp}-{i}"));
        if !dir.exists() {
            std::fs::create_dir_all(&dir).map_err(|e| {
                CliError::operation_failed("create session directory", e.to_string())
            })?;
            return Ok(dir);
        }
    }

    unreachable!()
}

pub async fn run(ctx: &AppContext, command: Option<CliCommand>) -> CliResult<()> {
    match command {
        Some(CliCommand::Transcribe { args }) => transcribe::run(ctx, args).await,
        #[cfg(feature = "standalone")]
        Some(CliCommand::Models { args }) => model::run(ctx, args).await,
        #[cfg(feature = "standalone")]
        Some(CliCommand::Play { args }) => play::run(ctx, args).await,
        #[cfg(feature = "standalone")]
        Some(CliCommand::Record { args }) => record::run(ctx, args).await,
        #[cfg(feature = "standalone")]
        Some(CliCommand::Skill { command }) => skill::run(ctx, command).await,
        Some(CliCommand::Completions { shell }) => {
            crate::cli::generate_completions(shell);
            Ok(())
        }
        #[cfg(feature = "standalone")]
        Some(CliCommand::Desktop) => {
            use desktop::DesktopAction;
            match desktop::run()? {
                DesktopAction::OpenedApp => eprintln!("Opened desktop app"),
                DesktopAction::OpenedDownloadPage => {
                    eprintln!("Desktop app not found — opened download page")
                }
            }
            Ok(())
        }
        #[cfg(feature = "standalone")]
        Some(CliCommand::Bug) => {
            bug::run()?;
            eprintln!("Opened bug report page in browser");
            Ok(())
        }
        #[cfg(feature = "standalone")]
        Some(CliCommand::Hello) => {
            hello::run()?;
            eprintln!("Opened char.com in browser");
            Ok(())
        }
        #[cfg(feature = "standalone")]
        Some(CliCommand::Update) => update::run(),
        #[cfg(all(feature = "standalone", target_os = "macos"))]
        Some(CliCommand::ShortcutDaemon) => shortcut::daemon::run().await,
        #[cfg(feature = "todo")]
        Some(CliCommand::Todo { command }) => todo::run(command).await,
        #[cfg(feature = "desktop")]
        Some(CliCommand::Meetings { command }) => meetings::run(ctx, command).await,
        #[cfg(feature = "desktop")]
        Some(CliCommand::Humans { command }) => humans::run(ctx, command).await,
        #[cfg(feature = "desktop")]
        Some(CliCommand::Orgs { command }) => orgs::run(ctx, command).await,
        #[cfg(feature = "desktop")]
        Some(CliCommand::Export { command }) => export::run(ctx, command).await,
        None => {
            use clap::CommandFactory;

            Cli::command().print_help().ok();
            println!();
            Ok(())
        }
    }
}
