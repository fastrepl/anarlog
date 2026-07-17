use std::ffi::{CStr, c_char, c_int, c_uint, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::OnceLock;

use libsqlite3_sys::{
    SQLITE_OK, SQLITE_TRACE_CLOSE, SQLITE_TRACE_PROFILE, SQLITE_TRACE_STMT, sqlite3,
    sqlite3_api_routines, sqlite3_auto_extension, sqlite3_db_handle, sqlite3_exec,
    sqlite3_get_autocommit, sqlite3_sql, sqlite3_stmt, sqlite3_trace_v2, sqlite3_wal_checkpoint,
    sqlite3_wal_hook,
};
use sqlx::sqlite::LockedSqliteHandle;

use crate::Error;

static REGISTRATION_RESULT: OnceLock<c_int> = OnceLock::new();
const TERMINATE_SQL: &[u8] = b"SELECT cloudsync_terminate()\0";
const DEFAULT_WAL_AUTOCHECKPOINT_PAGES: c_int = 1_000;

struct TransactionObserver {
    on_commit: Box<dyn FnMut() + Send>,
    on_rollback: Box<dyn FnMut() + Send>,
    active_statement: *mut sqlite3_stmt,
    active_rollback: bool,
    closing: bool,
}

pub(crate) fn install_terminate_on_close() -> Result<(), Error> {
    let result = *REGISTRATION_RESULT.get_or_init(|| {
        #[allow(unsafe_code)]
        unsafe {
            sqlite3_auto_extension(Some(register_close_trace))
        }
    });

    if result == SQLITE_OK {
        Ok(())
    } else {
        Err(Error::CloseHookRegistration(result))
    }
}

#[allow(unsafe_code)]
unsafe extern "C" fn register_close_trace(
    db: *mut sqlite3,
    _error: *mut *mut c_char,
    _api: *const sqlite3_api_routines,
) -> c_int {
    unsafe {
        sqlite3_trace_v2(
            db,
            SQLITE_TRACE_CLOSE,
            Some(trace_cloudsync_connection),
            ptr::null_mut(),
        )
    }
}

pub fn install_transaction_observer(
    handle: &mut LockedSqliteHandle<'_>,
    on_commit: impl FnMut() + Send + 'static,
    on_rollback: impl FnMut() + Send + 'static,
) -> Result<(), Error> {
    let observer = Box::new(TransactionObserver {
        on_commit: Box::new(on_commit),
        on_rollback: Box::new(on_rollback),
        active_statement: ptr::null_mut(),
        active_rollback: false,
        closing: false,
    });
    let observer = Box::into_raw(observer);

    #[allow(unsafe_code)]
    let result = unsafe {
        sqlite3_trace_v2(
            handle.as_raw_handle().as_ptr(),
            SQLITE_TRACE_CLOSE | SQLITE_TRACE_PROFILE | SQLITE_TRACE_STMT,
            Some(trace_cloudsync_connection),
            observer.cast(),
        )
    };
    if result == SQLITE_OK {
        #[allow(unsafe_code)]
        unsafe {
            sqlite3_wal_hook(
                handle.as_raw_handle().as_ptr(),
                Some(observe_wal_commit),
                observer.cast(),
            );
        }
        Ok(())
    } else {
        #[allow(unsafe_code)]
        unsafe {
            drop(Box::from_raw(observer));
        }
        Err(Error::TransactionObserverRegistration(result))
    }
}

#[allow(unsafe_code)]
unsafe extern "C" fn trace_cloudsync_connection(
    event: c_uint,
    context: *mut c_void,
    object: *mut c_void,
    _statement: *mut c_void,
) -> c_int {
    match event {
        SQLITE_TRACE_STMT if !context.is_null() && !object.is_null() => unsafe {
            let observer = &mut *context.cast::<TransactionObserver>();
            if !observer.closing && observer.active_statement.is_null() {
                let statement = object.cast::<sqlite3_stmt>();
                let database = sqlite3_db_handle(statement);
                if !database.is_null() && sqlite3_get_autocommit(database) != 0 {
                    invoke_observer(observer.on_rollback.as_mut());
                }
                observer.active_statement = statement;
                observer.active_rollback = statement_is_full_rollback(statement);
            }
        },
        SQLITE_TRACE_PROFILE if !context.is_null() && !object.is_null() => unsafe {
            let observer = &mut *context.cast::<TransactionObserver>();
            let statement = object.cast::<sqlite3_stmt>();
            if observer.active_statement == statement {
                let database = sqlite3_db_handle(statement);
                if observer.active_rollback
                    && !database.is_null()
                    && sqlite3_get_autocommit(database) != 0
                {
                    invoke_observer(observer.on_rollback.as_mut());
                }
                observer.active_statement = ptr::null_mut();
                observer.active_rollback = false;
            }
        },
        SQLITE_TRACE_CLOSE if !object.is_null() => unsafe {
            let database = object.cast::<sqlite3>();
            if context.is_null() {
                terminate_cloudsync(database);
            } else {
                let observer = &mut *context.cast::<TransactionObserver>();
                observer.closing = true;
                sqlite3_wal_hook(database, None, ptr::null_mut());
                let trace_unregistered =
                    sqlite3_trace_v2(database, 0, None, ptr::null_mut()) == SQLITE_OK;
                terminate_cloudsync(database);
                invoke_observer(observer.on_rollback.as_mut());

                if trace_unregistered {
                    drop(Box::from_raw(context.cast::<TransactionObserver>()));
                }
            }
        },
        _ => {}
    }

    SQLITE_OK
}

#[allow(unsafe_code)]
unsafe extern "C" fn observe_wal_commit(
    context: *mut c_void,
    database: *mut sqlite3,
    database_name: *const c_char,
    frame_count: c_int,
) -> c_int {
    if !context.is_null() {
        let observer = unsafe { &mut *context.cast::<TransactionObserver>() };
        if !observer.closing {
            invoke_observer(observer.on_commit.as_mut());
        }
    }

    // Registering a WAL hook replaces SQLite's default autocheckpoint callback.
    if !database.is_null() && frame_count >= DEFAULT_WAL_AUTOCHECKPOINT_PAGES {
        unsafe {
            sqlite3_wal_checkpoint(database, database_name);
        }
    }

    SQLITE_OK
}

fn invoke_observer(callback: &mut (dyn FnMut() + Send)) {
    let _ = catch_unwind(AssertUnwindSafe(|| callback()));
}

#[allow(unsafe_code)]
unsafe fn statement_is_full_rollback(statement: *mut sqlite3_stmt) -> bool {
    let sql = unsafe { sqlite3_sql(statement) };
    if sql.is_null() {
        return false;
    }

    let sql = unsafe { CStr::from_ptr(sql) }.to_str().unwrap_or_default();
    is_full_rollback(sql)
}

fn is_full_rollback(sql: &str) -> bool {
    let Some(tail) = strip_keyword(sql, "ROLLBACK") else {
        return false;
    };
    let tail = strip_keyword(tail, "TRANSACTION").unwrap_or(tail);
    strip_keyword(tail, "TO").is_none()
}

fn strip_keyword<'a>(sql: &'a str, keyword: &str) -> Option<&'a str> {
    let sql = sql.trim_start();
    let prefix = sql.get(..keyword.len())?;
    if !prefix.eq_ignore_ascii_case(keyword) {
        return None;
    }

    let tail = &sql[keyword.len()..];
    tail.chars()
        .next()
        .is_none_or(|next| next.is_ascii_whitespace() || next == ';')
        .then_some(tail)
}

#[allow(unsafe_code)]
unsafe fn terminate_cloudsync(connection: *mut sqlite3) {
    // SQLite invokes this trace before checking for the statements held by SQLite Sync.
    unsafe {
        sqlite3_exec(
            connection,
            TERMINATE_SQL.as_ptr().cast(),
            None,
            ptr::null_mut(),
            ptr::null_mut(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinguishes_full_and_savepoint_rollbacks() {
        assert!(is_full_rollback("ROLLBACK"));
        assert!(is_full_rollback(" rollback transaction;"));
        assert!(!is_full_rollback("ROLLBACK TO savepoint_name"));
        assert!(!is_full_rollback(
            "ROLLBACK TRANSACTION TO SAVEPOINT savepoint_name"
        ));
        assert!(!is_full_rollback("SELECT 1"));
    }
}
