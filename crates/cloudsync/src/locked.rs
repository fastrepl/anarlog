#![allow(unsafe_code)]

use std::ffi::{CStr, CString, c_char};
use std::mem::ManuallyDrop;
use std::ptr::{self, NonNull};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use libsqlite3_sys::{
    SQLITE_DONE, SQLITE_INTERRUPT, SQLITE_NULL, SQLITE_OK, SQLITE_ROW, SQLITE_TRANSIENT, sqlite3,
    sqlite3_bind_int64, sqlite3_bind_text, sqlite3_column_bytes, sqlite3_column_text,
    sqlite3_column_type, sqlite3_errmsg, sqlite3_extended_errcode, sqlite3_finalize,
    sqlite3_interrupt, sqlite3_prepare_v2, sqlite3_step, sqlite3_stmt,
};
use sqlx::pool::PoolConnection;
use sqlx::{Sqlite, SqliteConnection};
use tokio::sync::oneshot;
use tokio::task::{JoinError, JoinHandle};

use crate::error::Error;

type OwnerResult = Result<Result<Option<String>, Error>, JoinError>;

pub trait OwnedSqliteConnection: Send + 'static {
    fn sqlite_connection(&mut self) -> &mut SqliteConnection;
}

impl OwnedSqliteConnection for PoolConnection<Sqlite> {
    fn sqlite_connection(&mut self) -> &mut SqliteConnection {
        self
    }
}

/// Its in-flight `JoinHandle` retains the connection until nested SQLite
/// registrations around the native call have been torn down.
pub struct ReservedConnection<C> {
    state: ReservedState<C>,
}

enum ReservedState<C> {
    Idle(C),
    InFlight(JoinHandle<(C, OwnerResult)>),
    Lost,
}

impl<C: OwnedSqliteConnection> ReservedConnection<C> {
    pub fn new(connection: C) -> Self {
        Self {
            state: ReservedState::Idle(connection),
        }
    }

    /// Waits for any native call a cancelled caller left running, then lends
    /// the connection.
    pub async fn connection(&mut self) -> Result<&mut SqliteConnection, Error> {
        self.settle().await?;
        match &mut self.state {
            ReservedState::Idle(connection) => Ok(connection.sqlite_connection()),
            ReservedState::InFlight(_) | ReservedState::Lost => unreachable!(),
        }
    }

    /// Returns the connection only when idle. An in-flight owner keeps and
    /// later drops the connection itself.
    pub fn into_inner(self) -> Option<C> {
        match self.state {
            ReservedState::Idle(connection) => Some(connection),
            ReservedState::InFlight(_) | ReservedState::Lost => None,
        }
    }

    async fn settle(&mut self) -> Result<(), Error> {
        let joined = match &mut self.state {
            ReservedState::Idle(_) => return Ok(()),
            ReservedState::InFlight(owner) => (&mut *owner).await,
            ReservedState::Lost => return Err(lost_connection_error()),
        };

        match joined {
            Ok((connection, _)) => {
                self.state = ReservedState::Idle(connection);
                Ok(())
            }
            Err(error) => {
                self.state = ReservedState::Lost;
                Err(owner_join_error(error))
            }
        }
    }
}

pub(crate) enum RawArg {
    Text(String),
    Int(i64),
}

#[derive(Clone, Copy)]
struct SendDb(NonNull<sqlite3>);

unsafe impl Send for SendDb {}

struct WorkerCancelOnDrop {
    db: SendDb,
    cancelled: Arc<AtomicBool>,
    worker_finished: Arc<AtomicBool>,
}

impl Drop for WorkerCancelOnDrop {
    fn drop(&mut self) {
        if !self.worker_finished.load(Ordering::SeqCst) {
            self.cancelled.store(true, Ordering::SeqCst);
            unsafe { sqlite3_interrupt(self.db.0.as_ptr()) };
        }
    }
}

struct WorkerFinishedOnDrop(Arc<AtomicBool>);

impl Drop for WorkerFinishedOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

pub(crate) async fn execute_on_locked_handle<C: OwnedSqliteConnection>(
    connection: &mut ReservedConnection<C>,
    sql: &'static str,
    args: Vec<RawArg>,
) -> Result<Option<String>, Error> {
    connection.settle().await?;
    let state = std::mem::replace(&mut connection.state, ReservedState::Lost);
    let ReservedState::Idle(connection_owner) = state else {
        unreachable!()
    };
    let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
    let owner = tokio::spawn(run_owned(connection_owner, sql, args, cancel_rx));
    connection.state = ReservedState::InFlight(owner);
    let _cancel_on_drop = cancel_tx;

    let joined = match &mut connection.state {
        ReservedState::InFlight(owner) => (&mut *owner).await,
        ReservedState::Idle(_) | ReservedState::Lost => unreachable!(),
    };
    match joined {
        Ok((connection_owner, worker_result)) => {
            connection.state = ReservedState::Idle(connection_owner);
            match worker_result {
                Ok(result) => result,
                Err(error) if error.is_panic() => std::panic::resume_unwind(error.into_panic()),
                Err(error) => Err(Error::Io(std::io::Error::other(error))),
            }
        }
        Err(error) => {
            connection.state = ReservedState::Lost;
            Err(owner_join_error(error))
        }
    }
}

/// Leaks the connection if the owner task is dropped at runtime shutdown,
/// keeping SQLite's raw handle valid for any worker that still uses it.
async fn run_owned<C: OwnedSqliteConnection>(
    connection: C,
    sql: &'static str,
    args: Vec<RawArg>,
    cancel_rx: oneshot::Receiver<()>,
) -> (C, OwnerResult) {
    let mut connection = ManuallyDrop::new(connection);
    let result =
        execute_on_locked_handle_inner(connection.sqlite_connection(), sql, args, cancel_rx).await;
    (ManuallyDrop::into_inner(connection), result)
}

async fn execute_on_locked_handle_inner(
    connection: &mut SqliteConnection,
    sql: &'static str,
    args: Vec<RawArg>,
    mut cancel_rx: oneshot::Receiver<()>,
) -> Result<Result<Option<String>, Error>, JoinError> {
    let mut handle = match connection.lock_handle().await {
        Ok(handle) => handle,
        Err(error) => return Ok(Err(error.into())),
    };
    if !matches!(
        cancel_rx.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ) {
        drop(handle);
        return Ok(Err(interrupted_error()));
    }
    let db = SendDb(handle.as_raw_handle());
    let worker_db = SendDb(handle.as_raw_handle());
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_finished = Arc::new(AtomicBool::new(false));
    let worker_cancelled = Arc::clone(&cancelled);
    let worker_finished_on_exit = Arc::clone(&worker_finished);

    let mut worker = tokio::task::spawn_blocking(move || {
        let _worker_finished = WorkerFinishedOnDrop(worker_finished_on_exit);
        unsafe { step_to_completion(worker_db, sql, &args, &worker_cancelled) }
    });
    let _cancel_on_drop = WorkerCancelOnDrop {
        db,
        cancelled: Arc::clone(&cancelled),
        worker_finished,
    };

    let joined = tokio::select! {
        biased;
        joined = &mut worker => joined,
        _ = &mut cancel_rx => {
            cancelled.store(true, Ordering::SeqCst);
            loop {
                unsafe { sqlite3_interrupt(db.0.as_ptr()) };
                tokio::select! {
                    biased;
                    joined = &mut worker => break joined,
                    _ = tokio::time::sleep(Duration::from_millis(10)) => {}
                }
            }
        }
    };
    drop(handle);
    joined
}

fn owner_join_error(error: JoinError) -> Error {
    if error.is_panic() {
        std::panic::resume_unwind(error.into_panic())
    } else {
        Error::Io(std::io::Error::other(error))
    }
}

fn lost_connection_error() -> Error {
    Error::Io(std::io::Error::other(
        "CloudSync connection was lost with an aborted native call",
    ))
}

unsafe fn step_to_completion(
    db: SendDb,
    sql: &'static str,
    args: &[RawArg],
    cancelled: &AtomicBool,
) -> Result<Option<String>, Error> {
    if cancelled.load(Ordering::SeqCst) {
        return Err(interrupted_error());
    }

    let sql = CString::new(sql)
        .map_err(|error| Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, error)))?;
    let mut statement: *mut sqlite3_stmt = ptr::null_mut();
    let prepare_result = unsafe {
        sqlite3_prepare_v2(
            db.0.as_ptr(),
            sql.as_ptr(),
            -1,
            &mut statement,
            ptr::null_mut(),
        )
    };

    let result = if prepare_result != SQLITE_OK {
        Err(sqlite_error(&db))
    } else if statement.is_null() {
        Err(Error::Io(std::io::Error::other(
            "SQLite prepared an empty statement",
        )))
    } else {
        (|| -> Result<Option<String>, Error> {
            for (index, arg) in args.iter().enumerate() {
                let index = i32::try_from(index + 1).map_err(|error| {
                    Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, error))
                })?;

                let bind_result = match arg {
                    RawArg::Text(value) => {
                        let length = i32::try_from(value.len()).map_err(|error| {
                            Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, error))
                        })?;
                        unsafe {
                            sqlite3_bind_text(
                                statement,
                                index,
                                value.as_ptr().cast::<c_char>(),
                                length,
                                SQLITE_TRANSIENT(),
                            )
                        }
                    }
                    RawArg::Int(value) => unsafe { sqlite3_bind_int64(statement, index, *value) },
                };

                if bind_result != SQLITE_OK {
                    return Err(sqlite_error(&db));
                }
            }

            if cancelled.load(Ordering::SeqCst) {
                return Err(interrupted_error());
            }

            let mut first_column = None;
            let mut saw_first_row = false;
            loop {
                match unsafe { sqlite3_step(statement) } {
                    SQLITE_ROW => {
                        if !saw_first_row {
                            saw_first_row = true;
                            first_column = unsafe { first_column_text(statement, &db) }?;
                        }
                    }
                    SQLITE_DONE => return Ok(first_column),
                    _ => return Err(sqlite_error(&db)),
                }
            }
        })()
    };

    if !statement.is_null() {
        let _ = unsafe { sqlite3_finalize(statement) };
    }

    result
}

fn interrupted_error() -> Error {
    Error::Sqlite {
        code: SQLITE_INTERRUPT,
        message: "interrupted".into(),
    }
}

unsafe fn first_column_text(
    statement: *mut sqlite3_stmt,
    db: &SendDb,
) -> Result<Option<String>, Error> {
    if unsafe { sqlite3_column_type(statement, 0) } == SQLITE_NULL {
        return Ok(None);
    }

    let text = unsafe { sqlite3_column_text(statement, 0) };
    if text.is_null() {
        return Err(sqlite_error(db));
    }
    let length = unsafe { sqlite3_column_bytes(statement, 0) } as usize;
    let bytes = unsafe { std::slice::from_raw_parts(text.cast::<u8>(), length) }.to_vec();

    String::from_utf8(bytes).map(Some).map_err(|error| {
        Error::Sqlx(sqlx::Error::ColumnDecode {
            index: "0".to_string(),
            source: error.into(),
        })
    })
}

fn sqlite_error(db: &SendDb) -> Error {
    unsafe {
        Error::Sqlite {
            code: sqlite3_extended_errcode(db.0.as_ptr()),
            message: CStr::from_ptr(sqlite3_errmsg(db.0.as_ptr()))
                .to_string_lossy()
                .into_owned(),
        }
    }
}
