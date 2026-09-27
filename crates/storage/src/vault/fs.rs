use std::path::Path;

use crate::fs::copy_dir_recursive;

const VAULT_DIRECTORIES: &[&str] = &[
    "sessions",
    "humans",
    "organizations",
    "chats",
    "prompts",
    "plugins",
];

const VAULT_FILES: &[&str] = &[
    "AGENTS.md",
    "settings.json",
    "events.json",
    "calendars.json",
    "templates.json",
    "store.json",
];

// The search index is rebuilt from the database whenever it disagrees with it,
// so it is never copied: a stale copy would be opened as if it were current.
const DERIVED_DIRECTORIES: &[&str] = &["search_index"];

// Only artifacts the current app writes are removed from a folder it stops
// using. Legacy-era names such as `prompts` or `humans` are too generic to
// delete from a folder the user also uses for other things.
const OWNED_DIRECTORIES: &[&str] = &["sessions", "search_index"];
const OWNED_FILES: &[&str] = &["settings.json", "store.json"];
const GENERATED_AGENTS_HEADERS: &[&str] = &["# Anarlog", "# Hyprnote"];

/// Copies the vault items of `src` over `dst`; files in `src` win on conflict.
pub fn copy_vault_items(src: &Path, dst: &Path) -> std::io::Result<()> {
    for dir_name in VAULT_DIRECTORIES {
        let src_dir = src.join(dir_name);
        if src_dir.is_dir() {
            let dst_dir = dst.join(dir_name);
            std::fs::create_dir_all(&dst_dir)?;
            copy_dir_recursive(&src_dir, &dst_dir, None)?;
        }
    }

    for file_name in VAULT_FILES {
        let src_file = src.join(file_name);
        if src_file.is_file() {
            std::fs::copy(&src_file, dst.join(file_name))?;
        }
    }

    Ok(())
}

pub fn remove_derived_items(path: &Path) -> std::io::Result<()> {
    for dir_name in DERIVED_DIRECTORIES {
        let dir = path.join(dir_name);
        if dir.is_dir() {
            std::fs::remove_dir_all(&dir)?;
        }
    }
    Ok(())
}

/// Removes the artifacts the app owns from a folder it no longer stores data
/// in, leaving everything else in place.
pub fn remove_owned_items(path: &Path) -> std::io::Result<()> {
    for dir_name in OWNED_DIRECTORIES {
        let dir = path.join(dir_name);
        if dir.is_dir() {
            std::fs::remove_dir_all(&dir)?;
        }
    }

    for file_name in OWNED_FILES {
        let file = path.join(file_name);
        if file.is_file() {
            std::fs::remove_file(&file)?;
        }
    }

    let agents_file = path.join("AGENTS.md");
    if agents_file.is_file() && is_generated_agents_file(&agents_file)? {
        std::fs::remove_file(&agents_file)?;
    }

    Ok(())
}

fn is_generated_agents_file(path: &Path) -> std::io::Result<bool> {
    let content = std::fs::read_to_string(path)?;
    Ok(GENERATED_AGENTS_HEADERS
        .iter()
        .any(|header| content.trim_start().starts_with(header)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn copy_vault_items_copies_only_vault() {
        let temp = tempdir().unwrap();
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");

        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dst).unwrap();

        fs::create_dir_all(src.join("sessions")).unwrap();
        fs::write(src.join("sessions").join("test.json"), "session").unwrap();
        fs::create_dir_all(src.join("humans")).unwrap();
        fs::write(src.join("humans").join("person.md"), "human").unwrap();
        fs::write(src.join("events.json"), "events").unwrap();
        fs::write(src.join("settings.json"), "settings").unwrap();

        fs::write(src.join("store.json"), "store").unwrap();
        fs::create_dir_all(src.join("models")).unwrap();
        fs::write(src.join("models").join("model.gguf"), "model").unwrap();
        fs::create_dir_all(src.join("search_index")).unwrap();
        fs::write(src.join("search_index").join("meta.json"), "{}").unwrap();

        copy_vault_items(&src, &dst).unwrap();

        assert!(dst.join("sessions").join("test.json").exists());
        assert!(dst.join("humans").join("person.md").exists());
        assert!(dst.join("events.json").exists());
        assert!(dst.join("settings.json").exists());

        assert!(dst.join("store.json").exists());
        assert!(!dst.join("models").exists());
        assert!(!dst.join("search_index").exists());
    }

    #[test]
    fn copy_vault_items_handles_missing_items() {
        let temp = tempdir().unwrap();
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");

        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dst).unwrap();

        fs::write(src.join("events.json"), "events").unwrap();

        copy_vault_items(&src, &dst).unwrap();

        assert!(dst.join("events.json").exists());
        assert!(!dst.join("sessions").exists());
    }

    #[test]
    fn copy_vault_items_merges_sessions_and_prefers_the_source() {
        let temp = tempdir().unwrap();
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");

        fs::create_dir_all(src.join("sessions").join("shared")).unwrap();
        fs::write(src.join("sessions").join("shared").join("audio.wav"), "new").unwrap();
        fs::create_dir_all(src.join("sessions").join("only-src")).unwrap();
        fs::write(
            src.join("sessions").join("only-src").join("audio.wav"),
            "src",
        )
        .unwrap();
        fs::write(src.join("settings.json"), "src settings").unwrap();

        fs::create_dir_all(dst.join("sessions").join("shared")).unwrap();
        fs::write(dst.join("sessions").join("shared").join("audio.wav"), "old").unwrap();
        fs::write(dst.join("sessions").join("shared").join("note.md"), "keep").unwrap();
        fs::create_dir_all(dst.join("sessions").join("only-dst")).unwrap();
        fs::write(
            dst.join("sessions").join("only-dst").join("audio.wav"),
            "dst",
        )
        .unwrap();
        fs::write(dst.join("settings.json"), "dst settings").unwrap();

        copy_vault_items(&src, &dst).unwrap();

        let sessions = dst.join("sessions");
        assert_eq!(
            fs::read_to_string(sessions.join("shared").join("audio.wav")).unwrap(),
            "new"
        );
        assert_eq!(
            fs::read_to_string(sessions.join("shared").join("note.md")).unwrap(),
            "keep"
        );
        assert!(sessions.join("only-src").join("audio.wav").exists());
        assert!(sessions.join("only-dst").join("audio.wav").exists());
        assert_eq!(
            fs::read_to_string(dst.join("settings.json")).unwrap(),
            "src settings"
        );
    }

    #[test]
    fn remove_owned_items_leaves_user_content_and_legacy_names() {
        let temp = tempdir().unwrap();
        let vault = temp.path();

        fs::create_dir_all(vault.join("sessions").join("s1")).unwrap();
        fs::write(vault.join("sessions").join("s1").join("audio.wav"), "a").unwrap();
        fs::create_dir_all(vault.join("search_index")).unwrap();
        fs::write(vault.join("search_index").join("meta.json"), "{}").unwrap();
        fs::write(vault.join("settings.json"), "{}").unwrap();
        fs::write(vault.join("store.json"), "{}").unwrap();
        fs::write(
            vault.join("AGENTS.md"),
            "# Anarlog Desktop\n\nauto-generated",
        )
        .unwrap();

        fs::create_dir_all(vault.join("prompts")).unwrap();
        fs::write(vault.join("prompts").join("mine.md"), "prompt").unwrap();
        fs::create_dir_all(vault.join("humans")).unwrap();
        fs::write(vault.join("humans").join("person.md"), "human").unwrap();
        fs::create_dir_all(vault.join(".obsidian")).unwrap();
        fs::write(vault.join(".obsidian").join("app.json"), "{}").unwrap();
        fs::write(vault.join("Daily note.md"), "note").unwrap();
        fs::write(vault.join("templates.json"), "[]").unwrap();

        remove_owned_items(vault).unwrap();

        assert!(!vault.join("sessions").exists());
        assert!(!vault.join("search_index").exists());
        assert!(!vault.join("settings.json").exists());
        assert!(!vault.join("store.json").exists());
        assert!(!vault.join("AGENTS.md").exists());

        assert!(vault.join("prompts").join("mine.md").exists());
        assert!(vault.join("humans").join("person.md").exists());
        assert!(vault.join(".obsidian").join("app.json").exists());
        assert!(vault.join("Daily note.md").exists());
        assert!(vault.join("templates.json").exists());
    }

    #[test]
    fn remove_owned_items_keeps_a_user_authored_agents_file() {
        let temp = tempdir().unwrap();
        let vault = temp.path();
        fs::write(vault.join("AGENTS.md"), "# My vault\n\nHouse rules").unwrap();

        remove_owned_items(vault).unwrap();

        assert!(vault.join("AGENTS.md").exists());
    }

    #[test]
    fn remove_derived_items_only_touches_the_search_index() {
        let temp = tempdir().unwrap();
        let base = temp.path();
        fs::create_dir_all(base.join("search_index")).unwrap();
        fs::write(base.join("search_index").join("meta.json"), "{}").unwrap();
        fs::create_dir_all(base.join("sessions")).unwrap();

        remove_derived_items(base).unwrap();

        assert!(!base.join("search_index").exists());
        assert!(base.join("sessions").exists());
    }
}
