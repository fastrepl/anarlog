use std::path::{Path, PathBuf};
use std::sync::Arc;

use anlg_db_core::Db;
use anyhow::Context as _;

use crate::document::{self, Block};
use crate::timeline::{EventRow, SessionRow};

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

// apps/desktop/src/calendar/queries.ts, `useTimelineEventsTable`.
const TIMELINE_EVENTS_SQL: &str = "
    SELECT
      event.id,
      event.title,
      event.started_at,
      event.ended_at,
      event.tracking_id_event,
      event.is_all_day,
      event.meeting_link,
      COALESCE(calendar.color, '') AS calendar_color
    FROM events AS event
    LEFT JOIN calendars AS calendar
      ON calendar.id = event.calendar_id AND calendar.deleted_at IS NULL
    WHERE event.deleted_at IS NULL
    ORDER BY event.started_at, event.id
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
    pub blocks: Vec<Block>,
}

// apps/desktop/src/session/queries/sessions.ts, `useSessionTranscriptExistence`.
const HAS_TRANSCRIPT_SQL: &str = "
    SELECT EXISTS (
        SELECT 1
        FROM transcripts
        WHERE session_id = ?
          AND deleted_at IS NULL
          AND CASE
            WHEN json_valid(words_json) THEN json_array_length(words_json)
            ELSE 0
          END > 0
    )
";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotePreview {
    pub session: SessionRow,
    /// The raw memo (`kind = 'note'`).
    pub memo: Vec<Block>,
    /// Summaries and template outputs in the order the app tabs them.
    pub enhanced: Vec<NoteDocument>,
    pub has_transcript: bool,
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
    changes: tokio::sync::watch::Receiver<u64>,
}

const CHANGE_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(750);

impl Store {
    pub async fn open(runtime: tokio::runtime::Handle, path: PathBuf) -> anyhow::Result<Self> {
        if !path.is_file() {
            anyhow::bail!(
                "no Anarlog database at {}. Launch the desktop app once to create it, or pass --db-path.",
                path.display()
            );
        }
        let db = Arc::new(
            Db::connect_local_read_only(&path)
                .await
                .with_context(|| format!("failed to open {}", path.display()))?,
        );
        let changes = spawn_change_watcher(&runtime, db.clone()).await?;
        Ok(Self {
            runtime,
            db,
            path,
            changes,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Ticks whenever another process commits to the database. This is how
    /// the shell stays in step with the Tauri app's writes; in-process change
    /// notifications (`anlg-db-reactive`) cannot see other connections.
    pub fn changes(&self) -> tokio::sync::watch::Receiver<u64> {
        self.changes.clone()
    }

    /// Runs the sqlx futures on the tokio runtime and hands back a handle the
    /// GPUI foreground executor can await. Returns the two tables the Tauri
    /// timeline merges: sessions and calendar events.
    pub fn list_timeline(
        &self,
    ) -> tokio::task::JoinHandle<anyhow::Result<(Vec<SessionRow>, Vec<EventRow>)>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let sessions = sqlx::query_as::<_, SessionRow>(TIMELINE_SESSIONS_SQL)
                .fetch_all(db.pool())
                .await?;
            let events = sqlx::query_as::<_, EventRow>(TIMELINE_EVENTS_SQL)
                .fetch_all(db.pool())
                .await?;
            Ok((sessions, events))
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
                Some(note) => document::from_body(&note.body_format, &note.body),
                None => Vec::new(),
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
                        blocks: document::from_body(&body_format, &body),
                    })
                    .collect();
            let has_transcript: bool = sqlx::query_scalar(HAS_TRANSCRIPT_SQL)
                .bind(&session_id)
                .fetch_one(db.pool())
                .await?;
            Ok(Some(NotePreview {
                has_transcript,
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

/// `PRAGMA data_version` is scoped to one connection and increments when any
/// other connection commits, so the watcher holds a dedicated connection.
async fn spawn_change_watcher(
    runtime: &tokio::runtime::Handle,
    db: Arc<Db>,
) -> anyhow::Result<tokio::sync::watch::Receiver<u64>> {
    let mut connection = db
        .pool()
        .acquire()
        .await
        .context("failed to acquire change-watcher connection")?;
    let mut last = data_version(&mut connection).await?;
    let (tx, rx) = tokio::sync::watch::channel(0u64);
    runtime.spawn(async move {
        let mut generation = 0u64;
        loop {
            tokio::time::sleep(CHANGE_POLL_INTERVAL).await;
            match data_version(&mut connection).await {
                Ok(version) if version != last => {
                    last = version;
                    generation += 1;
                    if tx.send(generation).is_err() {
                        break;
                    }
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!(%error, "database change watcher failed; stopping");
                    break;
                }
            }
        }
    });
    Ok(rx)
}

async fn data_version(
    connection: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
) -> anyhow::Result<i64> {
    let version: i64 = sqlx::query_scalar("PRAGMA data_version")
        .fetch_one(&mut **connection)
        .await?;
    Ok(version)
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
                        ('summary-1', 'workspace-1', 'session-1', 'template_output', 'First', 'markdown', '## Sooner', 1);
                 INSERT INTO calendars (id, color) VALUES ('cal-1', '#ff0000');
                 INSERT INTO events (id, calendar_id, title, started_at, ended_at, tracking_id_event)
                 VALUES ('event-1', 'cal-1', 'Standup', '2099-01-01T09:00:00Z', '2099-01-01T09:15:00Z', 'track-1');",
            )
            .execute(db.pool())
            .await
            .unwrap();
        }

        let store = Store::open(tokio::runtime::Handle::current(), path)
            .await
            .unwrap();
        let (sessions, events) = store.list_timeline().await.unwrap().unwrap();
        assert_eq!(sessions.len(), 1, "deleted sessions stay hidden");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Standup");
        assert_eq!(events[0].calendar_color, "#ff0000");
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
        assert!(!note.has_transcript);
        assert_eq!(
            note.memo,
            vec![Block::Paragraph(vec![document::Span {
                text: "Decide on GPUI.".into(),
                ..document::Span::default()
            }])]
        );
        assert_eq!(
            note.enhanced
                .iter()
                .map(|doc| (doc.id.as_str(), doc.title.as_str(), doc.blocks.len()))
                .collect::<Vec<_>>(),
            [("summary-1", "First", 1), ("summary-2", "Second", 1)]
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
    async fn store_notices_commits_from_other_connections() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILENAME);
        let writer = Db::connect_local_plain(&path).await.unwrap();
        anlg_db_app::prepare_schema(&writer).await.unwrap();

        let store = Store::open(tokio::runtime::Handle::current(), path)
            .await
            .unwrap();
        let mut changes = store.changes();
        assert_eq!(*changes.borrow(), 0);

        sqlx::query("INSERT INTO sessions (id, workspace_id, title) VALUES ('s1', 'w1', 'New')")
            .execute(writer.pool())
            .await
            .unwrap();

        tokio::time::timeout(std::time::Duration::from_secs(5), changes.changed())
            .await
            .expect("watcher should tick after an external commit")
            .unwrap();
        assert_eq!(*changes.borrow(), 1);
        assert_eq!(store.list_timeline().await.unwrap().unwrap().0.len(), 1);
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
