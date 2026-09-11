//! The Tauri webview's `localStorage`, where `react-resizable-panels` keeps
//! the sidebar layout: WebKit stores each origin in an `ItemTable` SQLite
//! file (`key TEXT, value BLOB` of UTF-16LE) under the app's data directory
//! (WebKitGTK) or `~/Library/WebKit/<identifier>/WebsiteData/LocalStorage`
//! (WKWebView). Best effort: a missing directory or a busy file is skipped.

use std::path::{Path, PathBuf};

use sqlx::ConnectOptions as _;

/// The `*.localstorage` files of every origin the webview has stored.
pub fn origin_files(db_path: &Path, identifier: &str) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(data_dir) = db_path.parent() {
        dirs.push(data_dir.join("localstorage"));
    }
    if let Some(home) = dirs::home_dir() {
        dirs.push(
            home.join("Library")
                .join("WebKit")
                .join(identifier)
                .join("WebsiteData")
                .join("LocalStorage"),
        );
    }
    let mut files = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "localstorage") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// The value stored under `key` in the first origin file that has it.
pub async fn read(files: &[PathBuf], key: &str) -> Option<String> {
    for file in files {
        let Ok(mut connection) = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(file)
            .read_only(true)
            .connect()
            .await
        else {
            continue;
        };
        let row: Result<Option<(Vec<u8>,)>, _> =
            sqlx::query_as("SELECT value FROM ItemTable WHERE key = ?")
                .bind(key)
                .fetch_optional(&mut connection)
                .await;
        if let Ok(Some((bytes,))) = row {
            return Some(decode_utf16le(&bytes));
        }
    }
    None
}

/// Writes `value` under `key` in every origin file that already holds the
/// key, so the webview's next launch reads it back. Returns how many files
/// took the write.
pub async fn write(files: &[PathBuf], key: &str, value: &str) -> usize {
    let bytes = encode_utf16le(value);
    let mut written = 0;
    for file in files {
        let Ok(mut connection) = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(file)
            .connect()
            .await
        else {
            continue;
        };
        let result = sqlx::query("UPDATE ItemTable SET value = ? WHERE key = ?")
            .bind(&bytes)
            .bind(key)
            .execute(&mut connection)
            .await;
        match result {
            Ok(done) if done.rows_affected() > 0 => written += 1,
            Ok(_) => {}
            Err(error) => {
                tracing::debug!(%error, path = %file.display(), "webkit localStorage write skipped");
            }
        }
    }
    written
}

fn decode_utf16le(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    String::from_utf16_lossy(&units)
}

fn encode_utf16le(value: &str) -> Vec<u8> {
    value
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16le_round_trips() {
        let text = r#"{"layout":[18.2481751825,81.7518248175]}"#;
        let bytes = encode_utf16le(text);
        assert_eq!(bytes.len(), text.len() * 2);
        assert_eq!(bytes[0], b'{');
        assert_eq!(bytes[1], 0);
        assert_eq!(decode_utf16le(&bytes), text);
    }

    #[tokio::test]
    async fn reads_and_writes_an_origin_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("http_localhost_1422.localstorage");
        {
            let mut connection = sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&file)
                .create_if_missing(true)
                .connect()
                .await
                .unwrap();
            sqlx::query(
                "CREATE TABLE ItemTable (key TEXT UNIQUE ON CONFLICT REPLACE, value BLOB NOT NULL ON CONFLICT FAIL)",
            )
            .execute(&mut connection)
            .await
            .unwrap();
            sqlx::query("INSERT INTO ItemTable (key, value) VALUES (?, ?)")
                .bind("k")
                .bind(encode_utf16le("old"))
                .execute(&mut connection)
                .await
                .unwrap();
        }
        let files = vec![file];
        assert_eq!(read(&files, "k").await.as_deref(), Some("old"));
        assert_eq!(read(&files, "missing").await, None);
        assert_eq!(write(&files, "k", "new").await, 1);
        assert_eq!(write(&files, "missing", "x").await, 0);
        assert_eq!(read(&files, "k").await.as_deref(), Some("new"));
    }
}
