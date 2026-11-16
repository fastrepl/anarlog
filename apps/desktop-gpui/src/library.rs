use std::{path::Path, time::Duration};

use anyhow::{Context, Result, bail};
use sqlx::{Row, SqlitePool, sqlite::SqliteConnectOptions};

pub const PAGE_SIZE: usize = 200;
const MAX_DOCUMENT_BYTES: i64 = 2 * 1024 * 1024;

#[derive(Clone)]
pub struct Library(SqlitePool);

pub struct Session {
    pub id: String,
    pub title: String,
    pub date: String,
}

impl Library {
    pub async fn open(path: &Path) -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .read_only(true)
            .create_if_missing(false)
            .pragma("query_only", "ON")
            .busy_timeout(Duration::from_secs(2));
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .context("Could not open the selected library read-only")?;
        Ok(Self(pool))
    }

    /// One page of sessions whose title contains `filter`; an empty filter matches all.
    pub async fn sessions(&self, offset: usize, filter: &str) -> Result<(Vec<Session>, bool)> {
        // The filter is literal text, so LIKE's own wildcards are escaped.
        let pattern = format!(
            "%{}%",
            filter
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let rows = sqlx::query(
            "SELECT id, title, COALESCE(NULLIF(started_at, ''), created_at) AS date
             FROM sessions WHERE deleted_at IS NULL AND title LIKE ? ESCAPE '\\'
             ORDER BY date DESC, created_at DESC, id DESC LIMIT ? OFFSET ?",
        )
        .bind(pattern)
        .bind((PAGE_SIZE + 1) as i64)
        .bind(i64::try_from(offset)?)
        .fetch_all(&self.0)
        .await
        .context("Could not read sessions; this preview requires the current SQLite schema")?;
        let has_more = rows.len() > PAGE_SIZE;
        let sessions = rows
            .into_iter()
            .take(PAGE_SIZE)
            .map(|row| {
                Ok(Session {
                    id: row.try_get("id")?,
                    title: row.try_get("title")?,
                    date: row.try_get("date")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((sessions, has_more))
    }

    pub async fn note(&self, id: &str) -> Result<String> {
        // Match the shipping desktop's primary-note identity, not an arbitrary
        // document with the same session_id (which can be a generated summary).
        let row = sqlx::query(
            "SELECT note.body_format,
                    length(CAST(note.body AS BLOB)) AS bytes,
                    CASE WHEN length(CAST(note.body AS BLOB)) <= ? THEN note.body END AS body
             FROM sessions
             LEFT JOIN session_documents AS note
               ON note.id = sessions.id AND note.kind = 'note' AND note.deleted_at IS NULL
             WHERE sessions.id = ? AND sessions.deleted_at IS NULL",
        )
        .bind(MAX_DOCUMENT_BYTES)
        .bind(id)
        .fetch_optional(&self.0)
        .await?;
        let Some(row) = row else {
            bail!("This session is no longer available. Reload the library.");
        };
        if row.try_get::<Option<i64>, _>("bytes")?.unwrap_or(0) > MAX_DOCUMENT_BYTES {
            bail!(
                "This note exceeds the preview's 2 MiB limit. Open it in the current desktop app."
            );
        }
        let body = row
            .try_get::<Option<String>, _>("body")?
            .unwrap_or_default();
        if body.is_empty() {
            return Ok(String::new());
        }
        match row.try_get::<Option<String>, _>("body_format")?.as_deref() {
            Some("markdown") => Ok(body),
            Some("prosemirror_json") => {
                let json = serde_json::from_str(&body).context("The note contains invalid JSON")?;
                if let Some(error) = anlg_tiptap::validate_tiptap_json(&json).first() {
                    bail!("The note contains an invalid document: {error}");
                }
                anlg_tiptap::tiptap_json_to_md(&json).map_err(anyhow::Error::msg)
            }
            format => bail!("Unsupported note format: {format:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_equal_timestamp_sessions_without_duplicates() -> Result<()> {
        tokio::runtime::Runtime::new()?.block_on(async {
            let directory = tempfile::tempdir()?;
            let path = directory.path().join("app.db");
            let writer = SqlitePool::connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true),
            )
            .await?;
            sqlx::raw_sql(include_str!(
                "../../../crates/db-app/migrations/20260710223922_canonical_data_model.sql"
            ))
            .execute(&writer)
            .await?;
            for index in 0..=PAGE_SIZE {
                sqlx::query("INSERT INTO sessions (id, created_at) VALUES (?, '2026-10-29')")
                    .bind(format!("note-{index:04}"))
                    .execute(&writer)
                    .await?;
            }
            writer.close().await;
            let library = Library::open(&path).await?;
            let (first, has_more) = library.sessions(0, "").await?;
            assert_eq!(first.len(), PAGE_SIZE);
            assert!(has_more);
            let (last, has_more) = library.sessions(PAGE_SIZE, "").await?;
            assert_eq!(last.len(), 1);
            assert!(!has_more);
            assert_eq!(last[0].id, "note-0000");
            assert!(!first.iter().any(|session| session.id == last[0].id));
            Ok(())
        })
    }

    #[test]
    fn filters_titles_as_literal_text() -> Result<()> {
        tokio::runtime::Runtime::new()?.block_on(async {
            let directory = tempfile::tempdir()?;
            let path = directory.path().join("app.db");
            let writer = SqlitePool::connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true),
            )
            .await?;
            sqlx::raw_sql(include_str!(
                "../../../crates/db-app/migrations/20260710223922_canonical_data_model.sql"
            ))
            .execute(&writer)
            .await?;
            for (id, title) in [
                ("percent", "100% done"),
                ("plain", "100 done"),
                ("underscore", "a_b"),
                ("letter", "axb"),
                ("korean", "주간 회의록"),
                ("untitled", ""),
            ] {
                sqlx::query("INSERT INTO sessions (id, title) VALUES (?, ?)")
                    .bind(id)
                    .bind(title)
                    .execute(&writer)
                    .await?;
            }
            writer.close().await;
            let library = Library::open(&path).await?;
            let matching = async |filter: &str| -> Result<Vec<String>> {
                let (sessions, _) = library.sessions(0, filter).await?;
                let mut ids: Vec<String> = sessions.into_iter().map(|session| session.id).collect();
                ids.sort();
                Ok(ids)
            };
            assert_eq!(matching("").await?.len(), 6);
            assert_eq!(matching("100%").await?, ["percent"]);
            assert_eq!(matching("a_b").await?, ["underscore"]);
            assert_eq!(matching("회의").await?, ["korean"]);
            assert_eq!(matching("DONE").await?, ["percent", "plain"]);
            assert!(matching("\\").await?.is_empty());
            Ok(())
        })
    }

    #[test]
    fn reads_canonical_notes_without_modifying_the_library() -> Result<()> {
        tokio::runtime::Runtime::new()?.block_on(async {
            let directory = tempfile::tempdir()?;
            let path = directory.path().join("app.db");
            let writer = SqlitePool::connect_with(
                SqliteConnectOptions::new().filename(&path).create_if_missing(true),
            )
            .await?;
            sqlx::raw_sql(include_str!(
                "../../../crates/db-app/migrations/20260710223922_canonical_data_model.sql"
            ))
            .execute(&writer)
            .await?;
            sqlx::raw_sql(
                "INSERT INTO sessions (id, title) VALUES ('active', 'Meeting'), ('deleted', 'Gone');
                 UPDATE sessions SET deleted_at = '2026-10-29' WHERE id = 'deleted';
                 INSERT INTO session_documents (id, session_id, body) VALUES
                   ('active', 'active', '{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\",\"content\":[{\"type\":\"text\",\"text\":\"Hello 안녕 🦀\"}]}]}');
                 INSERT INTO session_documents (id, session_id, kind, body_format, body) VALUES
                   ('summary', 'active', 'summary', 'markdown', 'Do not display this');",
            )
            .execute(&writer)
            .await?;
            writer.close().await;
            let before = std::fs::read(&path)?;
            let library = Library::open(&path).await?;
            let (sessions, has_more) = library.sessions(0, "").await?;
            assert_eq!(sessions.len(), 1);
            assert_eq!(sessions[0].id, "active");
            assert!(!has_more);
            assert_eq!(library.note("active").await?.trim(), "Hello 안녕 🦀");
            assert!(library.note("deleted").await.is_err());
            assert!(sqlx::query("DELETE FROM sessions").execute(&library.0).await.is_err());
            library.0.close().await;
            assert_eq!(before, std::fs::read(&path)?);
            let missing = directory.path().join("missing.db");
            assert!(Library::open(&missing).await.is_err());
            assert!(!missing.exists());
            Ok(())
        })
    }
}
