use std::{
    fs,
    path::{Path, PathBuf},
};

use desktop_runtime::Result;
use serde_json::{Value, json};

use super::{failure, storage::install_copy};

#[derive(Clone)]
pub struct DeveloperTools {
    pub bundled_cli: PathBuf,
    pub installed_cli: PathBuf,
    pub home: PathBuf,
    pub skills_bundle: PathBuf,
}

impl DeveloperTools {
    pub fn cli_installed(&self) -> Result<bool> {
        match fs::symlink_metadata(&self.installed_cli) {
            Ok(meta) if !meta.is_file() => Err(failure("CLI destination is not a regular file")),
            Ok(_) => Ok(fs::read(&self.installed_cli).map_err(failure)?
                == fs::read(&self.bundled_cli).map_err(failure)?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(failure(error)),
        }
    }

    pub fn install_cli(&self) -> Result<()> {
        if self.cli_installed()? {
            return Ok(());
        }
        install_copy(&self.bundled_cli, &self.installed_cli)
    }

    pub fn mcp_config(&self) -> Value {
        json!({"mcpServers":{"anarlog":{"command":self.installed_cli,"args":["mcp"]}}})
    }

    pub fn install_mcp(&self, target: &Path) -> Result<()> {
        if !self.cli_installed()? {
            return Err(failure("Install the bundled CLI first"));
        }
        let mut config: Value = match fs::read_to_string(target) {
            Ok(content) => serde_json::from_str(&content).map_err(failure)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => json!({}),
            Err(error) => return Err(failure(error)),
        };
        let object = config
            .as_object_mut()
            .ok_or_else(|| failure("MCP configuration is not an object"))?;
        let servers = object
            .entry("mcpServers")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| failure("mcpServers is not an object"))?;
        let ours = self.mcp_config()["mcpServers"]["anarlog"].clone();
        if servers.get("anarlog") == Some(&ours) {
            return Ok(());
        }
        if servers
            .get("anarlog")
            .is_some_and(|existing| existing != &ours)
        {
            return Err(failure(
                "Anarlog MCP configuration already exists with different settings",
            ));
        }
        servers.insert("anarlog".into(), ours);
        if target.exists() {
            let backup = target.with_extension("anarlog-backup.json");
            install_copy(target, &backup)?;
        }
        anlg_storage::fs::atomic_write(
            target,
            &serde_json::to_string_pretty(&config).map_err(failure)?,
        )
        .map_err(failure)
    }

    pub fn agent_directory(&self, agent: &str) -> Result<PathBuf> {
        Ok(match agent {
            "claude_code" => self.home.join(".claude"),
            "codex" => self.home.join(".codex"),
            "cursor" => self.home.join(".cursor"),
            "opencode" => std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .unwrap_or_else(|| self.home.join(".config"))
                .join("opencode"),
            _ => return Err(failure("Unknown agent")),
        })
    }

    pub fn install_skills(&self, agent: &str) -> Result<()> {
        let config = self.agent_directory(agent)?;
        if !config.is_dir() {
            return Err(failure("Agent configuration directory does not exist"));
        }
        let destination = config.join("skills/anarlog");
        for relative in [
            "SKILL.md",
            "references/cli.md",
            "references/errors.md",
            "references/mcp.md",
            "references/setup.md",
        ] {
            let source = fs::read(self.skills_bundle.join(relative)).map_err(failure)?;
            let target = destination.join(relative);
            if target.exists()
                && (fs::symlink_metadata(&target)
                    .map_err(failure)?
                    .file_type()
                    .is_symlink()
                    || source != fs::read(target).map_err(failure)?)
            {
                return Err(failure(
                    "Installed skill differs; preserve your changes before reinstalling",
                ));
            }
        }
        for relative in [
            "SKILL.md",
            "references/cli.md",
            "references/errors.md",
            "references/mcp.md",
            "references/setup.md",
        ] {
            let source = self.skills_bundle.join(relative);
            let target = destination.join(relative);
            if target.exists() {
                if fs::read(&source).map_err(failure)? == fs::read(&target).map_err(failure)? {
                    continue;
                }
                return Err(failure(
                    "Installed skill differs; preserve your changes before reinstalling",
                ));
            }
            install_copy(&source, &target)?;
        }
        Ok(())
    }
}
