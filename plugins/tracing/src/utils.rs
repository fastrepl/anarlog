use std::io::Write;
use std::path::{Path, PathBuf};
use std::{fs, io};

use tauri::Manager;
use tracing_appender::non_blocking::WorkerGuard;

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;
const MAX_LOG_FILES: usize = 5;

pub(crate) fn cleanup_legacy_logs<M: Manager<tauri::Wry>>(app: &M) {
    let Ok(data_dir) = app.path().data_dir() else {
        return;
    };

    let bundle_id: &str = app.config().identifier.as_ref();
    let app_folder = if cfg!(debug_assertions) || bundle_id == "com.hyprnote.staging" {
        bundle_id
    } else {
        "hyprnote"
    };

    let old_logs_dir = data_dir.join(app_folder);
    if !old_logs_dir.exists() {
        return;
    }

    for name in ["log", "log.1", "log.2", "log.3", "log.4", "log.5"] {
        let _ = fs::remove_file(old_logs_dir.join(name));
    }
}

pub fn cleanup_old_daily_logs(logs_dir: &PathBuf) -> io::Result<()> {
    if !logs_dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(logs_dir)? {
        let entry = entry?;
        let path = entry.path();

        if let Some(filename) = path.file_name().and_then(|n| n.to_str())
            && filename.starts_with("log.")
            && filename.len() > 4
        {
            let suffix = &filename[4..];
            if suffix.chars().all(|c| c.is_ascii_digit() || c == '-') {
                let _ = fs::remove_file(path);
            }
        }
    }

    Ok(())
}

struct SizeRollingWriter {
    base_path: PathBuf,
    file: Option<fs::File>,
    max_bytes: u64,
    max_files: usize,
    written: u64,
}

impl SizeRollingWriter {
    fn new(base_path: PathBuf, max_bytes: u64, max_files: usize) -> io::Result<Self> {
        if max_bytes == 0 || max_files == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "log rotation limits must be greater than zero",
            ));
        }

        if let Some(parent) = base_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut writer = Self {
            base_path,
            file: None,
            max_bytes,
            max_files,
            written: 0,
        };
        writer.open_current_file()?;
        Ok(writer)
    }

    fn suffixed_path(&self, suffix: usize) -> PathBuf {
        PathBuf::from(format!("{}.{}", self.base_path.display(), suffix))
    }

    fn remove_if_exists(path: &Path) -> io::Result<()> {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn move_if_exists(source: &Path, destination: &Path) -> io::Result<()> {
        Self::remove_if_exists(destination)?;
        match fs::rename(source, destination) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn open_current_file(&mut self) -> io::Result<()> {
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.base_path)?;
        self.written = file.metadata()?.len();
        self.file = Some(file);
        Ok(())
    }

    fn rotate(&mut self) -> io::Result<()> {
        let flush_result = self
            .file
            .take()
            .map(|mut file| file.flush())
            .unwrap_or(Ok(()));
        let rotation_result = flush_result.and_then(|_| {
            Self::remove_if_exists(&self.suffixed_path(self.max_files))?;
            for suffix in (1..self.max_files).rev() {
                Self::move_if_exists(&self.suffixed_path(suffix), &self.suffixed_path(suffix + 1))?;
            }
            Self::move_if_exists(&self.base_path, &self.suffixed_path(1))
        });
        let open_result = self.open_current_file();

        rotation_result.and(open_result)
    }

    fn current_file(&mut self) -> io::Result<&mut fs::File> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("log file is not open"))
    }
}

impl Write for SizeRollingWriter {
    fn write(&mut self, mut buffer: &[u8]) -> io::Result<usize> {
        let total = buffer.len();

        while !buffer.is_empty() {
            if self.written >= self.max_bytes {
                self.rotate()?;
            }

            let remaining = usize::try_from(self.max_bytes - self.written)
                .unwrap_or(usize::MAX)
                .min(buffer.len());
            let written = self.current_file()?.write(&buffer[..remaining])?;
            if written == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "failed to write application log",
                ));
            }

            self.written += written as u64;
            buffer = &buffer[written..];
        }

        Ok(total)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.current_file()?.flush()
    }
}

pub fn make_file_writer(
    logs_dir: &PathBuf,
) -> io::Result<(tracing_appender::non_blocking::NonBlocking, WorkerGuard)> {
    let _ = cleanup_old_daily_logs(logs_dir);

    let log_path = logs_dir.join("app.log");
    let file_appender = SizeRollingWriter::new(log_path, MAX_LOG_BYTES, MAX_LOG_FILES)?;

    let redacting_appender = crate::redaction::RedactingWriter::new(file_appender);

    let (non_blocking, guard) = tracing_appender::non_blocking(redacting_appender);
    Ok((non_blocking, guard))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn cleanup_old_daily_logs_removes_matching_files() {
        let temp = tempdir().unwrap();
        let logs_dir = temp.path().to_path_buf();

        fs::write(logs_dir.join("log.2024-01-15"), "old log").unwrap();
        fs::write(logs_dir.join("log.2024-01-16"), "old log").unwrap();
        fs::write(logs_dir.join("log.2024-12-31"), "old log").unwrap();

        cleanup_old_daily_logs(&logs_dir).unwrap();

        assert!(!logs_dir.join("log.2024-01-15").exists());
        assert!(!logs_dir.join("log.2024-01-16").exists());
        assert!(!logs_dir.join("log.2024-12-31").exists());
    }

    #[test]
    fn cleanup_old_daily_logs_preserves_non_matching() {
        let temp = tempdir().unwrap();
        let logs_dir = temp.path().to_path_buf();

        fs::write(logs_dir.join("app.log"), "current log").unwrap();
        fs::write(logs_dir.join("app.log.1"), "rotated log").unwrap();
        fs::write(logs_dir.join("other.txt"), "other file").unwrap();
        fs::write(logs_dir.join("log.2024-01-15"), "old log").unwrap();

        cleanup_old_daily_logs(&logs_dir).unwrap();

        assert!(logs_dir.join("app.log").exists());
        assert!(logs_dir.join("app.log.1").exists());
        assert!(logs_dir.join("other.txt").exists());
        assert!(!logs_dir.join("log.2024-01-15").exists());
    }

    #[test]
    fn cleanup_old_daily_logs_handles_empty_dir() {
        let temp = tempdir().unwrap();
        let logs_dir = temp.path().to_path_buf();

        let result = cleanup_old_daily_logs(&logs_dir);
        assert!(result.is_ok());
    }

    #[test]
    fn cleanup_old_daily_logs_handles_nonexistent_dir() {
        let logs_dir = PathBuf::from("/nonexistent/path/that/does/not/exist");

        let result = cleanup_old_daily_logs(&logs_dir);
        assert!(result.is_ok());
    }

    #[test]
    fn cleanup_old_daily_logs_preserves_log_without_date_suffix() {
        let temp = tempdir().unwrap();
        let logs_dir = temp.path().to_path_buf();

        fs::write(logs_dir.join("log.txt"), "log file").unwrap();
        fs::write(logs_dir.join("log.backup"), "backup").unwrap();

        cleanup_old_daily_logs(&logs_dir).unwrap();

        assert!(logs_dir.join("log.txt").exists());
        assert!(logs_dir.join("log.backup").exists());
    }

    #[test]
    fn size_rolling_writer_rotates_without_panicking_on_existing_suffixes() {
        let temp = tempdir().unwrap();
        let log_path = temp.path().join("app.log");
        fs::write(&log_path, "0123456789").unwrap();
        fs::write(temp.path().join("app.log.1"), "previous").unwrap();

        let mut writer = SizeRollingWriter::new(log_path, 10, 2).unwrap();
        writer.write_all(b"abcdefghijk").unwrap();
        writer.flush().unwrap();

        assert_eq!(fs::read(temp.path().join("app.log")).unwrap(), b"k");
        assert_eq!(
            fs::read(temp.path().join("app.log.1")).unwrap(),
            b"abcdefghij"
        );
        assert_eq!(
            fs::read(temp.path().join("app.log.2")).unwrap(),
            b"0123456789"
        );
    }

    #[test]
    fn size_rolling_writer_recovers_when_the_active_log_disappears() {
        let temp = tempdir().unwrap();
        let log_path = temp.path().join("app.log");
        fs::write(&log_path, "0123456789").unwrap();

        let mut writer = SizeRollingWriter::new(log_path.clone(), 10, 2).unwrap();
        fs::remove_file(&log_path).unwrap();
        writer.write_all(b"new log").unwrap();
        writer.flush().unwrap();

        assert_eq!(fs::read(log_path).unwrap(), b"new log");
    }
}
