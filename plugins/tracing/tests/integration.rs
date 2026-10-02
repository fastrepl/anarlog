use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use tauri_plugin_tracing::make_file_writer;
use tempfile::tempdir;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, prelude::*};

fn create_test_file_writer(
    logs_dir: &PathBuf,
) -> (tracing_appender::non_blocking::NonBlocking, WorkerGuard) {
    make_file_writer(logs_dir).unwrap()
}

mod e2e {
    use super::*;

    #[test]
    fn test_tracing_with_redaction_e2e() {
        let temp = tempdir().unwrap();
        let logs_dir = temp.path().to_path_buf();
        fs::create_dir_all(&logs_dir).unwrap();

        let home_dir = dirs::home_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "/home/testuser".to_string());

        let (file_writer, guard) = create_test_file_writer(&logs_dir);

        let subscriber = tracing_subscriber::registry()
            .with(fmt::layer().with_ansi(false).with_writer(file_writer));

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("User logged in from {}/documents", home_dir);
            tracing::warn!("Email notification sent to user@example.com");
            tracing::error!("Connection from 192.168.1.100 failed");
        });

        drop(guard);
        std::thread::sleep(Duration::from_millis(100));

        let log_file = logs_dir.join("app.log");
        assert!(log_file.exists(), "Log file should be created");

        let content = fs::read_to_string(&log_file).unwrap();

        assert!(
            content.contains("[HOME]/documents"),
            "Home path should be redacted. Content: {}",
            content
        );
        assert!(
            content.contains("[EMAIL_REDACTED]"),
            "Email should be redacted. Content: {}",
            content
        );
        assert!(
            content.contains("[IP_REDACTED]"),
            "IP should be redacted. Content: {}",
            content
        );

        assert!(
            !content.contains(&home_dir),
            "Original home path should not appear"
        );
        assert!(
            !content.contains("user@example.com"),
            "Original email should not appear"
        );
        assert!(
            !content.contains("192.168.1.100"),
            "Original IP should not appear"
        );

        assert!(content.contains("User logged in"));
        assert!(content.contains("Email notification"));
        assert!(content.contains("Connection from"));
    }

    #[test]
    fn test_log_file_rotation_e2e() {
        let temp = tempdir().unwrap();
        let logs_dir = temp.path().to_path_buf();
        fs::create_dir_all(&logs_dir).unwrap();

        let (file_writer, guard) = create_test_file_writer(&logs_dir);

        let subscriber = tracing_subscriber::registry()
            .with(fmt::layer().with_ansi(false).with_writer(file_writer));

        let large_message = "x".repeat(1024);
        tracing::subscriber::with_default(subscriber, || {
            for i in 0..6000 {
                tracing::info!("Log entry {} - {}", i, large_message);
            }
        });

        drop(guard);
        std::thread::sleep(Duration::from_millis(100));

        let app_log = logs_dir.join("app.log");
        assert!(app_log.exists(), "app.log should exist");

        let rotated_exists = (1..=5).any(|i| logs_dir.join(format!("app.log.{}", i)).exists());
        assert!(
            rotated_exists,
            "At least one rotated log file should exist after writing 6MB+ of data"
        );
    }

    #[test]
    fn test_structured_logging_with_redaction_e2e() {
        let temp = tempdir().unwrap();
        let logs_dir = temp.path().to_path_buf();
        fs::create_dir_all(&logs_dir).unwrap();

        let home = dirs::home_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "/home/testuser".to_string());

        let (file_writer, guard) = create_test_file_writer(&logs_dir);

        let subscriber = tracing_subscriber::registry()
            .with(fmt::layer().with_ansi(false).with_writer(file_writer));

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(
                path = format!("{}/config", home),
                email = "admin@example.org",
                ip = "10.0.0.1",
                "Structured log with sensitive fields"
            );
        });

        drop(guard);
        std::thread::sleep(Duration::from_millis(100));

        let content = fs::read_to_string(logs_dir.join("app.log")).unwrap();

        assert!(
            content.contains("[HOME]/config") || content.contains("[HOME]"),
            "Home path in structured field should be redacted"
        );
        assert!(
            content.contains("[EMAIL_REDACTED]"),
            "Email in structured field should be redacted"
        );
        assert!(
            content.contains("[IP_REDACTED]"),
            "IP in structured field should be redacted"
        );
    }
}
