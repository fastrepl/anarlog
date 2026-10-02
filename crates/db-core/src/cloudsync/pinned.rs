use std::sync::Arc;

use anlg_cloudsync::{OwnedSqliteConnection, ReservedConnection};
use sqlx::pool::PoolConnection;
use sqlx::{Sqlite, SqliteConnection, SqlitePool};
use tokio::sync::{Mutex, OwnedMutexGuard};

pub(crate) struct PinnedCloudsyncConnection {
    guard: OwnedMutexGuard<Option<PoolConnection<Sqlite>>>,
    release_on_drop: bool,
}

impl PinnedCloudsyncConnection {
    pub(crate) fn new(
        guard: OwnedMutexGuard<Option<PoolConnection<Sqlite>>>,
        pool: &SqlitePool,
    ) -> Self {
        Self {
            guard,
            release_on_drop: pool.options().get_max_connections() == 1,
        }
    }
}

impl Drop for PinnedCloudsyncConnection {
    fn drop(&mut self) {
        if self.release_on_drop {
            drop(self.guard.take());
        }
    }
}

impl OwnedSqliteConnection for PinnedCloudsyncConnection {
    fn sqlite_connection(&mut self) -> &mut SqliteConnection {
        self.guard.as_mut().expect("pinned CloudSync connection")
    }
}

pub(crate) async fn reserve_pinned_connection(
    pool: &SqlitePool,
    slot: &Arc<Mutex<Option<PoolConnection<Sqlite>>>>,
) -> Result<ReservedConnection<PinnedCloudsyncConnection>, anlg_cloudsync::Error> {
    let mut pinned = Arc::clone(slot).lock_owned().await;
    if pinned.is_none() {
        *pinned = Some(pool.acquire().await?);
    }
    Ok(ReservedConnection::new(PinnedCloudsyncConnection::new(
        pinned, pool,
    )))
}

pub(crate) fn release_pinned_connection(connection: ReservedConnection<PinnedCloudsyncConnection>) {
    drop(connection);
}
