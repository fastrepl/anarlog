use std::ffi::{c_char, c_int, c_uint, c_void};
use std::ptr;
use std::sync::OnceLock;

use libsqlite3_sys::{
    SQLITE_OK, SQLITE_TRACE_CLOSE, sqlite3, sqlite3_api_routines, sqlite3_auto_extension,
    sqlite3_exec, sqlite3_trace_v2,
};

use crate::Error;

static REGISTRATION_RESULT: OnceLock<c_int> = OnceLock::new();
const TERMINATE_SQL: &[u8] = b"SELECT cloudsync_terminate()\0";

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
            Some(terminate_cloudsync),
            ptr::null_mut(),
        )
    }
}

#[allow(unsafe_code)]
unsafe extern "C" fn terminate_cloudsync(
    event: c_uint,
    _context: *mut c_void,
    connection: *mut c_void,
    _statement: *mut c_void,
) -> c_int {
    if event == SQLITE_TRACE_CLOSE && !connection.is_null() {
        // SQLite invokes this trace before checking for the statements held by SQLite Sync.
        unsafe {
            sqlite3_exec(
                connection.cast(),
                TERMINATE_SQL.as_ptr().cast(),
                None,
                ptr::null_mut(),
                ptr::null_mut(),
            );
        }
    }

    SQLITE_OK
}
