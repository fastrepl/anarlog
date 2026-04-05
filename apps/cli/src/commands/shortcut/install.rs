use std::fs;
use std::process::Command;
use std::time::{Duration, Instant};

use crate::error::{CliError, CliResult};

use super::{hotkey, plist, service};

pub(crate) fn run() -> CliResult<()> {
    if let Some(blocker) = hotkey::current_blocker() {
        eprintln!("Shortcut daemon cannot start yet.");
        eprintln!("  Reason: {}", blocker.message());
        if blocker.kind() == hotkey::HotkeyErrorKind::InputMonitoringDenied {
            eprintln!("Opening System Settings → Privacy & Security → Input Monitoring…");
            let _ = Command::new("open")
                .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent")
                .status();
        } else if blocker.kind() == hotkey::HotkeyErrorKind::AccessibilityDenied {
            eprintln!("Opening System Settings → Privacy & Security → Accessibility…");
            let _ = Command::new("open")
                .arg(
                    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
                )
                .status();
        }
        eprintln!("  Recovery: {}", blocker.recovery());
        return Ok(());
    }

    let binary_path = std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .map_err(|e| CliError::operation_failed("resolve binary path", e.to_string()))?;

    let plist_content = plist::service_config(&binary_path).plist_contents();
    let plist_path = plist::plist_path();

    if let Some(parent) = plist_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| CliError::operation_failed("create LaunchAgents dir", e.to_string()))?;
    }

    let log_dir = dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Logs/char");
    fs::create_dir_all(&log_dir)
        .map_err(|e| CliError::operation_failed("create log dir", e.to_string()))?;

    // Unload existing agent if present
    if plist_path.exists() {
        let _ = service::stop(&plist_path);
    }

    fs::write(&plist_path, plist_content)
        .map_err(|e| CliError::operation_failed("write plist", e.to_string()))?;

    service::start(&plist_path)?;
    let status = wait_for_service();

    eprintln!("Shortcut daemon installed.");
    eprintln!("  Plist: {}", plist_path.display());
    eprintln!("  Binary: {}", binary_path.display());
    eprintln!("  Logs: {}", plist::stderr_log_path().display());
    eprintln!();

    if status.running {
        eprintln!("Shortcut daemon is ready.");
        eprintln!("Double-tap Right Option to start recording.");
    } else if let Some(code) = status.last_exit_code {
        eprintln!("Shortcut daemon failed to start.");
        eprintln!("  Last exit code: {code}");
        print_recovery_guidance();
    } else if status.bootstrapped {
        eprintln!("Shortcut daemon is bootstrapped, but startup is ambiguous.");
        print_recovery_guidance();
    } else {
        eprintln!("Shortcut daemon did not bootstrap.");
        print_recovery_guidance();
    }

    eprintln!("Run `char shortcut uninstall` to remove.");

    Ok(())
}

fn wait_for_service() -> service::ServiceStatus {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let status = service::query();
        if status.running {
            // launchd can briefly report a PID before the daemon crashes, so only
            // accept "ready" after a second settled query.
            std::thread::sleep(Duration::from_millis(300));
            let settled = service::query();
            if settled.running {
                return settled;
            }
            return settled;
        }
        if status.last_exit_code.is_some() || !status.bootstrapped {
            return status;
        }
        if Instant::now() >= deadline {
            return status;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn print_recovery_guidance() {
    if let Some(blocker) = hotkey::current_blocker() {
        eprintln!("  Reason: {}", blocker.message());
        eprintln!("  Recovery: {}", blocker.recovery());
        return;
    }
    eprintln!(
        "  Recovery: Open System Settings → Privacy & Security → Accessibility and Input Monitoring. If Secure Keyboard Entry is enabled, disable it and retry. If macOS is stuck, run `tccutil reset ListenEvent` and then `char shortcut install`."
    );
}
