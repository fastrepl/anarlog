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
    on_savepoint: Box<dyn FnMut(SavepointEvent) + Send>,
    active_statement: *mut sqlite3_stmt,
    active_control: Option<TransactionControl>,
    closing: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SavepointEvent {
    Begin {
        name: String,
        starts_transaction: bool,
    },
    RollbackTo(String),
    Release {
        name: String,
        transaction_active: bool,
    },
}

#[derive(Debug, PartialEq, Eq)]
enum TransactionControl {
    FullRollback,
    Savepoint {
        name: String,
        starts_transaction: bool,
    },
    RollbackTo(String),
    Release(String),
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
    on_savepoint: impl FnMut(SavepointEvent) + Send + 'static,
) -> Result<(), Error> {
    let observer = Box::new(TransactionObserver {
        on_commit: Box::new(on_commit),
        on_rollback: Box::new(on_rollback),
        on_savepoint: Box::new(on_savepoint),
        active_statement: ptr::null_mut(),
        active_control: None,
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
                let starts_transaction =
                    !database.is_null() && sqlite3_get_autocommit(database) != 0;
                if starts_transaction {
                    invoke_observer(observer.on_rollback.as_mut());
                }
                observer.active_statement = statement;
                observer.active_control = statement_transaction_control(statement);
                if let Some(TransactionControl::Savepoint {
                    starts_transaction: active_starts_transaction,
                    ..
                }) = observer.active_control.as_mut()
                {
                    *active_starts_transaction = starts_transaction;
                }
            }
        },
        SQLITE_TRACE_PROFILE if !context.is_null() && !object.is_null() => unsafe {
            let observer = &mut *context.cast::<TransactionObserver>();
            let statement = object.cast::<sqlite3_stmt>();
            if observer.active_statement == statement {
                let database = sqlite3_db_handle(statement);
                match observer.active_control.take() {
                    Some(TransactionControl::FullRollback)
                        if !database.is_null() && sqlite3_get_autocommit(database) != 0 =>
                    {
                        invoke_observer(observer.on_rollback.as_mut());
                    }
                    Some(TransactionControl::Savepoint {
                        name,
                        starts_transaction,
                    }) => {
                        invoke_savepoint_observer(
                            observer.on_savepoint.as_mut(),
                            SavepointEvent::Begin {
                                name,
                                starts_transaction,
                            },
                        );
                    }
                    Some(TransactionControl::RollbackTo(name)) => {
                        invoke_savepoint_observer(
                            observer.on_savepoint.as_mut(),
                            SavepointEvent::RollbackTo(name),
                        );
                    }
                    Some(TransactionControl::Release(name)) => {
                        let transaction_active =
                            database.is_null() || sqlite3_get_autocommit(database) == 0;
                        invoke_savepoint_observer(
                            observer.on_savepoint.as_mut(),
                            SavepointEvent::Release {
                                name,
                                transaction_active,
                            },
                        );
                    }
                    _ => {}
                }
                observer.active_statement = ptr::null_mut();
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

fn invoke_savepoint_observer(
    callback: &mut (dyn FnMut(SavepointEvent) + Send),
    event: SavepointEvent,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| callback(event)));
}

#[allow(unsafe_code)]
unsafe fn statement_transaction_control(
    statement: *mut sqlite3_stmt,
) -> Option<TransactionControl> {
    let sql = unsafe { sqlite3_sql(statement) };
    if sql.is_null() {
        return None;
    }

    let sql = unsafe { CStr::from_ptr(sql) }.to_str().unwrap_or_default();
    parse_transaction_control(sql)
}

fn parse_transaction_control(sql: &str) -> Option<TransactionControl> {
    if let Some(tail) = strip_keyword(sql, "SAVEPOINT") {
        return parse_identifier(tail).map(|name| TransactionControl::Savepoint {
            name,
            starts_transaction: false,
        });
    }

    if let Some(tail) = strip_keyword(sql, "RELEASE") {
        let tail = strip_keyword(tail, "SAVEPOINT").unwrap_or(tail);
        return parse_identifier(tail).map(TransactionControl::Release);
    }

    let tail = strip_keyword(sql, "ROLLBACK")?;
    let tail = strip_keyword(tail, "TRANSACTION").unwrap_or(tail);
    let Some(tail) = strip_keyword(tail, "TO") else {
        return Some(TransactionControl::FullRollback);
    };
    let tail = strip_keyword(tail, "SAVEPOINT").unwrap_or(tail);
    parse_identifier(tail).map(TransactionControl::RollbackTo)
}

fn strip_keyword<'a>(sql: &'a str, keyword: &str) -> Option<&'a str> {
    let sql = trim_sql_prefix(sql);
    let prefix = sql.get(..keyword.len())?;
    if !prefix.eq_ignore_ascii_case(keyword) {
        return None;
    }

    let tail = &sql[keyword.len()..];
    (tail
        .chars()
        .next()
        .is_none_or(|next| next.is_ascii_whitespace() || next == ';')
        || tail.starts_with("--")
        || tail.starts_with("/*"))
    .then_some(tail)
}

fn trim_sql_prefix(mut sql: &str) -> &str {
    loop {
        sql = sql.trim_start();
        if let Some(comment) = sql.strip_prefix("--") {
            sql = comment
                .split_once('\n')
                .map_or("", |(_, remainder)| remainder);
        } else if let Some(comment) = sql.strip_prefix("/*") {
            sql = comment
                .split_once("*/")
                .map_or("", |(_, remainder)| remainder);
        } else {
            return sql;
        }
    }
}

fn parse_identifier(sql: &str) -> Option<String> {
    let sql = trim_sql_prefix(sql);
    let delimiter = sql.chars().next()?;
    match delimiter {
        '"' | '\'' | '`' => parse_quoted_identifier(sql, delimiter),
        '[' => sql[1..]
            .find(']')
            .and_then(|end| sql[1..].get(..end))
            .map(str::to_string),
        _ => {
            let end = sql
                .char_indices()
                .find_map(|(index, character)| {
                    (character.is_ascii_whitespace()
                        || character == ';'
                        || sql[index..].starts_with("--")
                        || sql[index..].starts_with("/*"))
                    .then_some(index)
                })
                .unwrap_or(sql.len());
            (end > 0).then(|| sql[..end].to_string())
        }
    }
}

fn parse_quoted_identifier(sql: &str, delimiter: char) -> Option<String> {
    let mut identifier = String::new();
    let mut characters = sql[delimiter.len_utf8()..].chars().peekable();
    while let Some(character) = characters.next() {
        if character != delimiter {
            identifier.push(character);
            continue;
        }
        if characters.peek() == Some(&delimiter) {
            characters.next();
            identifier.push(delimiter);
            continue;
        }
        return Some(identifier);
    }
    None
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
    fn parses_transaction_controls() {
        assert_eq!(
            parse_transaction_control("SAVEPOINT _sqlx_savepoint_1"),
            Some(TransactionControl::Savepoint {
                name: "_sqlx_savepoint_1".to_string(),
                starts_transaction: false,
            })
        );
        assert_eq!(
            parse_transaction_control("ROLLBACK TO SAVEPOINT _sqlx_savepoint_1"),
            Some(TransactionControl::RollbackTo(
                "_sqlx_savepoint_1".to_string()
            ))
        );
        assert_eq!(
            parse_transaction_control("RELEASE SAVEPOINT _sqlx_savepoint_1"),
            Some(TransactionControl::Release("_sqlx_savepoint_1".to_string()))
        );
        assert_eq!(
            parse_transaction_control("SAVEPOINT \"quoted \"\" name\""),
            Some(TransactionControl::Savepoint {
                name: "quoted \" name".to_string(),
                starts_transaction: false,
            })
        );
        assert_eq!(
            parse_transaction_control("ROLLBACK TRANSACTION TO [nested step]"),
            Some(TransactionControl::RollbackTo("nested step".to_string()))
        );
        assert_eq!(
            parse_transaction_control("RELEASE `nested``step`"),
            Some(TransactionControl::Release("nested`step".to_string()))
        );
        assert_eq!(
            parse_transaction_control(" rollback transaction;"),
            Some(TransactionControl::FullRollback)
        );
        assert_eq!(
            parse_transaction_control(
                "/* outer */ ROLLBACK/* transaction */TO/* target */SAVEPOINT \"nested step\""
            ),
            Some(TransactionControl::RollbackTo("nested step".to_string()))
        );
        assert_eq!(parse_transaction_control("SELECT 1"), None);
    }
}
