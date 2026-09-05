use std::path::{Path, PathBuf};
use std::sync::Arc;

use anlg_db_app::{ListSessions, SessionListItem};
use anlg_db_core::Db;
use anyhow::Context as _;

const DB_FILENAME: &str = "app.db";
const SESSION_PAGE_SIZE: u32 = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotePreview {
    pub session_id: String,
    pub title: String,
    pub started_at: String,
    pub body: String,
}

/// Read-only access to the SQLite database owned by the Tauri desktop app.
///
/// The GPUI shell coexists with the Tauri app during the migration, so it never
/// writes to `app.db` and never runs migrations; the Tauri app stays the sole
/// schema owner until the write path moves over.
pub struct Store {
    runtime: tokio::runtime::Handle,
    db: Arc<Db>,
    path: PathBuf,
}

impl Store {
    pub async fn open(runtime: tokio::runtime::Handle, path: PathBuf) -> anyhow::Result<Self> {
        if !path.is_file() {
            anyhow::bail!(
                "no Anarlog database at {}. Launch the desktop app once to create it, or pass --db-path.",
                path.display()
            );
        }
        let db = Db::connect_local_read_only(&path)
            .await
            .with_context(|| format!("failed to open {}", path.display()))?;
        Ok(Self {
            runtime,
            db: Arc::new(db),
            path,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Runs the sqlx future on the tokio runtime and hands back a handle the
    /// GPUI foreground executor can await.
    pub fn list_sessions(&self) -> tokio::task::JoinHandle<anyhow::Result<Vec<SessionListItem>>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let sessions = anlg_db_app::list_sessions(
                db.pool(),
                ListSessions {
                    query: None,
                    series_id: None,
                    limit: SESSION_PAGE_SIZE,
                    offset: 0,
                },
            )
            .await?;
            Ok(sessions)
        })
    }

    pub fn load_note(
        &self,
        session_id: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<Option<NotePreview>>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let Some(session) = anlg_db_app::get_session(db.pool(), &session_id).await? else {
                return Ok(None);
            };
            let body = match anlg_db_app::get_session_note(db.pool(), &session_id).await? {
                Some(note) => note_body_as_markdown(&note.body_format, &note.body),
                None => String::new(),
            };
            Ok(Some(NotePreview {
                session_id: session.id,
                title: session.title,
                started_at: session.started_at,
                body,
            }))
        })
    }
}

fn note_body_as_markdown(body_format: &str, body: &str) -> String {
    if body.trim().is_empty() {
        return String::new();
    }
    match body_format {
        "prosemirror_json" => serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|json| anlg_tiptap::tiptap_json_to_md(&json).ok())
            .unwrap_or_else(|| body.to_string()),
        _ => body.to_string(),
    }
}

/// Resolves the same `app.db` the Tauri desktop app opens for `identifier`.
///
/// Mirrors `apps/desktop/src-tauri/src/db.rs`: prefer the raw identifier
/// folder when it already holds a database and the storage default does not.
pub fn default_db_path(identifier: &str) -> anyhow::Result<PathBuf> {
    let data_dir = dirs::data_dir().context("application data directory is unavailable")?;
    let default_dir = anlg_storage::global::compute_default_base(identifier)
        .context("application data directory is unavailable")?;
    Ok(resolve_db_dir(&data_dir, &default_dir, identifier).join(DB_FILENAME))
}

fn resolve_db_dir(data_dir: &Path, default_dir: &Path, identifier: &str) -> PathBuf {
    let identifier_dir = data_dir.join(identifier);
    if identifier_dir.join(DB_FILENAME).is_file() && !default_dir.join(DB_FILENAME).is_file() {
        identifier_dir
    } else {
        default_dir.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_db_dir_prefers_identifier_folder_only_when_default_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path();
        let default_dir = data_dir.join("anarlog");
        let identifier_dir = data_dir.join("com.hyprnote.stable");

        assert_eq!(
            resolve_db_dir(data_dir, &default_dir, "com.hyprnote.stable"),
            default_dir
        );

        std::fs::create_dir_all(&identifier_dir).unwrap();
        std::fs::write(identifier_dir.join(DB_FILENAME), "").unwrap();
        assert_eq!(
            resolve_db_dir(data_dir, &default_dir, "com.hyprnote.stable"),
            identifier_dir
        );

        std::fs::create_dir_all(&default_dir).unwrap();
        std::fs::write(default_dir.join(DB_FILENAME), "").unwrap();
        assert_eq!(
            resolve_db_dir(data_dir, &default_dir, "com.hyprnote.stable"),
            default_dir
        );
    }

    #[test]
    fn prosemirror_bodies_render_as_markdown() {
        let body = serde_json::json!({
            "type": "doc",
            "content": [
                { "type": "heading", "attrs": { "level": 2 }, "content": [{ "type": "text", "text": "Agenda" }] },
                { "type": "paragraph", "content": [{ "type": "text", "text": "Ship the GPUI shell." }] }
            ]
        })
        .to_string();

        let markdown = note_body_as_markdown("prosemirror_json", &body);
        assert!(markdown.contains("## Agenda"));
        assert!(markdown.contains("Ship the GPUI shell."));
        assert_eq!(note_body_as_markdown("markdown", "# raw"), "# raw");
        assert_eq!(note_body_as_markdown("prosemirror_json", "   "), "");
    }

    #[tokio::test]
    async fn store_reads_sessions_written_by_the_app_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILENAME);
        {
            let db = Db::connect_local_plain(&path).await.unwrap();
            anlg_db_app::prepare_schema(&db).await.unwrap();
            sqlx::raw_sql(
                "INSERT INTO sessions (id, workspace_id, title, started_at)
                 VALUES ('session-1', 'workspace-1', 'Weekly sync', '2026-09-01T09:00:00Z');
                 INSERT INTO session_documents (id, workspace_id, session_id, kind, body)
                 VALUES (
                   'session-1', 'workspace-1', 'session-1', 'note',
                   '{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\",\"content\":[{\"type\":\"text\",\"text\":\"Decide on GPUI.\"}]}]}'
                 );",
            )
            .execute(db.pool())
            .await
            .unwrap();
        }

        let store = Store::open(tokio::runtime::Handle::current(), path)
            .await
            .unwrap();
        let sessions = store.list_sessions().await.unwrap().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].title, "Weekly sync");

        let note = store
            .load_note("session-1".to_string())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(note.title, "Weekly sync");
        assert_eq!(note.started_at, "2026-09-01T09:00:00Z");
        assert_eq!(note.body.trim(), "Decide on GPUI.");

        assert!(
            store
                .load_note("missing".to_string())
                .await
                .unwrap()
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn store_refuses_to_create_a_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILENAME);
        let error = Store::open(tokio::runtime::Handle::current(), path.clone())
            .await
            .err()
            .unwrap();
        assert!(error.to_string().contains("--db-path"));
        assert!(!path.exists());
    }
}
