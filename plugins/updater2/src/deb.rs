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
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

const APT_GET: &str = "/usr/bin/apt-get";
const ENV: &str = "/usr/bin/env";

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
    let (_dir, package) = stage_package(bytes)?;

    if !Path::new(APT_GET).exists() {
        return run_as_root(
            &[OsStr::new("dpkg"), OsStr::new("-i"), package.as_os_str()],
            None,
        );
    }

    // Unprivileged, so an update apt can't install is refused before any password prompt.
    let simulation = simulate_apt_install_with(OsStr::new(APT_GET), &package)?;
    if !simulation.status.success() {
        log_output("deb_update_dependencies_unavailable", &simulation);
        return Err(crate::Error::DebDependencies(reason(&simulation)));
    }

    let direct_command = apt_direct_command(OsStr::new(APT_GET), &package);
    let env_command = apt_env_command(&direct_command);
    run_as_root(&env_command, Some(&direct_command))
}

fn stage_package(bytes: &[u8]) -> std::io::Result<(tempfile::TempDir, PathBuf)> {
    let dir = tempfile::Builder::new()
        .prefix("anarlog-update")
        .tempdir()?;
    let package = dir.path().join("update.deb");
    std::fs::write(&package, bytes)?;
    // Allow `_apt` to traverse the staging directory and read `update.deb` without
    // falling back to an unsandboxed root download.
    std::fs::set_permissions(&package, std::fs::Permissions::from_mode(0o644))?;
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755))?;
    Ok((dir, package))
}

fn simulate_apt_install_with(apt_bin: &OsStr, package: &Path) -> std::io::Result<Output> {
    Command::new(apt_bin)
        .env("LC_ALL", "C")
        .arg("--simulate")
        .args(APT_INSTALL)
        .arg(package)
        .output()
}

fn apt_direct_command<'a>(apt_bin: &'a OsStr, package: &'a Path) -> Vec<&'a OsStr> {
    let mut command: Vec<&OsStr> = vec![apt_bin];
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
    command
}

fn apt_env_command<'a>(direct: &[&'a OsStr]) -> Vec<&'a OsStr> {
    let mut command: Vec<&OsStr> = Vec::with_capacity(3 + direct.len());
    command.extend([
        OsStr::new(ENV),
        OsStr::new("DEBIAN_FRONTEND=noninteractive"),
        OsStr::new("LC_ALL=C"),
    ]);
    command.extend_from_slice(direct);
    command
}

fn run_pkexec(
    pkexec_bin: &OsStr,
    command: &[&OsStr],
    direct_fallback: Option<&[&OsStr]>,
) -> Option<Result<(), crate::Error>> {
    let run = |args: &[&OsStr]| {
        Command::new(pkexec_bin)
            .env("DEBIAN_FRONTEND", "noninteractive")
            .env("LC_ALL", "C")
            .args(args)
            .stdin(Stdio::null())
            .output()
    };
    let output = match run(command) {
        Ok(output) => output,
        Err(error) => {
            tracing::info!("pkexec_unavailable: {}", error);
            return None;
        }
    };
    match output.status.code() {
        Some(0) => Some(Ok(())),
        Some(126) => Some(Err(cancelled())),
        // 127 signals polkit denial or missing agent. Retry the direct apt-get command once
        // in case policy permits apt-get without `/usr/bin/env`.
        Some(127) => {
            let direct = direct_fallback?;
            let output = match run(direct) {
                Ok(output) => output,
                Err(error) => {
                    tracing::info!("pkexec_unavailable: {}", error);
                    return None;
                }
            };
            match output.status.code() {
                Some(0) => Some(Ok(())),
                Some(126) => Some(Err(cancelled())),
                Some(127) => None,
                _ => Some(Err(failed(&output))),
            }
        }
        _ => Some(Err(failed(&output))),
    }
}

fn run_sudo_with_password(
    sudo_bin: &OsStr,
    password: &str,
    command: &[&OsStr],
) -> std::io::Result<Output> {
    let mut child = Command::new(sudo_bin)
        .env("DEBIAN_FRONTEND", "noninteractive")
        .env("LC_ALL", "C")
        .args(["-S", "-p", ""])
        .args(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = writeln!(stdin, "{password}");
    }
    child.wait_with_output()
}

/// tauri-plugin-updater's escalation order: pkexec, else sudo with a zenity or kdialog
/// password, else non-interactive sudo. Unlike the plugin, a dismissed prompt or a failed
/// command ends here instead of running the install again the next way.
fn run_as_root(command: &[&OsStr], direct_fallback: Option<&[&OsStr]>) -> Result<(), crate::Error> {
    if let Some(result) = run_pkexec(OsStr::new("pkexec"), command, direct_fallback) {
        return result;
    }

    match ask_password()? {
        Some(password) => {
            let mut output = run_sudo_with_password(OsStr::new("sudo"), &password, command)?;
            if !output.status.success()
                && let Some(direct) = direct_fallback
                && is_sudo_command_denied(&output)
            {
                output = run_sudo_with_password(OsStr::new("sudo"), &password, direct)?;
            }
            if output.status.success() {
                Ok(())
            } else {
                Err(failed(&output))
            }
        }
        None => run_sudo_non_interactive(OsStr::new("sudo"), command, direct_fallback),
    }
}

fn run_sudo_non_interactive(
    sudo_bin: &OsStr,
    command: &[&OsStr],
    direct_fallback: Option<&[&OsStr]>,
) -> Result<(), crate::Error> {
    let run = |args: &[&OsStr]| {
        Command::new(sudo_bin)
            .env("DEBIAN_FRONTEND", "noninteractive")
            .env("LC_ALL", "C")
            .arg("-n")
            .args(args)
            .stdin(Stdio::null())
            .output()
    };
    let mut output = match run(command) {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            tracing::info!("sudo_unavailable: {}", error);
            return Err(non_interactive_auth_unavailable());
        }
        Err(error) => return Err(error.into()),
    };

    if !output.status.success()
        && let Some(direct) = direct_fallback
        && (is_non_interactive_sudo_auth_failure(&output) || is_sudo_command_denied(&output))
    {
        output = run(direct)?;
    }

    if output.status.success() {
        return Ok(());
    }

    if is_non_interactive_sudo_auth_failure(&output) {
        log_output("deb_update_auth_unavailable", &output);
        return Err(non_interactive_auth_unavailable());
    }

    Err(failed(&output))
}

fn is_sudo_command_denied(output: &Output) -> bool {
    let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
    stderr.contains("not allowed to execute")
}

fn is_non_interactive_sudo_auth_failure(output: &Output) -> bool {
    let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
    [
        "password is required",
        "terminal is required",
        "no tty present",
        "authentication is required",
        "askpass",
    ]
    .iter()
    .any(|needle| stderr.contains(needle))
}

fn non_interactive_auth_unavailable() -> crate::Error {
    crate::Error::DebInstall(
        "no graphical authentication agent (polkit, zenity, or kdialog) or passwordless sudo is available; install a graphical authentication agent or update Anarlog using `apt`"
            .to_string(),
    )
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
    crate::Error::DebInstall(install_failure_message(output))
}

fn install_failure_message(output: &Output) -> String {
    let combined = combined_output(output);
    let message = reason_from_text(&combined, &output.status);
    if looks_like_stale_apt_index(&combined) {
        let separator = if message.ends_with(['.', '?', '!']) {
            " "
        } else {
            ". "
        };
        format!("{message}{separator}Run `sudo apt update`, then try the update again.")
    } else {
        message
    }
}

fn looks_like_stale_apt_index(text: &str) -> bool {
    text.contains("Unable to fetch")
        || text.contains("Failed to fetch")
        || text
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|token| token == "404")
}

/// apt's own explanation, for the message the update UI shows: the unmet dependencies,
/// else its last error (which sums up the ones before it), else the last line.
fn reason(output: &Output) -> String {
    reason_from_text(&combined_output(output), &output.status)
}

fn combined_output(output: &Output) -> String {
    [&output.stdout, &output.stderr]
        .map(|stream| String::from_utf8_lossy(stream).into_owned())
        .join("\n")
}

fn reason_from_text(text: &str, status: &std::process::ExitStatus) -> String {
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
        .map_or_else(|| status.to_string(), |line| line.to_string())
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
    use std::{
        ffi::OsStr,
        os::unix::{fs::PermissionsExt, process::ExitStatusExt},
        process::{Command, ExitStatus, Output},
    };

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

    #[test]
    fn deb_staging_and_install_preserve_permissions_and_env_across_pkexec() {
        let (dir, package) = super::stage_package(b"!<arch>\ndebian-binary").unwrap();
        assert_eq!(
            std::fs::metadata(dir.path()).unwrap().permissions().mode() & 0o777,
            0o755
        );
        assert_eq!(
            std::fs::metadata(&package).unwrap().permissions().mode() & 0o777,
            0o644
        );

        let sim_log = dir.path().join("sim.log");
        let fake_apt = dir.path().join("fake-apt");
        std::fs::write(
            &fake_apt,
            format!(
                "#!/bin/sh\nprintf '%s\\n%s\\n%s\\n' \"$LC_ALL\" \"$DEBIAN_FRONTEND\" \"$*\" > \"{}\"\n",
                sim_log.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&fake_apt, std::fs::Permissions::from_mode(0o755)).unwrap();

        let sim_output = super::simulate_apt_install_with(fake_apt.as_os_str(), &package).unwrap();
        assert!(sim_output.status.success());
        let sim_recorded = std::fs::read_to_string(&sim_log).unwrap();
        assert!(sim_recorded.starts_with("C\n"), "{sim_recorded}");
        assert!(sim_recorded.contains("--simulate"), "{sim_recorded}");
        assert!(
            sim_recorded.contains(&package.display().to_string()),
            "{sim_recorded}"
        );

        let fake_pkexec = dir.path().join("fake-pkexec");
        std::fs::write(&fake_pkexec, "#!/bin/sh\nexec env -i \"$@\"\n").unwrap();
        std::fs::set_permissions(&fake_pkexec, std::fs::Permissions::from_mode(0o755)).unwrap();

        let direct = super::apt_direct_command(fake_apt.as_os_str(), &package);
        let wrapped = super::apt_env_command(&direct);
        let installed =
            super::run_pkexec(fake_pkexec.as_os_str(), &wrapped, Some(&direct)).unwrap();
        assert!(installed.is_ok(), "{installed:?}");

        let install_recorded = std::fs::read_to_string(&sim_log).unwrap();
        assert!(
            install_recorded.contains("C\nnoninteractive\n"),
            "{install_recorded}"
        );
        assert!(
            install_recorded.contains(&package.display().to_string()),
            "{install_recorded}"
        );
    }

    #[test]
    fn apt_only_policies_fall_back_from_env_wrapper_to_direct_apt() {
        let dir = tempfile::tempdir().unwrap();
        let ran = dir.path().join("ran.log");
        let fake_apt = dir.path().join("apt-get");
        std::fs::write(
            &fake_apt,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$0 $*\" >> \"{}\"\n",
                ran.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&fake_apt, std::fs::Permissions::from_mode(0o755)).unwrap();

        let package = dir.path().join("update.deb");
        let direct = super::apt_direct_command(fake_apt.as_os_str(), &package);
        let wrapped = super::apt_env_command(&direct);

        let policy_pkexec = dir.path().join("pkexec");
        std::fs::write(
            &policy_pkexec,
            format!(
                "#!/bin/sh\nif [ \"$1\" != \"{}\" ]; then exit 127; fi\nexec \"$@\"\n",
                fake_apt.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&policy_pkexec, std::fs::Permissions::from_mode(0o755)).unwrap();

        let pkexec_result =
            super::run_pkexec(policy_pkexec.as_os_str(), &wrapped, Some(&direct)).unwrap();
        assert!(pkexec_result.is_ok(), "{pkexec_result:?}");

        let policy_sudo = dir.path().join("sudo");
        std::fs::write(
            &policy_sudo,
            format!(
                "#!/bin/sh\nif [ \"$1\" != \"-n\" ]; then exit 99; fi\nshift\n\
                 if read -r _unused; then exit 98; fi\n\
                 if [ \"$1\" != \"{}\" ]; then\n  \
                   echo \"Sorry, user is not allowed to execute '$1' as root.\" >&2\n  \
                   exit 1\n\
                 fi\nexec \"$@\"\n",
                fake_apt.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&policy_sudo, std::fs::Permissions::from_mode(0o755)).unwrap();

        super::run_sudo_non_interactive(policy_sudo.as_os_str(), &wrapped, Some(&direct)).unwrap();

        let recorded = std::fs::read_to_string(&ran).unwrap();
        assert_eq!(recorded.lines().count(), 2, "{recorded}");
        assert!(
            recorded
                .lines()
                .all(|line| line.starts_with(&fake_apt.display().to_string()))
        );
    }

    #[test]
    fn non_interactive_sudo_fallback_and_stale_index_error_hint() {
        let dir = tempfile::tempdir().unwrap();
        let fake_sudo = dir.path().join("fake-sudo");
        std::fs::write(
            &fake_sudo,
            "#!/bin/sh\nif [ \"$1\" != \"-n\" ]; then exit 99; fi\n\
             if read -r _unused; then exit 98; fi\n\
             echo 'sudo: a password is required' >&2\nexit 1\n",
        )
        .unwrap();
        std::fs::set_permissions(&fake_sudo, std::fs::Permissions::from_mode(0o755)).unwrap();

        let auth_err = super::run_sudo_non_interactive(
            fake_sudo.as_os_str(),
            &[OsStr::new("/bin/true")],
            None,
        )
        .unwrap_err();
        let crate::Error::DebInstall(message) = auth_err else {
            panic!("expected DebInstall error, got {auth_err:?}");
        };
        assert!(message.contains("polkit"), "{message}");
        assert!(message.contains("passwordless sudo"), "{message}");

        let missing_err = super::run_sudo_non_interactive(
            dir.path().join("missing-sudo").as_os_str(),
            &[OsStr::new("/bin/true")],
            None,
        )
        .unwrap_err();
        assert!(matches!(missing_err, crate::Error::DebInstall(_)));

        let failure_output = |stderr: &[u8]| Output {
            status: ExitStatus::from_raw(100 << 8),
            stdout: Vec::new(),
            stderr: stderr.to_vec(),
        };
        for stderr in [
            b"Err:1 http://deb.debian.org bookworm InRelease\nE: Unable to fetch some archives, maybe run apt-get update?".as_slice(),
            b"E: Failed to fetch http://example.com/pkg.deb  404  Not Found".as_slice(),
        ] {
            let msg = super::install_failure_message(&failure_output(stderr));
            assert!(msg.contains("sudo apt update"), "{msg}");
        }

        let lock_msg = super::install_failure_message(&failure_output(
            b"E: Could not get lock /var/lib/dpkg/lock-frontend. It is held by process 1404 (unattended-upgr)",
        ));
        assert!(!lock_msg.contains("sudo apt update"), "{lock_msg}");
    }
}
