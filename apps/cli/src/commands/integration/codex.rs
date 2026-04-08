use clap::Subcommand;

use crate::error::{CliError, CliResult};

#[derive(Subcommand)]
pub enum Commands {
    /// Receive a notification from Codex
    Notify {
        /// JSON payload from Codex
        payload: String,
    },
    /// Install char as the Codex notify handler
    Install,
    /// Remove char from the Codex notify handler
    Uninstall,
}

pub async fn run(command: Commands) -> CliResult<()> {
    match command {
        Commands::Notify { payload } => notify(&payload),
        Commands::Install => install(),
        Commands::Uninstall => uninstall(),
    }
}

fn notify(payload: &str) -> CliResult<()> {
    let event: hypr_codex::NotifyEvent = serde_json::from_str(payload)
        .map_err(|e| CliError::invalid_argument("payload", payload.to_string(), e.to_string()))?;

    // TODO: write to app DB
    super::print_pretty_json(&event)
}

fn install() -> CliResult<()> {
    let config_path = hypr_codex::config_path();
    let command = hypr_codex::notify_command();

    let mut table = hypr_codex::read_config(&config_path)
        .map_err(|e| CliError::operation_failed("read codex config", e))?;

    if table.contains_key("notify") && !hypr_codex::has_notify(&table, &command) {
        return Err(CliError::operation_failed(
            "install codex integration",
            format!(
                "refusing to replace existing notify handler in {}",
                config_path.display()
            ),
        ));
    }

    hypr_codex::set_notify(&mut table, command);

    hypr_codex::write_config(&config_path, &table)
        .map_err(|e| CliError::operation_failed("write codex config", e))?;

    eprintln!(
        "Installed char as Codex notify handler in {}",
        config_path.display()
    );
    Ok(())
}

fn uninstall() -> CliResult<()> {
    let config_path = hypr_codex::config_path();
    let command = hypr_codex::notify_command();

    let mut table = hypr_codex::read_config(&config_path)
        .map_err(|e| CliError::operation_failed("read codex config", e))?;

    if hypr_codex::has_notify(&table, &command) {
        hypr_codex::remove_notify(&mut table);
    }

    hypr_codex::write_config(&config_path, &table)
        .map_err(|e| CliError::operation_failed("write codex config", e))?;

    eprintln!(
        "Removed char from Codex notify handler in {}",
        config_path.display()
    );
    Ok(())
}
