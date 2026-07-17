use std::fs::{copy, remove_file, rename};
use std::io::ErrorKind;
use std::path::Path;

fn is_cross_device(err: &std::io::Error) -> bool {
    #[cfg(unix)]
    {
        err.raw_os_error() == Some(18)
    }
    #[cfg(not(unix))]
    {
        let _ = err;
        false
    }
}

fn rename_or_copy(from: &Path, to: &Path) -> Result<(), std::io::Error> {
    match rename(from, to) {
        Ok(()) => Ok(()),
        Err(err) if is_cross_device(&err) => {
            copy(from, to)?;
            remove_file(from)?;
            Ok(())
        }
        Err(err) => Err(err),
    }
}

pub(crate) fn atomic_move(from: &Path, to: &Path) -> Result<(), std::io::Error> {
    match rename_or_copy(from, to) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::AlreadyExists => {
            remove_file(to)?;
            rename_or_copy(from, to)
        }
        Err(err) => Err(err),
    }
}
