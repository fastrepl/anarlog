use std::path::{Path, PathBuf};

pub fn plugins_dir() -> PathBuf {
    dirs::config_dir()
        .expect("could not determine config directory")
        .join("opencode")
        .join("plugins")
}

pub fn plugin_path() -> PathBuf {
    plugins_dir().join("char.ts")
}

pub fn write_plugin(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    std::fs::write(path, PLUGIN_CONTENTS)
        .map_err(|e| format!("failed to write {}: {e}", path.display()))
}

pub fn remove_plugin(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("failed to remove {}: {e}", path.display())),
    }
}

pub fn has_char_plugin(path: &Path) -> Result<bool, String> {
    match std::fs::read_to_string(path) {
        Ok(contents) => Ok(contents == PLUGIN_CONTENTS),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("failed to read {}: {e}", path.display())),
    }
}

pub fn is_char_plugin(path: &Path) -> Result<bool, String> {
    match std::fs::read_to_string(path) {
        Ok(contents) => Ok(contents.contains("char")
            && contents.contains("opencode")
            && contents.contains("notify")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("failed to read {}: {e}", path.display())),
    }
}

const PLUGIN_CONTENTS: &str = r#"import type { Plugin } from "@opencode-ai/plugin";

export const CharPlugin: Plugin = async () => {
  return {
    event: async ({ event }) => {
      if (event.type !== "session.idle") {
        return;
      }

      const child = Bun.spawn(["char", "opencode", "notify", JSON.stringify(event)], {
        stdout: "inherit",
        stderr: "inherit",
      });

      const exitCode = await child.exited;
      if (exitCode !== 0) {
        throw new Error(`char opencode notify exited with code ${exitCode}`);
      }
    },
  };
};
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_exact_char_plugin_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("char.ts");

        std::fs::write(&path, PLUGIN_CONTENTS).unwrap();

        assert!(has_char_plugin(&path).unwrap());
    }

    #[test]
    fn ignores_different_plugin_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("char.ts");

        std::fs::write(&path, "export const plugin = {};\n").unwrap();

        assert!(!has_char_plugin(&path).unwrap());
    }

    #[test]
    fn is_char_plugin_detects_current_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("char.ts");

        std::fs::write(&path, PLUGIN_CONTENTS).unwrap();

        assert!(is_char_plugin(&path).unwrap());
    }

    #[test]
    fn is_char_plugin_detects_outdated_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("char.ts");

        let old_plugin = r#"
            const child = Bun.spawn(["char", "opencode", "notify"]);
        "#;
        std::fs::write(&path, old_plugin).unwrap();

        assert!(is_char_plugin(&path).unwrap());
    }

    #[test]
    fn is_char_plugin_rejects_unrelated_plugin() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("char.ts");

        std::fs::write(&path, "export const plugin = {};\n").unwrap();

        assert!(!is_char_plugin(&path).unwrap());
    }
}
