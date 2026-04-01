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

const PLUGIN_CONTENTS: &str = r#"import type { Plugin } from "@opencode-ai/plugin";

export const CharPlugin: Plugin = async ({ $ }) => {
  return {
    "session.idle": async (_input, _output) => {
      await $`char opencode notify`;
    },
  };
};
"#;
