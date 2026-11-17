use std::{
    ffi::OsString,
    fs::{File, OpenOptions, TryLockError},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anlg_db_core::{Db, DbOpenOptions, DbStorage};
use anlg_db_execute::DbExecutor;

use crate::{Profile, Result, ServiceError, types::failure};

pub(crate) struct ProfileLease {
    _files: Vec<File>,
}

fn suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = OsString::from(path.as_os_str());
    name.push(suffix);
    name.into()
}

fn lock(path: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(failure)?;
    file.try_lock().map_err(|error| match error {
        TryLockError::WouldBlock => ServiceError::Failed(
            format!(
                "Profile is already open in another process: {}",
                path.display()
            )
            .into(),
        ),
        TryLockError::Error(error) => failure(error),
    })?;
    Ok(file)
}

impl ProfileLease {
    pub(crate) fn acquire(profile: &Profile) -> Result<Self> {
        let parent = profile
            .database
            .parent()
            .filter(|p| !p.as_os_str().is_empty());
        let parent = parent.ok_or_else(|| failure("Database path must include a directory"))?;
        std::fs::create_dir_all(parent).map_err(failure)?;
        let canonical = std::fs::canonicalize(parent).map_err(failure)?.join(
            profile
                .database
                .file_name()
                .ok_or_else(|| failure("Invalid database path"))?,
        );
        if canonical.is_symlink() {
            return Err(failure(
                "Database symlinks are not supported for isolated profiles",
            ));
        }
        let mut files = vec![lock(&suffix(&canonical, ".gpui.lock"))?];
        for name in [
            "launch.lock",
            "com.hyprnote.stable.running.lock",
            "com.hyprnote.nightly.running.lock",
        ] {
            files.push(lock(&parent.join(name))?);
        }
        Ok(Self { _files: files })
    }
}

pub(crate) async fn open(profile: &Profile) -> Result<Arc<Db>> {
    let mut retries = 0;
    let db = loop {
        match Db::open(DbOpenOptions {
            storage: DbStorage::Local(&profile.database),
            cloudsync_enabled: false,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(crate::DATABASE_POOL_SIZE),
        })
        .await
        {
            Ok(db) => break Arc::new(db),
            Err(error) => {
                let text = error.to_string();
                if retries < 12
                    && (text.contains("database is locked")
                        || text.contains("database table is locked"))
                {
                    retries += 1;
                    tokio::time::sleep(Duration::from_secs(5)).await;
                } else {
                    return Err(failure(error));
                }
            }
        }
    };
    let executor = DbExecutor::new(db.clone());
    let initialized = executor.execute(
        "SELECT 1 AS initialized FROM sqlite_master WHERE (type = 'table' AND name = 'cloudsync_table_settings') OR (type = 'trigger' AND instr(replace(lower(COALESCE(sql, '')), 'cloudsync_workspace_binding', ''), 'cloudsync_') > 0) LIMIT 1".into(),
        vec![],
    ).await.map_err(failure);
    match initialized {
        Ok(rows) if rows.is_empty() => {}
        result => {
            db.pool().close().await;
            return Err(result.err().unwrap_or_else(|| ServiceError::Unsupported(
                "This profile contains an initialized CloudSync replica. A native sync bootstrap is required; the database has not been migrated.".into(),
            )));
        }
    }
    if let Err(error) = anlg_db_app::prepare_schema(&db).await {
        db.pool().close().await;
        return Err(failure(error));
    }
    Ok(db)
}

pub(crate) fn check_reset(profile: &Profile) -> Result<()> {
    if suffix(&profile.database, ".reset-requested").exists() {
        return Err(ServiceError::Unsupported(
            "A database reset is pending. Preserve this profile and finish the reset in the shipping application before copying it again.".into(),
        ));
    }
    Ok(())
}
