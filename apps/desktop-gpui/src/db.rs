use std::path::{Path, PathBuf};
use std::sync::Arc;

use anlg_db_core::Db;
use anyhow::Context as _;

use crate::timeline::SessionRow;

const DB_FILENAME: &str = "app.db";

// Same rows the Tauri sidebar reads (apps/desktop/src/calendar/queries.ts,
// `useTimelineSessionsTable`), minus the tags aggregate it only shows behind a
// setting. Ordering is applied afterwards by `timeline::build`, as in the app.
const TIMELINE_SESSIONS_SQL: &str = "
    SELECT id, title, created_at, event_json, folder_path AS folder_id, locked
    FROM sessions
    WHERE deleted_at IS NULL
    ORDER BY created_at, id
";

// apps/desktop/src/session/queries/enhanced-notes.ts, `useEnhancedNoteRecords`.
const ENHANCED_NOTES_SQL: &str = "
    SELECT id, title, body, body_format
    FROM session_documents
    WHERE session_id = ?
      AND kind IN ('summary', 'template_output')
      AND deleted_at IS NULL
    ORDER BY sort_order, id
";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteDocument {
    pub id: String,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotePreview {
    pub session: SessionRow,
    /// The raw memo (`kind = 'note'`), converted to Markdown.
    pub memo: String,
    /// Summaries and template outputs in the order the app tabs them.
    pub enhanced: Vec<NoteDocument>,
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
    pub fn list_sessions(&self) -> tokio::task::JoinHandle<anyhow::Result<Vec<SessionRow>>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let rows = sqlx::query_as::<_, SessionRow>(TIMELINE_SESSIONS_SQL)
                .fetch_all(db.pool())
                .await?;
            Ok(rows)
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
            let memo = match anlg_db_app::get_session_note(db.pool(), &session_id).await? {
                Some(note) => note_body_as_markdown(&note.body_format, &note.body),
                None => String::new(),
            };
            let enhanced =
                sqlx::query_as::<_, (String, String, String, String)>(ENHANCED_NOTES_SQL)
                    .bind(&session_id)
                    .fetch_all(db.pool())
                    .await?
                    .into_iter()
                    .map(|(id, title, body, body_format)| NoteDocument {
                        id,
                        title,
                        body: note_body_as_markdown(&body_format, &body),
                    })
                    .collect();
            Ok(Some(NotePreview {
                session: SessionRow {
                    id: session.id,
                    title: session.title,
                    created_at: session.created_at,
                    event_json: session.event_json,
                    folder_id: session.folder_path,
                    locked: session.locked,
                },
                memo,
                enhanced,
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
                "INSERT INTO sessions (id, workspace_id, title, created_at, event_json)
                 VALUES ('session-1', 'workspace-1', 'Weekly sync', '2026-09-01T09:00:00Z',
                         '{\"started_at\":\"2026-09-02T10:00:00Z\"}');
                 INSERT INTO sessions (id, workspace_id, title, created_at, deleted_at)
                 VALUES ('deleted', 'workspace-1', 'Gone', '2026-09-03T09:00:00Z', '2026-09-04T00:00:00Z');
                 INSERT INTO session_documents (id, workspace_id, session_id, kind, body)
                 VALUES (
                   'session-1', 'workspace-1', 'session-1', 'note',
                   '{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\",\"content\":[{\"type\":\"text\",\"text\":\"Decide on GPUI.\"}]}]}'
                 );
                 INSERT INTO session_documents (id, workspace_id, session_id, kind, title, body_format, body, sort_order)
                 VALUES ('summary-2', 'workspace-1', 'session-1', 'summary', 'Second', 'markdown', '## Later', 2),
                        ('summary-1', 'workspace-1', 'session-1', 'template_output', 'First', 'markdown', '## Sooner', 1);",
            )
            .execute(db.pool())
            .await
            .unwrap();
        }

        let store = Store::open(tokio::runtime::Handle::current(), path)
            .await
            .unwrap();
        let sessions = store.list_sessions().await.unwrap().unwrap();
        assert_eq!(sessions.len(), 1, "deleted sessions stay hidden");
        assert_eq!(sessions[0].title, "Weekly sync");
        assert_eq!(
            sessions[0].event_json,
            "{\"started_at\":\"2026-09-02T10:00:00Z\"}"
        );

        let note = store
            .load_note("session-1".to_string())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(note.session.title, "Weekly sync");
        assert_eq!(note.session.created_at, "2026-09-01T09:00:00Z");
        assert_eq!(note.memo.trim(), "Decide on GPUI.");
        assert_eq!(
            note.enhanced
                .iter()
                .map(|doc| (doc.id.as_str(), doc.title.as_str(), doc.body.as_str()))
                .collect::<Vec<_>>(),
            [
                ("summary-1", "First", "## Sooner"),
                ("summary-2", "Second", "## Later")
            ]
        );

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
