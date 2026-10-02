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

    fn write_temp_plugin(dir: &tempfile::TempDir, contents: &str) -> std::path::PathBuf {
        let path = dir.path().join("char.ts");
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn has_char_plugin_requires_exact_contents() {
        let current_dir = tempfile::tempdir().unwrap();
        let current_path = write_temp_plugin(&current_dir, PLUGIN_CONTENTS);
        let other_dir = tempfile::tempdir().unwrap();
        let other_path = write_temp_plugin(&other_dir, "export const plugin = {};\n");

        assert!(has_char_plugin(&current_path).unwrap());
        assert!(!has_char_plugin(&other_path).unwrap());
    }

    #[test]
    fn is_char_plugin_detects_current_and_outdated_but_not_unrelated() {
        let current_dir = tempfile::tempdir().unwrap();
        let current_path = write_temp_plugin(&current_dir, PLUGIN_CONTENTS);
        let outdated_dir = tempfile::tempdir().unwrap();
        let outdated_path = write_temp_plugin(
            &outdated_dir,
            r#"
            const child = Bun.spawn(["char", "opencode", "notify"]);
        "#,
        );
        let unrelated_dir = tempfile::tempdir().unwrap();
        let unrelated_path = write_temp_plugin(&unrelated_dir, "export const plugin = {};\n");

        assert!(is_char_plugin(&current_path).unwrap());
        assert!(is_char_plugin(&outdated_path).unwrap());
        assert!(!is_char_plugin(&unrelated_path).unwrap());
    }
}
