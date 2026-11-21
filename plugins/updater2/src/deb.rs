//! Deb updates that never leave the package half-installed.
//!
//! tauri-plugin-updater installs a deb with `dpkg -i`, which doesn't install missing
//! dependencies: when the update depends on a package the machine lacks, dpkg has already
//! replaced the app's files by the time it stops, and it leaves the package unconfigured.
//! apt resolves dependencies before it changes anything, and changes nothing when it can't,
//! so the installed app keeps working.

use std::{
    ffi::OsStr,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

const APT_GET: &str = "/usr/bin/apt-get";

// `--no-remove` aborts instead of removing another package. Recommended packages (GPUI's
// Vulkan drivers; it renders with GL without them) stay out, so the update fetches no more
// than it needs to run.
const APT_INSTALL: [&str; 4] = ["install", "--yes", "--no-remove", "--no-install-recommends"];

const PASSWORD_PROMPTS: [&[&str]; 2] = [
    &[
        "zenity",
        "--password",
        "--title=Authentication Required",
        "--text=Enter your password to install the update:",
    ],
    &[
        "kdialog",
        "--password",
        "Enter your password to install the update:",
    ],
];

pub(crate) fn install(bytes: &[u8]) -> Result<(), crate::Error> {
    if !bytes.starts_with(b"!<arch>\ndebian-binary") {
        return Err(tauri_plugin_updater::Error::InvalidUpdaterFormat.into());
    }
    let dir = tempfile::Builder::new()
        .prefix("anarlog-update")
        .tempdir()?;
    let package = dir.path().join("update.deb");
    std::fs::write(&package, bytes)?;

    if !Path::new(APT_GET).exists() {
        return run_as_root(&[OsStr::new("dpkg"), OsStr::new("-i"), package.as_os_str()]);
    }

    // Unprivileged, so an update apt can't install is refused before any password prompt.
    let simulation = Command::new(APT_GET)
        .arg("--simulate")
        .args(APT_INSTALL)
        .arg(&package)
        .output()?;
    if !simulation.status.success() {
        log_output("deb_update_dependencies_unavailable", &simulation);
        return Err(crate::Error::DebDependencies(reason(&simulation)));
    }

    let mut command: Vec<&OsStr> = vec![OsStr::new(APT_GET)];
    command.extend(APT_INSTALL.map(OsStr::new));
    command.extend(
        [
            // Fail at once while unattended-upgrades holds the dpkg lock, as `dpkg -i` did: the
            // caller checked for a recording just before, and a later tick retries.
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
    run_as_root(&command)
}

/// tauri-plugin-updater's escalation order: pkexec, else sudo with a zenity or kdialog
/// password, else sudo on the terminal. Unlike the plugin, a dismissed prompt or a failed
/// command ends here instead of running the install again the next way.
fn run_as_root(command: &[&OsStr]) -> Result<(), crate::Error> {
    match Command::new("pkexec").args(command).output() {
        Ok(output) => match output.status.code() {
            Some(0) => return Ok(()),
            Some(126) => return Err(cancelled()),
            // Not authorized, for example because no polkit agent is running.
            Some(127) => {}
            _ => return Err(failed(&output)),
        },
        Err(error) => tracing::info!("pkexec_unavailable: {}", error),
    }

    let output = match ask_password()? {
        Some(password) => {
            let mut child = Command::new("sudo")
                .args(["-S", "-p", ""])
                .args(command)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;
            if let Some(mut stdin) = child.stdin.take() {
                let _ = writeln!(stdin, "{password}");
            }
            child.wait_with_output()?
        }
        None => Command::new("sudo")
            .args(command)
            .stdin(Stdio::inherit())
            .output()?,
    };
    if output.status.success() {
        Ok(())
    } else {
        Err(failed(&output))
    }
}

/// `Ok(None)` when neither prompt is installed.
fn ask_password() -> Result<Option<String>, crate::Error> {
    for prompt in PASSWORD_PROMPTS {
        let Ok(output) = Command::new(prompt[0]).args(&prompt[1..]).output() else {
            continue;
        };
        if !output.status.success() {
            return Err(cancelled());
        }
        let password = String::from_utf8_lossy(&output.stdout);
        return Ok(Some(password.trim_end_matches(['\r', '\n']).to_string()));
    }
    Ok(None)
}

fn cancelled() -> crate::Error {
    crate::Error::DebInstall("authentication was cancelled".to_string())
}

fn failed(output: &Output) -> crate::Error {
    log_output("deb_update_install_failed", output);
    crate::Error::DebInstall(reason(output))
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
        .map_or_else(|| output.status.to_string(), |line| line.to_string())
}

fn log_output(event: &str, output: &Output) {
    tracing::error!(
        "{}: {}\n{}{}",
        event,
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    // A deb whose dependency no repository has must be refused before the privileged
    // install: `dpkg -i` would unpack it over the installed app and leave it unconfigured.
    #[test]
    fn deb_update_with_uninstallable_dependency_is_refused_before_install() {
        if !std::path::Path::new(super::APT_GET).exists() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("pkg");
        std::fs::create_dir_all(root.join("DEBIAN")).unwrap();
        std::fs::write(
            root.join("DEBIAN/control"),
            "Package: anarlog-updater2-test\nVersion: 1.0\nArchitecture: all\n\
             Maintainer: test\nDepends: anarlog-updater2-test-missing\nDescription: test\n",
        )
        .unwrap();
        let deb = dir.path().join("test.deb");
        let built = Command::new("dpkg-deb")
            .arg("--build")
            .args([&root, &deb])
            .output()
            .unwrap();
        assert!(built.status.success(), "{built:?}");

        let error = super::install(&std::fs::read(&deb).unwrap()).unwrap_err();

        let crate::Error::DebDependencies(reason) = error else {
            panic!("expected a dependency error, got {error:?}");
        };
        assert!(reason.contains("anarlog-updater2-test-missing"), "{reason}");
    }
}
