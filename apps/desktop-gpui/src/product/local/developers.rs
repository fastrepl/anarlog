//! The Developers page's local tools, ported from shipping's `embedded_cli.rs` and
//! `agent_skills.rs`: the bundled CLI installed as a managed symlink, and the Anarlog agent skill.

use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliState {
    Installed,
    Missing,
    Conflict,
    Unsupported,
    ResourceMissing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliStatus {
    pub supported: bool,
    pub command_name: String,
    pub install_path: String,
    pub state: CliState,
    pub details: Option<String>,
}

#[derive(Clone)]
pub struct DeveloperTools {
    pub bundled_cli: PathBuf,
    pub home: PathBuf,
    pub command_name: &'static str,
    pub version: &'static str,
}

impl DeveloperTools {
    pub fn install_path(&self) -> PathBuf {
        self.home.join(".local/bin").join(self.command_name)
    }

    pub fn cli_status(&self) -> CliStatus {
        let install_path = self.install_path();
        let status = |supported, state, details: Option<&str>| CliStatus {
            supported,
            command_name: self.command_name.into(),
            install_path: install_path.display().to_string(),
            state,
            details: details.map(Into::into),
        };
        if !cfg!(unix) {
            return status(
                false,
                CliState::Unsupported,
                Some("Bundled CLI installation is not yet available on this platform."),
            );
        }
        if !self.bundled_cli.is_file() {
            return status(
                true,
                CliState::ResourceMissing,
                Some("The CLI is not included in this build of Anarlog."),
            );
        }
        match classify(&install_path, &self.managed_path()) {
            Ok(state) => status(true, state, details(state, &install_path).as_deref()),
            Err(error) => status(true, CliState::Conflict, Some(&error)),
        }
    }

    pub fn install_cli(&self) -> Result<CliStatus, String> {
        let status = self.cli_status();
        match status.state {
            CliState::Unsupported | CliState::ResourceMissing => return Ok(status),
            CliState::Conflict => {
                return Err(format!(
                    "Another file already exists at {}. Move it before installing the Anarlog CLI.",
                    status.install_path
                ));
            }
            CliState::Installed | CliState::Missing => {}
        }
        install_managed(
            &self.bundled_cli,
            &self.managed_path(),
            &self.install_path(),
        )?;
        Ok(self.cli_status())
    }

    fn managed_path(&self) -> PathBuf {
        self.home
            .join(".local/bin/.anarlog-cli")
            .join(self.command_name)
            .join(self.version)
    }

    pub fn skill_agents(&self) -> Vec<SkillAgentStatus> {
        SkillAgent::ALL
            .into_iter()
            .map(|agent| self.skill_status(agent))
            .collect()
    }

    pub fn install_skill(&self, agent: SkillAgent) -> Result<SkillAgentStatus, String> {
        if !agent.config_dir(&self.home).is_dir() {
            return Err(format!(
                "{} was not found on this machine.",
                agent.display_name()
            ));
        }
        let directory = agent.skill_dir(&self.home);
        for (relative, content) in SKILL_FILES {
            let target = directory.join(relative);
            let parent = target
                .parent()
                .ok_or_else(|| "The skill install path is invalid.".to_string())?;
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
            std::fs::write(&target, content)
                .map_err(|error| format!("Could not write {}: {error}", target.display()))?;
        }
        Ok(self.skill_status(agent))
    }

    fn skill_status(&self, agent: SkillAgent) -> SkillAgentStatus {
        SkillAgentStatus {
            agent,
            detected: agent.config_dir(&self.home).is_dir(),
            installed: agent.skill_dir(&self.home).join("SKILL.md").is_file(),
        }
    }
}

/// `buildMcpConfiguration`: the client entry that runs `<command> mcp`, in shipping's key order.
pub fn mcp_configuration(command: &str) -> String {
    #[derive(Serialize)]
    struct Config<'a> {
        #[serde(rename = "mcpServers")]
        servers: Servers<'a>,
    }
    #[derive(Serialize)]
    struct Servers<'a> {
        anarlog: Server<'a>,
    }
    #[derive(Serialize)]
    struct Server<'a> {
        command: &'a str,
        args: [&'a str; 1],
    }
    serde_json::to_string_pretty(&Config {
        servers: Servers {
            anarlog: Server {
                command,
                args: ["mcp"],
            },
        },
    })
    .unwrap_or_default()
}

fn details(state: CliState, install_path: &Path) -> Option<String> {
    let path = install_path.display();
    match state {
        CliState::Installed => Some(format!("Installed at {path} and managed by Anarlog.")),
        CliState::Missing => Some(format!("Install the command at {path}.")),
        CliState::Conflict => Some(format!("Another file already exists at {path}.")),
        CliState::Unsupported | CliState::ResourceMissing => None,
    }
}

fn classify(install_path: &Path, managed_path: &Path) -> Result<CliState, String> {
    let metadata = match std::fs::symlink_metadata(install_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CliState::Missing);
        }
        Err(error) => {
            return Err(format!(
                "Failed to inspect {}: {error}",
                install_path.display()
            ));
        }
    };
    if !metadata.file_type().is_symlink() {
        return Ok(CliState::Conflict);
    }
    let target = std::fs::read_link(install_path).map_err(|error| {
        format!(
            "Failed to inspect the installed command at {}: {error}",
            install_path.display()
        )
    })?;
    if !replaceable_target(&target, managed_path) {
        return Ok(CliState::Conflict);
    }
    if target != managed_path {
        return Ok(CliState::Missing);
    }
    let managed = match std::fs::symlink_metadata(managed_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CliState::Missing);
        }
        Err(error) => {
            return Err(format!(
                "Failed to resolve the managed CLI at {}: {error}",
                managed_path.display()
            ));
        }
    };
    if !managed.file_type().is_file() || !executable(&managed) {
        return Ok(CliState::Missing);
    }
    Ok(CliState::Installed)
}

#[cfg(unix)]
fn executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o100 != 0
}

#[cfg(not(unix))]
fn executable(_: &std::fs::Metadata) -> bool {
    false
}

/// Another Anarlog version's managed binary, or (macOS) a CLI inside an Anarlog app bundle.
fn replaceable_target(target: &Path, managed_path: &Path) -> bool {
    if managed_path
        .parent()
        .is_some_and(|directory| target.parent() == Some(directory))
    {
        return true;
    }
    cfg!(target_os = "macos") && legacy_app_target(target)
}

fn legacy_app_target(target: &Path) -> bool {
    let name = |path: Option<&Path>| {
        path.and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .map(str::to_owned)
    };
    if !matches!(
        name(Some(target)).as_deref(),
        Some(
            "anarlog-cli" | "anarlog-cli-aarch64-apple-darwin" | "anarlog-cli-x86_64-apple-darwin"
        )
    ) {
        return false;
    }
    let parent = target.parent();
    let contents = match name(parent).as_deref() {
        Some("MacOS" | "Resources") => parent.and_then(Path::parent),
        Some("cli") => parent
            .and_then(Path::parent)
            .filter(|path| name(Some(path)).as_deref() == Some("Resources"))
            .and_then(Path::parent),
        _ => None,
    };
    let app = contents
        .filter(|path| name(Some(path)).as_deref() == Some("Contents"))
        .and_then(Path::parent);
    matches!(
        name(app).as_deref(),
        Some("Anarlog.app" | "Anarlog Staging.app" | "Anarlog Dev.app")
    )
}

#[cfg(unix)]
fn temporary(path: &Path) -> Result<PathBuf, String> {
    let name = path
        .file_name()
        .ok_or_else(|| "The CLI install path is invalid.".to_string())?;
    let temp = path.with_file_name(format!(
        ".{}.tmp-{}",
        name.to_string_lossy(),
        std::process::id()
    ));
    if std::fs::symlink_metadata(&temp).is_ok() {
        std::fs::remove_file(&temp).map_err(|error| {
            format!(
                "Could not prepare the CLI update at {}: {error}",
                temp.display()
            )
        })?;
    }
    Ok(temp)
}

#[cfg(unix)]
fn install_managed(resource: &Path, managed: &Path, install: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let directory = managed
        .parent()
        .ok_or_else(|| "The managed CLI directory is invalid.".to_string())?;
    std::fs::create_dir_all(directory)
        .map_err(|error| format!("Could not create {}: {error}", directory.display()))?;
    let temp = temporary(managed)?;
    std::fs::copy(resource, &temp).map_err(|error| {
        format!(
            "Could not copy the bundled CLI to {}: {error}",
            temp.display()
        )
    })?;
    let result = std::fs::metadata(&temp)
        .and_then(|metadata| {
            let mut permissions = metadata.permissions();
            permissions.set_mode(permissions.mode() | 0o100);
            std::fs::set_permissions(&temp, permissions)
        })
        .map_err(|error| {
            format!(
                "Could not make the CLI executable at {}: {error}",
                temp.display()
            )
        })
        .and_then(|()| {
            std::fs::rename(&temp, managed).map_err(|error| {
                format!(
                    "Could not install the managed CLI at {}: {error}",
                    managed.display()
                )
            })
        });
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result?;

    let directory = install
        .parent()
        .ok_or_else(|| "The CLI install directory is invalid.".to_string())?;
    std::fs::create_dir_all(directory)
        .map_err(|error| format!("Could not create {}: {error}", directory.display()))?;
    let temp = temporary(install)?;
    std::os::unix::fs::symlink(managed, &temp).map_err(|error| {
        format!(
            "Could not prepare the command at {}: {error}",
            temp.display()
        )
    })?;
    // Re-checked right before replacing it, in case something else appeared meanwhile.
    let replaceable = match std::fs::symlink_metadata(install) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Ok(metadata) if metadata.file_type().is_symlink() => {
            std::fs::read_link(install).is_ok_and(|target| replaceable_target(&target, managed))
        }
        _ => false,
    };
    let result = if replaceable {
        std::fs::rename(&temp, install).map_err(|error| {
            format!(
                "Could not install the command at {}: {error}",
                install.display()
            )
        })
    } else {
        Err(format!(
            "Another file already exists at {}.",
            install.display()
        ))
    };
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

#[cfg(not(unix))]
fn install_managed(_: &Path, _: &Path, _: &Path) -> Result<(), String> {
    Err("Bundled CLI installation is not yet available on this platform.".into())
}

/// The published `skills/anarlog` package, embedded so installs match this build.
const SKILL_FILES: &[(&str, &str)] = &[
    (
        "SKILL.md",
        include_str!("../../../../../skills/anarlog/SKILL.md"),
    ),
    (
        "references/cli.md",
        include_str!("../../../../../skills/anarlog/references/cli.md"),
    ),
    (
        "references/errors.md",
        include_str!("../../../../../skills/anarlog/references/errors.md"),
    ),
    (
        "references/mcp.md",
        include_str!("../../../../../skills/anarlog/references/mcp.md"),
    ),
    (
        "references/setup.md",
        include_str!("../../../../../skills/anarlog/references/setup.md"),
    ),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillAgent {
    ClaudeCode,
    Codex,
    Cursor,
    Opencode,
}

impl SkillAgent {
    pub const ALL: [Self; 4] = [Self::ClaudeCode, Self::Codex, Self::Cursor, Self::Opencode];

    pub fn display_name(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::Codex => "Codex",
            Self::Cursor => "Cursor",
            Self::Opencode => "OpenCode",
        }
    }

    /// The agent's config directory doubles as its detection signal.
    fn config_dir(self, home: &Path) -> PathBuf {
        match self {
            Self::ClaudeCode => home.join(".claude"),
            Self::Codex => home.join(".codex"),
            Self::Cursor => home.join(".cursor"),
            Self::Opencode => std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .unwrap_or_else(|| home.join(".config"))
                .join("opencode"),
        }
    }

    fn skill_dir(self, home: &Path) -> PathBuf {
        self.config_dir(home).join("skills/anarlog")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkillAgentStatus {
    pub agent: SkillAgent,
    pub detected: bool,
    pub installed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools(home: &Path) -> DeveloperTools {
        let bundled = home.join("resources/anarlog");
        std::fs::create_dir_all(bundled.parent().unwrap()).unwrap();
        std::fs::write(&bundled, b"fixture-cli").unwrap();
        DeveloperTools {
            bundled_cli: bundled,
            home: home.into(),
            command_name: "anarlog",
            version: "2.0.0",
        }
    }

    #[cfg(unix)]
    #[test]
    fn cli_install_replaces_older_managed_versions_but_never_a_user_file() {
        let home = tempfile::tempdir().unwrap();
        let tools = tools(home.path());
        let install = tools.install_path();
        // An older app version's managed link is upgraded in place.
        let old = home.path().join(".local/bin/.anarlog-cli/anarlog/1.0.0");
        std::fs::create_dir_all(old.parent().unwrap()).unwrap();
        std::fs::write(&old, b"old").unwrap();
        std::os::unix::fs::symlink(&old, &install).unwrap();
        assert_eq!(tools.cli_status().state, CliState::Missing);
        let status = tools.install_cli().unwrap();
        assert_eq!(status.state, CliState::Installed);
        assert_eq!(std::fs::read(&install).unwrap(), b"fixture-cli");
        assert!(mcp_configuration(&status.install_path).contains(&status.install_path));

        std::fs::remove_file(&install).unwrap();
        std::fs::write(&install, b"user binary").unwrap();
        assert_eq!(tools.cli_status().state, CliState::Conflict);
        assert!(tools.install_cli().is_err());
        assert_eq!(std::fs::read(&install).unwrap(), b"user binary");
    }

    #[test]
    fn skills_install_only_into_detected_agents_and_refresh_stale_files() {
        let home = tempfile::tempdir().unwrap();
        let tools = tools(home.path());
        assert!(tools.install_skill(SkillAgent::Codex).is_err());
        std::fs::create_dir(home.path().join(".codex")).unwrap();
        let skill = home.path().join(".codex/skills/anarlog/SKILL.md");
        std::fs::create_dir_all(skill.parent().unwrap()).unwrap();
        std::fs::write(&skill, "outdated").unwrap();
        let status = tools.install_skill(SkillAgent::Codex).unwrap();
        assert!(status.detected && status.installed);
        assert_eq!(std::fs::read_to_string(&skill).unwrap(), SKILL_FILES[0].1);
    }
}
