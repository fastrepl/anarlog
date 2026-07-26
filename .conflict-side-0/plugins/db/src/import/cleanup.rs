use std::fmt::Write;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

const CLEANUP_COMPLETE_PHASE: &str = "legacy_files_removed";

#[derive(sqlx::FromRow)]
struct CleanupState {
    phase: String,
    importer_version: i64,
    latest_run_id: String,
    parity_verified: bool,
}

#[derive(sqlx::FromRow)]
struct CleanupRun {
    source_root: String,
    dry_run: bool,
    status: String,
}

#[derive(sqlx::FromRow)]
struct CleanupSource {
    source_path: String,
    source_sha256: String,
}

struct VerifiedFile {
    path: PathBuf,
    expected_sha256: String,
    size_bytes: u64,
}

struct CleanupPlan {
    status: crate::LegacyCleanupStatus,
    files: Vec<VerifiedFile>,
    latest_run_id: String,
    source_root: PathBuf,
}

pub async fn get_status(pool: &SqlitePool) -> crate::Result<crate::LegacyCleanupStatus> {
    Ok(build_plan(pool).await?.status)
}

pub async fn execute(pool: &SqlitePool) -> crate::Result<crate::LegacyCleanupResult> {
    let plan = build_plan(pool).await?;

    if plan.status.already_cleaned {
        return Ok(crate::LegacyCleanupResult {
            deleted_file_count: 0,
            deleted_bytes: 0,
        });
    }

    if let Some(reason) = &plan.status.blocking_reason {
        return Err(std::io::Error::other(reason.clone()).into());
    }

    let mut deleted_file_count = 0;
    let mut deleted_bytes = 0;
    let mut parent_directories = Vec::new();

    for file in &plan.files {
        let bytes = std::fs::read(&file.path)?;
        if sha256(&bytes) != file.expected_sha256 {
            return Err(std::io::Error::other(format!(
                "legacy file changed after cleanup verification: {}",
                file.path.display()
            ))
            .into());
        }
    }

    for file in &plan.files {
        std::fs::remove_file(&file.path)?;
        deleted_file_count += 1;
        deleted_bytes += file.size_bytes;
        if let Some(parent) = file.path.parent() {
            parent_directories.push(parent.to_path_buf());
        }
    }

    remove_empty_directories(&plan.source_root, parent_directories);

    let updated = sqlx::query(
        "UPDATE storage_migration_state
         SET phase = ?,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = 'legacy_v1'
           AND latest_run_id = ?
           AND importer_version = ?
           AND parity_verified = 1",
    )
    .bind(CLEANUP_COMPLETE_PHASE)
    .bind(&plan.latest_run_id)
    .bind(hypr_db_app::LEGACY_IMPORTER_VERSION)
    .execute(pool)
    .await?;

    if updated.rows_affected() != 1 {
        return Err(std::io::Error::other(
            "migration verification changed before cleanup could be recorded",
        )
        .into());
    }

    Ok(crate::LegacyCleanupResult {
        deleted_file_count,
        deleted_bytes,
    })
}

async fn build_plan(pool: &SqlitePool) -> crate::Result<CleanupPlan> {
    let state = sqlx::query_as::<_, CleanupState>(
        "SELECT phase, importer_version, latest_run_id, parity_verified
         FROM storage_migration_state
         WHERE id = 'legacy_v1'",
    )
    .fetch_one(pool)
    .await?;

    if state.phase == CLEANUP_COMPLETE_PHASE {
        return Ok(empty_plan(
            crate::LegacyCleanupStatus {
                migration_verified: true,
                available: false,
                already_cleaned: true,
                file_count: 0,
                total_bytes: 0,
                source_root: String::new(),
                blocking_reason: None,
            },
            state.latest_run_id,
        ));
    }

    if !state.parity_verified || state.importer_version != hypr_db_app::LEGACY_IMPORTER_VERSION {
        return Ok(blocked_plan(
            "SQLite migration has not passed current parity verification",
            state.latest_run_id,
        ));
    }

    let run = sqlx::query_as::<_, CleanupRun>(
        "SELECT source_root, dry_run, status
         FROM migration_import_runs
         WHERE id = ? AND importer_version = ?",
    )
    .bind(&state.latest_run_id)
    .bind(hypr_db_app::LEGACY_IMPORTER_VERSION)
    .fetch_optional(pool)
    .await?;

    let Some(run) = run else {
        return Ok(blocked_plan(
            "Verified migration run is unavailable",
            state.latest_run_id,
        ));
    };

    if run.dry_run || run.status != "completed" {
        return Ok(blocked_plan(
            "Verified migration run is not complete",
            state.latest_run_id,
        ));
    }

    let source_root = PathBuf::from(&run.source_root);
    if !source_root.is_absolute() {
        return Ok(blocked_plan(
            "Legacy migration source is not an absolute path",
            state.latest_run_id,
        ));
    }

    let sources = sqlx::query_as::<_, CleanupSource>(
        "SELECT source_path, source_sha256
         FROM migration_import_items
         WHERE run_id = ? AND status IN ('complete', 'unchanged')
         ORDER BY source_path",
    )
    .bind(&state.latest_run_id)
    .fetch_all(pool)
    .await?;

    let mut files = Vec::new();
    let mut changed_file_count = 0;

    for source in sources {
        if !is_legacy_text_source(&source.source_path) {
            continue;
        }

        let path = match safe_source_path(&source_root, &source.source_path) {
            Ok(path) => path,
            Err(_) => {
                return Ok(blocked_plan_with_root(
                    "Migration report contains an unsafe source path",
                    state.latest_run_id,
                    run.source_root,
                ));
            }
        };

        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Ok(blocked_plan_with_root(
                    format!("A legacy file could not be verified: {error}"),
                    state.latest_run_id,
                    run.source_root,
                ));
            }
        };

        if sha256(&bytes) != source.source_sha256 {
            changed_file_count += 1;
            continue;
        }

        files.push(VerifiedFile {
            path,
            expected_sha256: source.source_sha256,
            size_bytes: bytes.len() as u64,
        });
    }

    if changed_file_count > 0 {
        return Ok(blocked_plan_with_root(
            format!(
                "{changed_file_count} legacy file{} changed after migration and will not be removed",
                if changed_file_count == 1 { "" } else { "s" }
            ),
            state.latest_run_id,
            run.source_root,
        ));
    }

    let total_bytes = files.iter().map(|file| file.size_bytes).sum();
    let status = crate::LegacyCleanupStatus {
        migration_verified: true,
        available: !files.is_empty(),
        already_cleaned: false,
        file_count: files.len() as u64,
        total_bytes,
        source_root: run.source_root,
        blocking_reason: None,
    };

    Ok(CleanupPlan {
        status,
        files,
        latest_run_id: state.latest_run_id,
        source_root,
    })
}

fn empty_plan(status: crate::LegacyCleanupStatus, latest_run_id: String) -> CleanupPlan {
    CleanupPlan {
        status,
        files: Vec::new(),
        latest_run_id,
        source_root: PathBuf::new(),
    }
}

fn blocked_plan(reason: impl Into<String>, latest_run_id: String) -> CleanupPlan {
    blocked_plan_with_root(reason, latest_run_id, String::new())
}

fn blocked_plan_with_root(
    reason: impl Into<String>,
    latest_run_id: String,
    source_root: String,
) -> CleanupPlan {
    empty_plan(
        crate::LegacyCleanupStatus {
            migration_verified: false,
            available: false,
            already_cleaned: false,
            file_count: 0,
            total_bytes: 0,
            source_root,
            blocking_reason: Some(reason.into()),
        },
        latest_run_id,
    )
}

fn safe_source_path(source_root: &Path, relative_path: &str) -> std::io::Result<PathBuf> {
    let relative = Path::new(relative_path);
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(std::io::Error::other("unsafe legacy source path"));
    }

    Ok(source_root.join(relative))
}

fn is_legacy_text_source(relative_path: &str) -> bool {
    matches!(
        Path::new(relative_path)
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("json" | "md")
    )
}

fn remove_empty_directories(source_root: &Path, directories: Vec<PathBuf>) {
    let mut directories = directories
        .into_iter()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));

    for directory in directories {
        let mut current = directory.as_path();
        while current != source_root && current.starts_with(source_root) {
            if std::fs::remove_dir(current).is_err() {
                break;
            }
            let Some(parent) = current.parent() else {
                break;
            };
            current = parent;
        }
    }
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing to String cannot fail");
            output
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn seed_verified_import(pool: &SqlitePool, source_root: &Path, sources: &[(&str, &str)]) {
        sqlx::query(
            "INSERT INTO migration_import_runs
             (id, importer_version, source_root, dry_run, status, completed_at)
             VALUES ('verified-run', ?, ?, 0, 'completed', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
        )
        .bind(hypr_db_app::LEGACY_IMPORTER_VERSION)
        .bind(source_root.to_string_lossy().as_ref())
        .execute(pool)
        .await
        .unwrap();

        for (index, (source_path, source_sha256)) in sources.iter().enumerate() {
            sqlx::query(
                "INSERT INTO migration_import_items
                 (id, run_id, source_path, source_kind, source_sha256, status, completed_at)
                 VALUES (?, 'verified-run', ?, 'test', ?, 'complete', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            )
            .bind(format!("item-{index}"))
            .bind(source_path)
            .bind(source_sha256)
            .execute(pool)
            .await
            .unwrap();
        }

        sqlx::query(
            "UPDATE storage_migration_state
             SET importer_version = ?, latest_run_id = 'verified-run', parity_verified = 1
             WHERE id = 'legacy_v1'",
        )
        .bind(hypr_db_app::LEGACY_IMPORTER_VERSION)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn removes_only_verified_json_and_markdown_sources() {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        let vault = tempfile::tempdir().unwrap();
        let session = vault.path().join("sessions/session-1");
        std::fs::create_dir_all(session.join("attachments")).unwrap();

        let settings = br#"{"theme":"dark"}"#;
        let note = b"# Meeting notes";
        let audio = b"audio";
        std::fs::write(vault.path().join("settings.json"), settings).unwrap();
        std::fs::write(session.join("note.md"), note).unwrap();
        std::fs::write(session.join("attachments/audio.mp3"), audio).unwrap();

        seed_verified_import(
            db.pool(),
            vault.path(),
            &[
                ("settings.json", &sha256(settings)),
                ("sessions/session-1/note.md", &sha256(note)),
                ("sessions/session-1/attachments/audio.mp3", &sha256(audio)),
            ],
        )
        .await;

        let status = get_status(db.pool()).await.unwrap();
        assert!(status.available);
        assert_eq!(status.file_count, 2);
        assert_eq!(status.total_bytes, (settings.len() + note.len()) as u64);

        let result = execute(db.pool()).await.unwrap();
        assert_eq!(result.deleted_file_count, 2);
        assert!(!vault.path().join("settings.json").exists());
        assert!(!session.join("note.md").exists());
        assert!(session.join("attachments/audio.mp3").exists());

        let phase: String =
            sqlx::query_scalar("SELECT phase FROM storage_migration_state WHERE id = 'legacy_v1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(phase, CLEANUP_COMPLETE_PHASE);
        assert!(get_status(db.pool()).await.unwrap().already_cleaned);
    }

    #[tokio::test]
    async fn changed_source_blocks_all_cleanup() {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        let vault = tempfile::tempdir().unwrap();
        let original = b"original";
        let unchanged = b"unchanged";
        std::fs::write(vault.path().join("settings.json"), b"changed").unwrap();
        std::fs::write(vault.path().join("tasks.json"), unchanged).unwrap();

        seed_verified_import(
            db.pool(),
            vault.path(),
            &[
                ("settings.json", &sha256(original)),
                ("tasks.json", &sha256(unchanged)),
            ],
        )
        .await;

        let status = get_status(db.pool()).await.unwrap();
        assert!(!status.available);
        assert!(
            status
                .blocking_reason
                .unwrap()
                .contains("changed after migration")
        );
        assert!(execute(db.pool()).await.is_err());
        assert!(vault.path().join("settings.json").exists());
        assert!(vault.path().join("tasks.json").exists());
    }

    #[test]
    fn rejects_paths_outside_the_source_root() {
        assert!(safe_source_path(Path::new("/vault"), "../settings.json").is_err());
        assert!(safe_source_path(Path::new("/vault"), "/settings.json").is_err());
        assert_eq!(
            safe_source_path(Path::new("/vault"), "sessions/one/note.md").unwrap(),
            Path::new("/vault/sessions/one/note.md")
        );
    }
}
