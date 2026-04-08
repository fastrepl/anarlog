use clap::Subcommand;

use crate::error::{CliError, CliResult};

#[derive(Subcommand)]
pub enum Commands {
    /// Receive a hook event from OpenCode
    Notify {
        /// JSON payload from OpenCode
        payload: String,
    },
    /// Install char as an OpenCode plugin
    Install,
    /// Remove char from OpenCode plugins
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
    let event: serde_json::Value = serde_json::from_str(payload)
        .map_err(|e| CliError::invalid_argument("payload", payload.to_string(), e.to_string()))?;

    // TODO: write to app DB
    super::print_pretty_json(&event)
}

fn install() -> CliResult<()> {
    let plugin_path = hypr_opencode::plugin_path();

    if plugin_path.exists()
        && !hypr_opencode::has_char_plugin(&plugin_path)
            .map_err(|e| CliError::operation_failed("read opencode plugin", e))?
    {
        return Err(CliError::operation_failed(
            "install opencode plugin",
            format!(
                "refusing to replace existing plugin at {}",
                plugin_path.display()
            ),
        ));
    }

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

    if hypr_opencode::has_char_plugin(&plugin_path)
        .map_err(|e| CliError::operation_failed("read opencode plugin", e))?
    {
        hypr_opencode::remove_plugin(&plugin_path)
            .map_err(|e| CliError::operation_failed("remove opencode plugin", e))?;
    }

    eprintln!(
        "Removed char OpenCode plugin from {}",
        plugin_path.display()
    );
    Ok(())
}
