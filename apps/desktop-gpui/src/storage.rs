//! The General settings' storage location: `settings/general/storage/path-utils.ts`
//! and the `AGENTS.md` the Tauri app drops into the vault on startup.

use std::path::Path;

/// `apps/desktop/src-tauri/src/agents-content.md`, verbatim.
const AGENTS_CONTENT: &str = include_str!("../../desktop/src-tauri/src/agents-content.md");

/// `agents::write_agents_file(vault_base)`: the guidance file agents find
/// when they open the vault, rewritten on every start.
pub fn write_agents_file(vault_base: &Path) -> std::io::Result<()> {
    std::fs::write(vault_base.join("AGENTS.md"), AGENTS_CONTENT)
}

fn tildify(path: &str, home: &str) -> String {
    match path.strip_prefix(&format!("{home}/")) {
        Some(rest) => format!("~/{rest}"),
        None => path.to_string(),
    }
}

/// `shortenPath`: the last `max_length` characters, cut to the first `/`
/// inside them and led by an ellipsis.
fn shorten_path(path: &str, max_length: usize) -> String {
    let chars: Vec<char> = path.chars().collect();
    if chars.len() <= max_length {
        return path.to_string();
    }
    let short: String = chars[chars.len() - max_length..].iter().collect();
    match short.find('/') {
        Some(slash) if slash > 0 => format!("\u{2026}{}", &short[slash..]),
        _ => format!("\u{2026}{short}"),
    }
}

/// `displayPath(path, home)`: `~`-relative and at most 48 characters.
pub fn display_path(path: &str, home: Option<&str>) -> String {
    let tildified = match home {
        Some(home) if !home.is_empty() => tildify(path, home),
        _ => path.to_string(),
    };
    shorten_path(&tildified, 48)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_path_tildifies_and_shortens_like_the_web_helper() {
        assert_eq!(
            display_path("/home/ann/.local/share/Anarlog", Some("/home/ann")),
            "~/.local/share/Anarlog"
        );
        // The home folder itself is not a prefix match (`home + "/"`).
        assert_eq!(display_path("/home/ann", Some("/home/ann")), "/home/ann");
        assert_eq!(display_path("/srv/data", None), "/srv/data");
        let long = "/home/ann/Documents/Projects/Anarlog/vaults/team/quarterly-planning";
        let shown = display_path(long, Some("/home/ann"));
        assert!(shown.starts_with("\u{2026}/"), "{shown}");
        assert!(shown.chars().count() <= 49, "{shown}");
        assert!(
            long.ends_with(shown.trim_start_matches('\u{2026}')),
            "{shown}"
        );
        // Without a slash in the tail the ellipsis leads the raw tail.
        let flat = format!("/{}", "x".repeat(60));
        assert_eq!(
            display_path(&flat, None),
            format!("\u{2026}{}", "x".repeat(48))
        );
    }

    #[test]
    fn agents_guidance_is_the_tauri_apps() {
        assert!(AGENTS_CONTENT.contains("anarlog --json meetings list"));
        assert!(AGENTS_CONTENT.contains("https://docs.anarlog.so/skill.md"));
    }
}
