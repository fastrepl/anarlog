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
    env: &'a str,
    pkexec: &'a str,
    sudo: &'a str,
    /// zenity, then kdialog.
    prompts: [&'a str; 2],
}

const SYSTEM: Programs<'static> = Programs {
    apt_get: "/usr/bin/apt-get",
    env: "/usr/bin/env",
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
            None,
        );
    }

    // Unprivileged, so an update apt can't install is refused before any password prompt.
    let simulation =
        simulate_apt_install_with(OsStr::new(programs.apt_get), package).map_err(failure)?;
    if !simulation.status.success() {
        log_output("deb_update_dependencies_unavailable", &simulation);
        return Err(failure(format!(
            "The update needs system packages apt can't install, so Anarlog stays on its current version ({}). Run `sudo apt update`, then try the update again.",
            reason(&simulation)
        )));
    }

    let direct_command = apt_direct_command(OsStr::new(programs.apt_get), package);
    let env_command = apt_env_command(OsStr::new(programs.env), &direct_command);
    run_as_root(programs, &env_command, Some(&direct_command))
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
    command
}

fn apt_env_command<'a>(env_bin: &'a OsStr, direct: &[&'a OsStr]) -> Vec<&'a OsStr> {
    let mut command: Vec<&OsStr> = Vec::with_capacity(3 + direct.len());
    command.extend([
        env_bin,
        OsStr::new("DEBIAN_FRONTEND=noninteractive"),
        OsStr::new("LC_ALL=C"),
    ]);
    command.extend_from_slice(direct);
    command
}

fn run_pkexec(pkexec_bin: &OsStr, command: &[&OsStr]) -> Option<Result<()>> {
    let output = match Command::new(pkexec_bin)
        .env("DEBIAN_FRONTEND", "noninteractive")
        .env("LC_ALL", "C")
        .args(command)
        .stdin(Stdio::null())
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            tracing::info!(%error, "pkexec_unavailable");
            return None;
        }
    };
    match output.status.code() {
        Some(0) => Some(Ok(())),
        Some(126) => Some(Err(cancelled())),
        // 127 signals polkit denial (e.g. policy rejects `/usr/bin/env`) or a missing agent. Do
        // not invoke `apt-get` directly via `pkexec` because `pkexec` strips `DEBIAN_FRONTEND`;
        // fall through to sudo instead.
        Some(127) => None,
        _ => Some(Err(failed(&output))),
    }
}

fn run_sudo_with_password(
    sudo_bin: &OsStr,
    password: &str,
    preserve_env: bool,
    command: &[&OsStr],
) -> std::io::Result<Output> {
    let mut cmd = Command::new(sudo_bin);
    cmd.env("DEBIAN_FRONTEND", "noninteractive")
        .env("LC_ALL", "C")
        .args(["-S", "-p", ""]);
    if preserve_env {
        cmd.arg("--preserve-env=DEBIAN_FRONTEND,LC_ALL");
    }
    let mut child = cmd
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

/// `tauri-plugin-updater`'s escalation order: pkexec, else sudo with a zenity or kdialog
/// password, else non-interactive sudo. A dismissed prompt or a failed command ends here
/// instead of running the install again the next way.
fn run_as_root(
    programs: &Programs,
    command: &[&OsStr],
    direct_fallback: Option<&[&OsStr]>,
) -> Result<()> {
    if let Some(result) = run_pkexec(OsStr::new(programs.pkexec), command) {
        return result;
    }

    match ask_password(programs)? {
        Some(password) => {
            let mut output =
                run_sudo_with_password(OsStr::new(programs.sudo), &password, false, command)
                    .map_err(failure)?;
            if !output.status.success()
                && let Some(direct) = direct_fallback
                && is_sudo_command_denied(&output)
            {
                output = run_sudo_with_password(OsStr::new(programs.sudo), &password, true, direct)
                    .map_err(failure)?;
                if is_sudo_env_denied(&output) {
                    log_output("deb_update_env_unavailable", &output);
                    return Err(sudo_env_preservation_unavailable());
                }
            }
            if output.status.success() {
                Ok(())
            } else {
                Err(failed(&output))
            }
        }
        None => run_sudo_non_interactive(OsStr::new(programs.sudo), command, direct_fallback),
    }
}

fn run_sudo_non_interactive(
    sudo_bin: &OsStr,
    command: &[&OsStr],
    direct_fallback: Option<&[&OsStr]>,
) -> Result<()> {
    let run = |args: &[&OsStr], preserve_env: bool| {
        let mut cmd = Command::new(sudo_bin);
        cmd.env("DEBIAN_FRONTEND", "noninteractive")
            .env("LC_ALL", "C")
            .arg("-n");
        if preserve_env {
            cmd.arg("--preserve-env=DEBIAN_FRONTEND,LC_ALL");
        }
        cmd.args(args).stdin(Stdio::null()).output()
    };
    let mut output = match run(command, false) {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            tracing::info!(%error, "sudo_unavailable");
            return Err(non_interactive_auth_unavailable());
        }
        Err(error) => return Err(failure(error)),
    };

    if !output.status.success()
        && let Some(direct) = direct_fallback
        && (is_non_interactive_sudo_auth_failure(&output) || is_sudo_command_denied(&output))
    {
        output = run(direct, true).map_err(failure)?;
        if is_sudo_env_denied(&output) {
            log_output("deb_update_env_unavailable", &output);
            return Err(sudo_env_preservation_unavailable());
        }
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

fn is_sudo_env_denied(output: &Output) -> bool {
    let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
    stderr.contains("not allowed to set the following environment variables")
        || stderr.contains("not allowed to preserve")
}

fn sudo_env_preservation_unavailable() -> ServiceError {
    install_failed(
        "sudo policy does not permit running apt noninteractively; update Anarlog using `apt`",
    )
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

fn non_interactive_auth_unavailable() -> ServiceError {
    install_failed(
        "no graphical authentication agent (polkit, zenity, or kdialog) or passwordless sudo is available; install a graphical authentication agent or update Anarlog using `apt`",
    )
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
    install_failed(&install_failure_message(output))
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

/// Unlike the refusal, this can't promise the current version is kept: apt may have unpacked
/// the package by the time it fails.
fn install_failed(reason: &str) -> ServiceError {
    failure(format!("The update failed to install: {reason}"))
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
        .map_or_else(|| status.to_string(), |line| (*line).to_owned())
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
    use std::{
        os::unix::{fs::PermissionsExt, process::ExitStatusExt},
        process::ExitStatus,
    };

    use super::*;

    fn fake_script(dir: &Path, name: &str, body: &str) -> String {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.into_os_string().into_string().unwrap()
    }

    // A deb whose dependency apt can't install must be refused before anything asks for a
    // password: `pkexec dpkg -i` would unpack it over the installed app and leave it
    // unconfigured.
    #[test]
    fn deb_update_apt_cannot_satisfy_is_refused_before_any_password_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let prompted = dir.path().join("prompted");
        let apt_get = fake_script(
            dir.path(),
            "apt-get",
            "echo 'The following packages have unmet dependencies:'\n\
             echo ' anarlog : Depends: libanarlog-missing0 but it is not installable'\n\
             echo 'E: Unable to correct problems, you have held broken packages.' >&2\n\
             exit 100",
        );
        let env = fake_script(dir.path(), "env", "exec /usr/bin/env \"$@\"");
        let escalation = |name: &str| {
            fake_script(
                dir.path(),
                name,
                &format!("echo {name} >> '{}'", prompted.display()),
            )
        };
        let (pkexec, sudo, zenity, kdialog) = (
            escalation("pkexec"),
            escalation("sudo"),
            escalation("zenity"),
            escalation("kdialog"),
        );
        let programs = Programs {
            apt_get: &apt_get,
            env: &env,
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

    #[test]
    fn deb_simulation_and_install_preserve_env_across_pkexec() {
        let dir = tempfile::tempdir().unwrap();
        let sim_log = dir.path().join("sim.log");
        let apt_get = fake_script(
            dir.path(),
            "fake-apt",
            &format!(
                "printf '%s\\n%s\\n%s\\n' \"$LC_ALL\" \"$DEBIAN_FRONTEND\" \"$*\" > \"{}\"",
                sim_log.display()
            ),
        );
        let pkexec = fake_script(dir.path(), "fake-pkexec", "exec /usr/bin/env -i \"$@\"");
        let missing = dir.path().join("missing");
        let missing = missing.to_str().unwrap();
        let programs = Programs {
            apt_get: &apt_get,
            env: "/usr/bin/env",
            pkexec: &pkexec,
            sudo: missing,
            prompts: [missing, missing],
        };
        let package = dir.path().join("update.deb");

        install_with(&programs, &package).unwrap();

        let install_recorded = std::fs::read_to_string(&sim_log).unwrap();
        assert!(
            install_recorded.starts_with("C\nnoninteractive\n"),
            "{install_recorded}"
        );
        assert!(
            install_recorded.contains(&package.display().to_string()),
            "{install_recorded}"
        );

        let sim_output = simulate_apt_install_with(OsStr::new(&apt_get), &package).unwrap();
        assert!(sim_output.status.success());
        let sim_recorded = std::fs::read_to_string(&sim_log).unwrap();
        assert!(sim_recorded.starts_with("C\n"), "{sim_recorded}");
        assert!(sim_recorded.contains("--simulate"), "{sim_recorded}");
    }

    #[test]
    fn apt_only_policies_fall_back_from_env_wrapper_to_direct_apt() {
        let dir = tempfile::tempdir().unwrap();
        let ran = dir.path().join("ran.log");
        let fake_apt = fake_script(
            dir.path(),
            "apt-get",
            &format!(
                "printf '%s|%s|%s\\n' \"$DEBIAN_FRONTEND\" \"$LC_ALL\" \"$0 $*\" >> \"{}\"",
                ran.display()
            ),
        );
        let package = dir.path().join("update.deb");
        let direct = apt_direct_command(OsStr::new(&fake_apt), &package);
        let wrapped = apt_env_command(OsStr::new("/usr/bin/env"), &direct);

        let policy_pkexec = fake_script(
            dir.path(),
            "pkexec",
            &format!("if [ \"$1\" != \"{fake_apt}\" ]; then exit 127; fi\nexec \"$@\""),
        );
        assert!(run_pkexec(OsStr::new(&policy_pkexec), &wrapped).is_none());

        let policy_sudo = fake_script(
            dir.path(),
            "sudo",
            &format!(
                "if [ \"$1\" != \"-n\" ]; then exit 99; fi\nshift\n\
                 if read -r _unused; then exit 98; fi\n\
                 preserved=0\n\
                 if [ \"$1\" = \"--preserve-env=DEBIAN_FRONTEND,LC_ALL\" ]; then preserved=1; shift; fi\n\
                 if [ \"$1\" != \"{fake_apt}\" ]; then\n  \
                   echo \"Sorry, user is not allowed to execute '$1' as root.\" >&2\n  \
                   exit 1\n\
                 fi\n\
                 if [ \"$preserved\" = \"1\" ]; then\n  \
                   exec /usr/bin/env -i \"PATH=/usr/bin:/bin\" \"DEBIAN_FRONTEND=$DEBIAN_FRONTEND\" \"LC_ALL=$LC_ALL\" \"$@\"\n\
                 fi\n\
                 exec /usr/bin/env -i \"PATH=/usr/bin:/bin\" \"$@\""
            ),
        );
        run_sudo_non_interactive(OsStr::new(&policy_sudo), &wrapped, Some(&direct)).unwrap();

        let deny_env_sudo = fake_script(
            dir.path(),
            "sudo-deny-env",
            "if [ \"$1\" != \"-n\" ]; then exit 99; fi\nshift\n\
             if read -r _unused; then exit 98; fi\n\
             if [ \"$1\" = \"--preserve-env=DEBIAN_FRONTEND,LC_ALL\" ]; then\n  \
               echo \"sudo: sorry, you are not allowed to set the following environment variables: DEBIAN_FRONTEND, LC_ALL\" >&2\n  \
               exit 1\n\
             fi\n\
             echo \"Sorry, user is not allowed to execute '$1' as root.\" >&2\nexit 1",
        );
        let env_err = run_sudo_non_interactive(OsStr::new(&deny_env_sudo), &wrapped, Some(&direct))
            .unwrap_err()
            .to_string();
        assert!(env_err.contains("update Anarlog using `apt`"), "{env_err}");

        let recorded = std::fs::read_to_string(&ran).unwrap();
        let lines = recorded.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 1, "{recorded}");
        assert!(
            lines[0].starts_with(&format!("noninteractive|C|{fake_apt} ")),
            "{recorded}"
        );
    }

    #[test]
    fn non_interactive_sudo_fallback_and_stale_index_error_hint() {
        let dir = tempfile::tempdir().unwrap();
        let fake_sudo = fake_script(
            dir.path(),
            "fake-sudo",
            "if [ \"$1\" != \"-n\" ]; then exit 99; fi\n\
             if read -r _unused; then exit 98; fi\n\
             echo 'sudo: a password is required' >&2\nexit 1",
        );

        let auth_err =
            run_sudo_non_interactive(OsStr::new(&fake_sudo), &[OsStr::new("/bin/true")], None)
                .unwrap_err()
                .to_string();
        assert!(auth_err.contains("polkit"), "{auth_err}");
        assert!(auth_err.contains("passwordless sudo"), "{auth_err}");

        let missing_err = run_sudo_non_interactive(
            dir.path().join("missing-sudo").as_os_str(),
            &[OsStr::new("/bin/true")],
            None,
        )
        .unwrap_err()
        .to_string();
        assert!(missing_err.contains("passwordless sudo"), "{missing_err}");

        let failure_output = |stderr: &[u8]| Output {
            status: ExitStatus::from_raw(100 << 8),
            stdout: Vec::new(),
            stderr: stderr.to_vec(),
        };
        for stderr in [
            b"Err:1 http://deb.debian.org bookworm InRelease\nE: Unable to fetch some archives, maybe run apt-get update?".as_slice(),
            b"E: Failed to fetch http://example.com/pkg.deb  404  Not Found".as_slice(),
        ] {
            let msg = install_failure_message(&failure_output(stderr));
            assert!(msg.contains("sudo apt update"), "{msg}");
        }

        let lock_msg = install_failure_message(&failure_output(
            b"E: Could not get lock /var/lib/dpkg/lock-frontend. It is held by process 1404 (unattended-upgr)",
        ));
        assert!(!lock_msg.contains("sudo apt update"), "{lock_msg}");
    }
}
