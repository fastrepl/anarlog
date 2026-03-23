use std::ffi::c_void;
use std::sync::Arc;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tokio::sync::broadcast;

use crate::{ManagedState, TableUpdate, update_hook_callback};

pub struct ReactiveDb<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> ReactiveDb<'a, R, M> {
    /// Initialize with custom connection options.
    /// Returns a clone of the pool so the caller can run migrations, etc.
    pub async fn init(&self, options: SqliteConnectOptions) -> Result<SqlitePool, crate::Error> {
        let (tx, _) = broadcast::channel::<TableUpdate>(256);
        let tx_for_hook = tx.clone();

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .after_connect(move |conn, _| {
                let tx = tx_for_hook.clone();
                Box::pin(async move {
                    let mut handle = conn.lock_handle().await?;
                    let raw = handle.as_raw_handle().as_ptr();

                    // Leak an Arc so the broadcast sender outlives the C callback.
                    let tx_ptr = Arc::into_raw(Arc::new(tx)) as *mut c_void;
                    unsafe {
                        libsqlite3_sys::sqlite3_update_hook(
                            raw,
                            Some(update_hook_callback),
                            tx_ptr,
                        );
                    }

                    Ok(())
                })
            })
            .connect_with(options)
            .await?;

        {
            let state = self.manager.state::<ManagedState>();
            let mut guard = state.lock().await;
            guard.pool = Some(pool.clone());
            guard.updates_tx = Some(tx);
        }

        Ok(pool)
    }

    /// Convenience: open a local file-backed database.
    pub async fn init_local(
        &self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<SqlitePool, crate::Error> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .pragma("journal_mode", "WAL")
            .pragma("foreign_keys", "ON");
        self.init(options).await
    }

    /// Convenience: open an in-memory database (useful for tests).
    pub async fn init_memory(&self) -> Result<SqlitePool, crate::Error> {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .pragma("foreign_keys", "ON");
        self.init(options).await
    }

    /// Access the underlying pool (e.g. for migrations).
    pub async fn pool(&self) -> Option<SqlitePool> {
        let state = self.manager.state::<ManagedState>();
        let guard = state.lock().await;
        guard.pool.clone()
    }
}

pub trait ReactiveDbExt<R: tauri::Runtime> {
    fn reactive_db(&self) -> ReactiveDb<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> ReactiveDbExt<R> for T {
    fn reactive_db(&self) -> ReactiveDb<'_, R, Self>
    where
        Self: Sized,
    {
        ReactiveDb {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
