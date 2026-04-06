use clap::Subcommand;

use crate::error::{CliError, CliResult};

#[derive(Subcommand)]
pub enum Commands {
    /// Receive a hook event from OpenCode (reads JSON from stdin)
    Notify,
    /// Install char as an OpenCode plugin
    Install,
    /// Remove char from OpenCode plugins
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
    let plugin_path = hypr_opencode::plugin_path();

    hypr_opencode::write_plugin(&plugin_path)
        .map_err(|e| CliError::operation_failed("write opencode plugin", e))?;

    eprintln!(
        "Installed char as OpenCode plugin at {}",
        plugin_path.display()
    );
    Ok(())
}

fn uninstall() -> CliResult<()> {
    let plugin_path = hypr_opencode::plugin_path();

    hypr_opencode::remove_plugin(&plugin_path)
        .map_err(|e| CliError::operation_failed("remove opencode plugin", e))?;

    eprintln!(
        "Removed char OpenCode plugin from {}",
        plugin_path.display()
    );
    Ok(())
}
