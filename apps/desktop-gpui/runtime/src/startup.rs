use std::{
    ffi::OsString,
    fs::{File, OpenOptions, TryLockError},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use anlg_db_core::{Db, DbStorage};
use anlg_desktop_identity::{
    channel::{NIGHTLY_IDENTIFIER, STABLE_IDENTIFIER},
    paths::{LAUNCH_LOCK_FILENAME, channel_lock_filename},
};

use crate::{Profile, Result, ServiceError, Startup, StartupStatus, types::failure};

const LOCK_POLL: Duration = Duration::from_millis(500);
const DB_OPEN_LOCK_RETRIES: u32 = 12;
const DB_OPEN_LOCK_RETRY_DELAY: Duration = Duration::from_secs(5);

pub(crate) struct ProfileLease {
    _files: Vec<File>,
}

fn suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = OsString::from(path.as_os_str());
    name.push(suffix);
    name.into()
}

/// The lock file, or `None` while another process holds it.
fn lock(path: &Path) -> Result<Option<File>> {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(failure)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(error)) => Err(failure(error)),
    }
}

impl ProfileLease {
    /// Takes this app's lock and the shipping app's launch and channel locks. While another
    /// process holds one, retries until `startup.lock_wait` passes: an update relaunch starts
    /// before the replaced app has exited.
    pub(crate) async fn acquire(
        profile: &Profile,
        startup: &Startup,
        waiting: impl Fn(),
    ) -> Result<Self> {
        let deadline = Instant::now() + startup.lock_wait;
        loop {
            let held = match Self::try_acquire(profile, startup.channel.as_deref())? {
                Ok(lease) => return Ok(lease),
                Err(held) => held,
            };
            if Instant::now() >= deadline {
                return Err(ServiceError::Failed(
                    format!(
                        "Profile is already open in another process: {}",
                        held.display()
                    )
                    .into(),
                ));
            }
            waiting();
            tokio::time::sleep(LOCK_POLL).await;
        }
    }

    fn try_acquire(
        profile: &Profile,
        channel: Option<&str>,
    ) -> Result<std::result::Result<Self, PathBuf>> {
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
        let mut paths = vec![
            suffix(&canonical, ".gpui.lock"),
            parent.join(LAUNCH_LOCK_FILENAME),
            parent.join(channel_lock_filename(STABLE_IDENTIFIER)),
            parent.join(channel_lock_filename(NIGHTLY_IDENTIFIER)),
        ];
        // Every channel's shipping app holds its own while it runs, not only the two that share
        // a database, so this app never writes the channel's files alongside it.
        if let Some(channel) = channel {
            let own = parent.join(channel_lock_filename(channel));
            if !paths.contains(&own) {
                paths.push(own);
            }
        }
        let mut files = Vec::new();
        for path in paths {
            match lock(&path)? {
                Some(file) => files.push(file),
                None => return Ok(Err(path)),
            }
        }
        Ok(Ok(Self { _files: files }))
    }
}

/// Opens like the shipping app: the bundled sqlite-sync extension loaded, so a database that
/// ran sqlite-sync opens and migrates, retrying while the previous process still holds it.
pub(crate) async fn open(
    profile: &Profile,
    mut progress: impl FnMut(StartupStatus) + Send,
) -> Result<Arc<Db>> {
    let mut retries = 0;
    let db = loop {
        match anlg_db_core::open_app_db(
            DbStorage::Local(&profile.database),
            crate::DATABASE_POOL_SIZE,
        )
        .await
        {
            Ok(db) => break Arc::new(db),
            Err(error) => {
                let text = error.to_string();
                if retries < DB_OPEN_LOCK_RETRIES
                    && (text.contains("database is locked")
                        || text.contains("database table is locked"))
                {
                    retries += 1;
                    tokio::time::sleep(DB_OPEN_LOCK_RETRY_DELAY).await;
                } else {
                    return Err(failure(error));
                }
            }
        }
    };
    let prepared = anlg_db_app::prepare_schema_with_progress(&db, |migration| {
        progress(if migration.completed < migration.total {
            StartupStatus::Migrating {
                current: migration.completed + 1,
                total: migration.total,
            }
        } else {
            StartupStatus::Opening
        })
    })
    .await;
    if let Err(error) = prepared {
        db.pool().close().await;
        return Err(failure(error));
    }
    Ok(db)
}

/// Shipping's next start moves notes from a folder older versions kept them in into the channel's
/// data folder, under the launch lock this app now holds. This app moves no files, so until
/// shipping has, it refuses instead of opening half of the library.
pub(crate) fn unfinished_move(
    startup: &Startup,
) -> Option<anlg_desktop_identity::vault::UnfinishedMove> {
    startup.channel.as_ref()?;
    anlg_desktop_identity::vault::unfinished_move(startup.vault.as_deref()?)
}

/// A missing marker is retried at the next launch, so failing to write it doesn't stop this one.
pub(crate) fn record_takeover(startup: &Startup) {
    let (Some(vault), Some(takeover)) = (&startup.vault, &startup.takeover) else {
        return;
    };
    match anlg_desktop_identity::takeover::record(vault, takeover) {
        Ok(true) => {
            tracing::info!(version = %takeover.version, "recorded the takeover of this channel's data")
        }
        Ok(false) => {}
        Err(error) => {
            tracing::warn!(%error, "could not record the takeover of this channel's data")
        }
    }
}

/// Shipping's "Start fresh" leaves a marker for the next launch. Its data moves the database
/// aside the way shipping does; an isolated profile refuses to open instead.
pub(crate) fn check_reset(profile: &Profile, startup: &Startup) -> Result<()> {
    if startup.apply_reset {
        let directory = profile
            .database
            .parent()
            .ok_or_else(|| failure("Database path must include a directory"))?;
        if let Some(backup) = anlg_desktop_identity::paths::apply_pending_database_reset(directory)
            .map_err(failure)?
        {
            tracing::info!(backup = %backup.display(), "moved the database aside for a fresh start");
        }
        return Ok(());
    }
    if suffix(&profile.database, ".reset-requested").exists() {
        return Err(ServiceError::Unsupported(
            "A database reset is pending. Preserve this profile and finish the reset in the shipping application before copying it again.".into(),
        ));
    }
    Ok(())
}
