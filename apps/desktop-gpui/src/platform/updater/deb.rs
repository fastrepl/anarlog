//! Deb updates that never leave the package half-installed, as shipping's
//! `plugins/updater2/src/deb.rs` installs them.
//!
//! `dpkg -i` doesn't install missing dependencies: when the update depends on a package the
//! machine lacks, dpkg has already replaced the app's files by the time it stops, and it
//! leaves the package unconfigured. apt resolves dependencies before it changes anything, and
//! changes nothing when it can't, so the installed app keeps working.

use std::{
    ffi::OsStr,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

use desktop_runtime::{Result, ServiceError};

use super::failure;

// `--no-remove` aborts instead of removing another package. Recommended packages (GPUI's
// Vulkan drivers; it renders with GL without them) stay out, so the update fetches no more
// than it needs to run.
const APT_INSTALL: [&str; 4] = ["install", "--yes", "--no-remove", "--no-install-recommends"];

/// The arguments for `Programs::prompts`, in the same order.
const PROMPT_ARGS: [&[&str]; 2] = [
    &[
        "--password",
        "--title=Authentication Required",
        "--text=Enter your password to install the update:",
    ],
    &["--password", "Enter your password to install the update:"],
];

/// The programs an install runs, so tests can run it against fakes.
struct Programs<'a> {
    apt_get: &'a str,
    pkexec: &'a str,
    sudo: &'a str,
    /// zenity, then kdialog.
    prompts: [&'a str; 2],
}

const SYSTEM: Programs<'static> = Programs {
    apt_get: "/usr/bin/apt-get",
    pkexec: "pkexec",
    sudo: "sudo",
    prompts: ["zenity", "kdialog"],
};

pub(super) fn install(package: &Path) -> Result<()> {
    install_with(&SYSTEM, package)
}

fn install_with(programs: &Programs, package: &Path) -> Result<()> {
    if !Path::new(programs.apt_get).exists() {
        return run_as_root(
            programs,
            &[OsStr::new("dpkg"), OsStr::new("-i"), package.as_os_str()],
        );
    }

    // Unprivileged, so an update apt can't install is refused before any password prompt.
    let simulation = Command::new(programs.apt_get)
        .arg("--simulate")
        .args(APT_INSTALL)
        .arg(package)
        .output()
        .map_err(failure)?;
    if !simulation.status.success() {
        log_output("deb_update_dependencies_unavailable", &simulation);
        return Err(failure(format!(
            "The update needs system packages apt can't install, so Anarlog stays on its current version ({}). Run `sudo apt update`, then try the update again.",
            reason(&simulation)
        )));
    }

    let mut command: Vec<&OsStr> = vec![OsStr::new(programs.apt_get)];
    command.extend(APT_INSTALL.map(OsStr::new));
    command.extend(
        [
            // Fail at once while unattended-upgrades holds the dpkg lock, as `dpkg -i` did: the
            // caller checked for a recording just before, and a later pass retries.
            "-o",
            "DPkg::Lock::Timeout=0",
            // Nobody can answer a config file prompt for an upgraded dependency.
            "-o",
            "Dpkg::Options::=--force-confdef",
            "-o",
            "Dpkg::Options::=--force-confold",
        ]
        .map(OsStr::new),
    );
    command.push(package.as_os_str());
    run_as_root(programs, &command)
}

/// `tauri-plugin-updater`'s escalation order: pkexec, else sudo with a zenity or kdialog
/// password, else sudo on the terminal. A dismissed prompt or a failed command ends here
/// instead of running the install again the next way.
fn run_as_root(programs: &Programs, command: &[&OsStr]) -> Result<()> {
    match Command::new(programs.pkexec).args(command).output() {
        Ok(output) => match output.status.code() {
            Some(0) => return Ok(()),
            Some(126) => return Err(cancelled()),
            // Not authorized, for example because no polkit agent is running.
            Some(127) => {}
            _ => return Err(failed(&output)),
        },
        Err(error) => tracing::info!(%error, "pkexec_unavailable"),
    }

    let output = match ask_password(programs)? {
        Some(password) => {
            let mut child = Command::new(programs.sudo)
                .args(["-S", "-p", ""])
                .args(command)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(failure)?;
            if let Some(mut stdin) = child.stdin.take() {
                let _ = writeln!(stdin, "{password}");
            }
            child.wait_with_output().map_err(failure)?
        }
        None => Command::new(programs.sudo)
            .args(command)
            .stdin(Stdio::inherit())
            .output()
            .map_err(failure)?,
    };
    if output.status.success() {
        Ok(())
    } else {
        Err(failed(&output))
    }
}

/// `Ok(None)` when neither prompt is installed.
fn ask_password(programs: &Programs) -> Result<Option<String>> {
    for (program, args) in programs.prompts.into_iter().zip(PROMPT_ARGS) {
        let Ok(output) = Command::new(program).args(args).output() else {
            continue;
        };
        if !output.status.success() {
            return Err(cancelled());
        }
        let password = String::from_utf8_lossy(&output.stdout);
        return Ok(Some(password.trim_end_matches(['\r', '\n']).to_owned()));
    }
    Ok(None)
}

fn cancelled() -> ServiceError {
    install_failed("authentication was cancelled")
}

fn failed(output: &Output) -> ServiceError {
    log_output("deb_update_install_failed", output);
    install_failed(&reason(output))
}

/// Unlike the refusal, this can't promise the current version is kept: apt may have unpacked
/// the package by the time it fails.
fn install_failed(reason: &str) -> ServiceError {
    failure(format!("The update failed to install: {reason}"))
}

/// apt's own explanation, for the message the update UI shows: the unmet dependencies,
/// else its last error (which sums up the ones before it), else the last line.
fn reason(output: &Output) -> String {
    let text = [&output.stdout, &output.stderr]
        .map(|stream| String::from_utf8_lossy(stream).into_owned())
        .join("\n");
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let unmet: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| line.contains("Depends:"))
        .collect();
    if !unmet.is_empty() {
        return unmet.join("; ");
    }
    lines
        .iter()
        .rfind(|line| line.starts_with("E:"))
        .or(lines.last())
        .map_or_else(|| output.status.to_string(), |line| (*line).to_owned())
}

fn log_output(event: &str, output: &Output) {
    tracing::error!(
        status = %output.status,
        stdout = %String::from_utf8_lossy(&output.stdout),
        stderr = %String::from_utf8_lossy(&output.stderr),
        "{event}"
    );
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    // A deb whose dependency apt can't install must be refused before anything asks for a
    // password: `pkexec dpkg -i` would unpack it over the installed app and leave it
    // unconfigured.
    #[test]
    fn deb_update_apt_cannot_satisfy_is_refused_before_any_password_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let prompted = dir.path().join("prompted");
        let fake = |name: &str, body: &str| {
            let path = dir.path().join(name);
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            path.into_os_string().into_string().unwrap()
        };
        let apt_get = fake(
            "apt-get",
            "echo 'The following packages have unmet dependencies:'\n\
             echo ' anarlog : Depends: libanarlog-missing0 but it is not installable'\n\
             echo 'E: Unable to correct problems, you have held broken packages.' >&2\n\
             exit 100",
        );
        let escalation =
            |name: &str| fake(name, &format!("echo {name} >> '{}'", prompted.display()));
        let (pkexec, sudo, zenity, kdialog) = (
            escalation("pkexec"),
            escalation("sudo"),
            escalation("zenity"),
            escalation("kdialog"),
        );
        let programs = Programs {
            apt_get: &apt_get,
            pkexec: &pkexec,
            sudo: &sudo,
            prompts: [&zenity, &kdialog],
        };

        let error = install_with(&programs, &dir.path().join("update.deb")).unwrap_err();

        let message = error.to_string();
        assert!(
            message.contains("Depends: libanarlog-missing0 but it is not installable")
                && message.contains("Run `sudo apt update`"),
            "{message}"
        );
        assert!(
            !prompted.exists(),
            "{}",
            std::fs::read_to_string(&prompted).unwrap_or_default()
        );
    }
}
