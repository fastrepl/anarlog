use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const SKILL_DIR_NAME: &str = "anarlog";

// The published skill package from `skills/anarlog`, embedded at build time so
// installs work offline and always match the running app version.
const SKILL_FILES: &[(&str, &str)] = &[
    (
        "SKILL.md",
        include_str!("../../../../skills/anarlog/SKILL.md"),
    ),
    (
        "references/cli.md",
        include_str!("../../../../skills/anarlog/references/cli.md"),
    ),
    (
        "references/errors.md",
        include_str!("../../../../skills/anarlog/references/errors.md"),
    ),
    (
        "references/mcp.md",
        include_str!("../../../../skills/anarlog/references/mcp.md"),
    ),
    (
        "references/setup.md",
        include_str!("../../../../skills/anarlog/references/setup.md"),
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SkillAgent {
    ClaudeCode,
    Codex,
    Cursor,
    Opencode,
}

const AGENTS: [SkillAgent; 4] = [
    SkillAgent::ClaudeCode,
    SkillAgent::Codex,
    SkillAgent::Cursor,
    SkillAgent::Opencode,
];

impl SkillAgent {
    fn display_name(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::Codex => "Codex",
            Self::Cursor => "Cursor",
            Self::Opencode => "OpenCode",
        }
    }

    // The config directory doubles as the detection signal: it only exists
    // once the agent has been used on this machine.
    fn config_dir(self, home: &Path) -> PathBuf {
        match self {
            Self::ClaudeCode => home.join(".claude"),
            Self::Codex => home.join(".codex"),
            Self::Cursor => home.join(".cursor"),
            Self::Opencode => {
                opencode_config_dir(home, std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
            }
        }
    }
}

// OpenCode resolves its config root from XDG_CONFIG_HOME before falling back
// to ~/.config; a relative value is ignored per the XDG spec.
fn opencode_config_dir(home: &Path, xdg_config_home: Option<PathBuf>) -> PathBuf {
    xdg_config_home
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home.join(".config"))
        .join("opencode")
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SkillAgentStatus {
    pub agent: SkillAgent,
    pub display_name: String,
    pub detected: bool,
    pub installed: bool,
    pub skill_path: String,
}

pub fn list() -> Result<Vec<SkillAgentStatus>, String> {
    let home = home_dir()?;
    Ok(AGENTS
        .into_iter()
        .map(|agent| status(agent, &home))
        .collect())
}

pub fn install(agent: SkillAgent) -> Result<SkillAgentStatus, String> {
    let home = home_dir()?;
    if !agent.config_dir(&home).is_dir() {
        return Err(format!(
            "{} was not found on this machine.",
            agent.display_name()
        ));
    }

    install_at(&skill_dir(agent, &home))?;
    Ok(status(agent, &home))
}

fn home_dir() -> Result<PathBuf, String> {
    dirs::home_dir().ok_or_else(|| "Anarlog could not find your home directory.".to_string())
}

fn skill_dir(agent: SkillAgent, home: &Path) -> PathBuf {
    agent.config_dir(home).join("skills").join(SKILL_DIR_NAME)
}

fn status(agent: SkillAgent, home: &Path) -> SkillAgentStatus {
    let skill_dir = skill_dir(agent, home);
    SkillAgentStatus {
        agent,
        display_name: agent.display_name().to_string(),
        detected: agent.config_dir(home).is_dir(),
        installed: skill_dir.join("SKILL.md").is_file(),
        skill_path: skill_dir.display().to_string(),
    }
}

fn install_at(skill_dir: &Path) -> Result<(), String> {
    for (relative_path, content) in SKILL_FILES {
        let target = skill_dir.join(relative_path);
        let parent = target
            .parent()
            .ok_or_else(|| "The skill install path is invalid.".to_string())?;
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
        std::fs::write(&target, content)
            .map_err(|error| format!("Could not write {}: {error}", target.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeds_the_complete_skill_package() {
        assert!(SKILL_FILES.iter().any(|(path, _)| *path == "SKILL.md"));
        for (path, content) in SKILL_FILES {
            assert!(!content.trim().is_empty(), "{path} is empty");
        }
    }

    #[test]
    fn install_writes_every_skill_file() {
        let home = tempfile::tempdir().unwrap();
        let skill_dir = skill_dir(SkillAgent::ClaudeCode, home.path());

        install_at(&skill_dir).unwrap();

        for (relative_path, content) in SKILL_FILES {
            assert_eq!(
                std::fs::read_to_string(skill_dir.join(relative_path)).unwrap(),
                *content
            );
        }
    }

    #[test]
    fn reinstall_overwrites_stale_content() {
        let home = tempfile::tempdir().unwrap();
        let skill_dir = skill_dir(SkillAgent::Codex, home.path());
        install_at(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "outdated").unwrap();

        install_at(&skill_dir).unwrap();

        assert_ne!(
            std::fs::read_to_string(skill_dir.join("SKILL.md")).unwrap(),
            "outdated"
        );
    }

    #[test]
    fn opencode_config_dir_honors_xdg_config_home() {
        let home = Path::new("/home/user");

        assert_eq!(
            opencode_config_dir(home, None),
            Path::new("/home/user/.config/opencode")
        );
        assert_eq!(
            opencode_config_dir(home, Some(PathBuf::from("/custom/config"))),
            Path::new("/custom/config/opencode")
        );
        assert_eq!(
            opencode_config_dir(home, Some(PathBuf::from("relative/config"))),
            Path::new("/home/user/.config/opencode")
        );
    }

    #[test]
    fn status_reports_detection_and_installation_separately() {
        let home = tempfile::tempdir().unwrap();

        let missing = status(SkillAgent::Cursor, home.path());
        assert!(!missing.detected);
        assert!(!missing.installed);

        std::fs::create_dir_all(SkillAgent::Cursor.config_dir(home.path())).unwrap();
        let detected = status(SkillAgent::Cursor, home.path());
        assert!(detected.detected);
        assert!(!detected.installed);

        install_at(&skill_dir(SkillAgent::Cursor, home.path())).unwrap();
        let installed = status(SkillAgent::Cursor, home.path());
        assert!(installed.installed);
    }
}
