use clap::Subcommand;

use crate::error::{CliError, CliResult};

const COMMAND: &str = "char claude notify";

#[derive(Subcommand)]
pub enum Commands {
    /// Receive a hook event from Claude Code (reads JSON from stdin)
    Notify,
    /// Install char as a Claude Code hook handler
    Install,
    /// Remove char from Claude Code hooks
    Uninstall,
}

pub async fn run(command: Commands) -> CliResult<()> {
    match command {
        Commands::Notify => notify(),
        Commands::Install => install(),
        Commands::Uninstall => uninstall(),
    }
}

fn notify() -> CliResult<()> {
    let event = super::read_stdin_json()?;

    // TODO: write to app DB
    super::print_pretty_json(&event)
}

fn install() -> CliResult<()> {
    let settings_path = hypr_claude::settings_path();

    let mut settings = hypr_claude::read_settings(&settings_path)
        .map_err(|e| CliError::operation_failed("read claude settings", e))?;

    hypr_claude::upsert_command_hook(&mut settings, "Stop", COMMAND)
        .map_err(|e| CliError::operation_failed("update claude settings hooks", e))?;

    hypr_claude::write_settings(&settings_path, &settings)
        .map_err(|e| CliError::operation_failed("write claude settings", e))?;

    eprintln!(
        "Installed char as Claude Code hook handler in {}",
        settings_path.display()
    );
    Ok(())
}

fn uninstall() -> CliResult<()> {
    let settings_path = hypr_claude::settings_path();

    let mut settings = hypr_claude::read_settings(&settings_path)
        .map_err(|e| CliError::operation_failed("read claude settings", e))?;

    hypr_claude::remove_command_hook(&mut settings, "Stop", COMMAND)
        .map_err(|e| CliError::operation_failed("update claude settings hooks", e))?;

    hypr_claude::write_settings(&settings_path, &settings)
        .map_err(|e| CliError::operation_failed("write claude settings", e))?;

    eprintln!(
        "Removed char from Claude Code hooks in {}",
        settings_path.display()
    );
    Ok(())
}
