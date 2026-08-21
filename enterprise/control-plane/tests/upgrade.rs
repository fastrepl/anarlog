use std::{fs, path::PathBuf};

#[test]
fn control_plane_migrations_are_additive() {
    let migrations = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let mut files: Vec<_> = fs::read_dir(&migrations)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("sql"))
        .collect();
    files.sort();
    assert!(!files.is_empty());

    for path in files {
        let sql = fs::read_to_string(&path).unwrap();
        let normalized = sql
            .lines()
            .filter(|line| !line.trim_start().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n")
            .to_uppercase();
        assert!(
            !normalized.contains("DROP TABLE"),
            "{} drops a table and would lose durable capture jobs",
            path.display()
        );
        assert!(
            !normalized.contains("DROP COLUMN"),
            "{} drops a column and would lose durable capture jobs",
            path.display()
        );
        assert!(
            !normalized.contains("RENAME COLUMN"),
            "{} renames a column and would break older capture jobs",
            path.display()
        );
    }
}
