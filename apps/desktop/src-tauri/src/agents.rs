#[cfg(any(not(feature = "app-store"), test))]
use std::path::Path;

#[cfg(any(not(feature = "app-store"), test))]
const AGENTS_CONTENT: &str = include_str!("agents-content.md");

#[cfg(any(not(feature = "app-store"), test))]
pub fn write_agents_file(base_dir: &Path) -> std::io::Result<()> {
    let agents_path = base_dir.join("AGENTS.md");
    std::fs::write(agents_path, AGENTS_CONTENT)
}
