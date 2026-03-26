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
    let input = std::io::read_to_string(std::io::stdin())
        .map_err(|e| CliError::operation_failed("read stdin", e.to_string()))?;

    let event: serde_json::Value = serde_json::from_str(&input)
        .map_err(|e| CliError::invalid_argument("stdin", input, e.to_string()))?;

    // TODO: write to app DB
    println!(
        "{}",
        serde_json::to_string_pretty(&event)
            .map_err(|e| CliError::operation_failed("serialize", e.to_string()))?
    );
    Ok(())
}

fn has_char_hook(entry: &serde_json::Value) -> bool {
    entry
        .get("hooks")
        .and_then(|h| h.as_array())
        .is_some_and(|hooks| {
            hooks
                .iter()
                .any(|h| h.get("command").and_then(|c| c.as_str()) == Some(COMMAND))
        })
}

fn install() -> CliResult<()> {
    let settings_path = hypr_claude::settings_path();

    let mut settings = hypr_claude::read_settings(&settings_path)
        .map_err(|e| CliError::operation_failed("read claude settings", e))?;

    let obj = settings.as_object_mut().ok_or_else(|| {
        CliError::operation_failed("read claude settings", "expected object".to_string())
    })?;

    let hooks = obj
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .ok_or_else(|| {
            CliError::operation_failed(
                "read claude settings",
                "hooks must be an object".to_string(),
            )
        })?;

    let stop_hooks = hooks.entry("Stop").or_insert_with(|| serde_json::json!([]));

    if let Some(arr) = stop_hooks.as_array_mut() {
        if !arr.iter().any(has_char_hook) {
            arr.push(serde_json::json!({
                "hooks": [{ "type": "command", "command": COMMAND }]
            }));
        }
    }

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

    if let Some(hooks) = settings
        .as_object_mut()
        .and_then(|obj| obj.get_mut("hooks"))
        .and_then(|h| h.as_object_mut())
    {
        if let Some(stop_hooks) = hooks.get_mut("Stop").and_then(|s| s.as_array_mut()) {
            stop_hooks.retain(|entry| !has_char_hook(entry));
        }
    }

    hypr_claude::write_settings(&settings_path, &settings)
        .map_err(|e| CliError::operation_failed("write claude settings", e))?;

    eprintln!(
        "Removed char from Claude Code hooks in {}",
        settings_path.display()
    );
    Ok(())
}
