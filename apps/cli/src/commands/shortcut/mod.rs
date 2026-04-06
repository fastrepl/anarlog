mod install;
mod plist;
mod screen;
mod service;
mod uninstall;

pub(crate) mod daemon;
pub(crate) mod hotkey;

use clap::Subcommand;

use crate::error::CliResult;

#[derive(Subcommand)]
pub enum Commands {
    /// Install the global shortcut daemon
    Install,
    /// Uninstall the shortcut daemon
    Uninstall,
    /// Check if the shortcut daemon is running
    Status,
}

pub async fn run(command: Option<Commands>) -> CliResult<()> {
    match command {
        Some(Commands::Install) => install::run(),
        Some(Commands::Uninstall) => uninstall::run(),
        Some(Commands::Status) => status(),
        None => screen::run(),
    }
}

fn status() -> CliResult<()> {
    let status = service::query();
    let local_blocker = hotkey::current_blocker();
    let ready = status.running && local_blocker.is_none();

    if status.installed || status.bootstrapped {
        if ready {
            eprintln!("Shortcut daemon is ready.");
        } else {
            eprintln!("Shortcut daemon is degraded.");
        }
    } else {
        eprintln!("Shortcut daemon is not installed.");
        eprintln!("Run `char shortcut install` to set it up.");
    }

    eprintln!("  Installed: {}", yes_no(status.installed));
    eprintln!("  Enabled: {}", yes_no(status.enabled));
    eprintln!("  Bootstrapped: {}", yes_no(status.bootstrapped));
    eprintln!("  Running: {}", yes_no(ready));
    if let Some(pid) = status.pid {
        eprintln!("  PID: {pid}");
    }
    if let Some(code) = status.last_exit_code {
        eprintln!("  Last exit code: {code}");
    }
    if let Some(reason) = status
        .reason()
        .or_else(|| local_blocker.map(|err| err.message().to_string()))
    {
        eprintln!("  Reason: {reason}");
    }
    eprintln!("  Logs: {}", plist::stderr_log_path().display());
    if let Some(blocker) = local_blocker {
        eprintln!("  Recovery: {}", blocker.recovery());
    } else if !ready && (status.bootstrapped || status.installed) {
        eprintln!(
            "  Recovery: Open System Settings → Privacy & Security → Accessibility and Input Monitoring. If macOS is stuck, run `tccutil reset ListenEvent` and then `char shortcut install`."
        );
    }

    Ok(())
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
