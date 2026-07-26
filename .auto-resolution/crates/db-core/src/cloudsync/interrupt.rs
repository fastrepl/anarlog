use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicPtr, Ordering};

use sqlx::SqliteConnection;

#[derive(Default)]
pub(crate) struct CloudsyncInterruptHandle {
    state: Mutex<CloudsyncInterruptState>,
}

#[derive(Default)]
struct CloudsyncInterruptState {
    connection: AtomicPtr<libsqlite3_sys::sqlite3>,
    generation: u64,
}

pub(crate) struct CloudsyncInterruptRegistration<'a> {
    handle: &'a CloudsyncInterruptHandle,
    generation: u64,
}

impl CloudsyncInterruptHandle {
    pub(crate) async fn register<'a>(
        &'a self,
        connection: &mut SqliteConnection,
    ) -> Result<CloudsyncInterruptRegistration<'a>, sqlx::Error> {
        let mut handle = connection.lock_handle().await?;
        let connection = handle.as_raw_handle().as_ptr();
        let mut state = self.state.lock().unwrap();
        if !state.connection.load(Ordering::Acquire).is_null() {
            return Err(sqlx::Error::Protocol(
                "another CloudSync network operation is already interruptible".to_string(),
            ));
        }
        state.generation = state.generation.wrapping_add(1);
        state.connection.store(connection, Ordering::Release);
        Ok(CloudsyncInterruptRegistration {
            handle: self,
            generation: state.generation,
        })
    }

    pub(crate) fn interrupt(&self) -> bool {
        let state = self.state.lock().unwrap();
        let connection = state.connection.load(Ordering::Acquire);
        if connection.is_null() {
            return false;
        }

        // SAFETY: callers retain the pinned SQLx connection until registration
        // cleanup, and this mutex serializes interruption with that cleanup.
        // Calls made before the statement starts are no-ops, so callers retry.
        // Avoid redundant interrupts once SQLite reports one is in effect.
        unsafe {
            if libsqlite3_sys::sqlite3_is_interrupted(connection) == 0 {
                libsqlite3_sys::sqlite3_interrupt(connection);
            }
        }
        true
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub(crate) fn is_registered(&self) -> bool {
        let state = self.state.lock().unwrap();
        !state.connection.load(Ordering::Acquire).is_null()
    }
}

impl CloudsyncInterruptRegistration<'_> {
    pub(crate) async fn finish(
        mut self,
        connection: &mut SqliteConnection,
    ) -> Result<(), sqlx::Error> {
        self.unregister();
        let worker_idle = connection.lock_handle().await?;
        drop(worker_idle);
        Ok(())
    }

    fn unregister(&mut self) {
        let state = &mut *self.handle.state.lock().unwrap();
        if state.generation == self.generation {
            state.connection.store(ptr::null_mut(), Ordering::Release);
        }
    }
}

impl Drop for CloudsyncInterruptRegistration<'_> {
    fn drop(&mut self) {
        self.unregister();
    }
}
