use std::{
    collections::{HashMap, VecDeque},
    path::Path,
    sync::Arc,
};

use anlg_db_execute::TransactionStatement;
use futures::TryStreamExt;
use icu_collator::{Collator, CollatorBorrowed, options::CollatorOptions};
use icu_locale_core::Locale;
use serde_json::json;
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use uuid::Uuid;

use crate::{
    CancellationToken, DocumentSnapshot, LibraryPage, LibraryQuery, MAX_DOCUMENT_BYTES,
    OpenSession, RenameSession, Reply, Result, RuntimeHandle, SaveDocument, ServiceError, Services,
    SessionId, SessionSummary, types::failure,
};

const PAGE_STRIDE: u32 = 256;
const CACHE_ROWS: u32 = PAGE_STRIDE + 200;
const CACHE_BYTES: usize = 8 * 1024 * 1024;
const TAG_LINE_BYTES: usize = 4096;
const TAG_WHITESPACE: &str = "\t\n\u{000b}\u{000c}\r \u{00a0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}";

#[derive(Default)]
struct TagNames {
    names: Vec<String>,
    bytes: usize,
}

impl TagNames {
    fn insert(&mut self, name: &str, collator: &CollatorBorrowed<'_>) {
        let name = name.trim_matches(|ch| TAG_WHITESPACE.contains(ch));
        if name.is_empty() {
            return;
        }
        let index = match self.names.binary_search_by(|existing| {
            collator
                .compare(existing, name)
                .then_with(|| existing.as_str().cmp(name))
        }) {
            Ok(_) => return,
            Err(index) => index,
        };
        if index == self.names.len() && self.bytes.saturating_sub(1) >= TAG_LINE_BYTES {
            return;
        }
        self.bytes += name.len() + 2;
        self.names.insert(index, name.into());
        while let Some(last) = self.names.last() {
            let remaining = self.bytes - last.len() - 2;
            if remaining.saturating_sub(1) < TAG_LINE_BYTES {
                break;
            }
            self.bytes = remaining;
            self.names.pop();
        }
    }

    fn line(self) -> Arc<str> {
        let mut line = self
            .names
            .into_iter()
            .map(|name| format!("#{name}"))
            .collect::<Vec<_>>()
            .join(" ");
        line.truncate(line.floor_char_boundary(TAG_LINE_BYTES.min(line.len())));
        line.into()
    }
}

impl Services {
    pub async fn session_tag_lines(&self, ids: &[&str]) -> Result<HashMap<String, Arc<str>>> {
        if ids.len() > CACHE_ROWS as usize {
            return Err(ServiceError::Unsupported(
                "Too many sessions for tag projection".into(),
            ));
        }
        let locale = tag_locale(sys_locale::get_locale().as_deref());
        let collator =
            Collator::try_new(locale.into(), CollatorOptions::default()).map_err(failure)?;
        let mut rows = sqlx::query_as::<_, (String, String)>(
            "SELECT st.session_id, substr(ltrim(t.name, ?2), 1, 4096)
             FROM session_tags st JOIN tags t ON t.id=st.tag_id AND t.deleted_at IS NULL
             WHERE st.session_id IN (SELECT value FROM json_each(?1)) AND st.deleted_at IS NULL",
        )
        .bind(json!(ids).to_string())
        .bind(TAG_WHITESPACE)
        .fetch(self.db.pool());
        let mut tags: HashMap<String, TagNames> = HashMap::new();
        while let Some((session, name)) = rows.try_next().await.map_err(failure)? {
            tags.entry(session).or_default().insert(&name, &collator);
        }
        Ok(tags
            .into_iter()
            .map(|(id, tags)| (id, tags.line()))
            .collect())
    }
}

fn tag_locale(name: Option<&str>) -> Locale {
    if matches!(name, Some("C" | "POSIX")) || (name.is_none() && cfg!(target_os = "linux")) {
        return icu_locale_core::locale!("en-US-posix");
    }
    name.and_then(|name| name.parse().ok())
        .unwrap_or(icu_locale_core::locale!("en"))
}

pub(crate) struct Cache {
    /// `None` once shutdown closed it.
    observer: Option<SqliteConnection>,
    version: i64,
    pages: VecDeque<(PageKey, LibraryPage)>,
    bytes: usize,
}

/// Search text and sort order of a cached page.
type PageKey = (Arc<str>, bool);

impl Cache {
    pub(crate) async fn new(path: &Path) -> Result<Self> {
        let observer = SqliteConnection::connect_with(
            &SqliteConnectOptions::new().filename(path).read_only(true),
        )
        .await
        .map_err(failure)?;
        Ok(Self {
            observer: Some(observer),
            version: -1,
            pages: VecDeque::new(),
            bytes: 0,
        })
    }

    /// Shutdown closes the observer with the pool, so no connection outlives it.
    pub(crate) async fn close(&mut self) {
        if let Some(observer) = self.observer.take() {
            let _ = observer.close().await;
        }
    }

    async fn version(&mut self) -> Result<i64> {
        let observer = self.observer.as_mut().ok_or(ServiceError::Closed)?;
        sqlx::query_scalar("PRAGMA data_version")
            .fetch_one(observer)
            .await
            .map_err(failure)
    }

    fn clear(&mut self, version: i64) {
        self.pages.clear();
        self.bytes = 0;
        self.version = version;
    }
}

fn page_slice(page: &LibraryPage, offset: u32, limit: u32) -> LibraryPage {
    let start = (offset - page.offset) as usize;
    let end = (start + limit as usize).min(page.items.len());
    LibraryPage {
        items: page
            .items
            .get(start..end)
            .unwrap_or_default()
            .to_vec()
            .into(),
        offset,
        has_more: end < page.items.len() || page.has_more,
    }
}

/// Notes ordered like shipping's timeline: by the start of the calendar event they were created
/// from (the live event, else the copy saved on the note), falling back to creation time.
fn library_sql(filter: &str, oldest: bool) -> String {
    let order = if oldest { "ASC" } else { "DESC" };
    format!(
        "WITH base AS (
            SELECT id, title, created_at, updated_at, folder_path, event_id,
                CASE WHEN json_valid(event_json) THEN event_json END AS saved
            FROM sessions
            WHERE deleted_at IS NULL AND {filter}
        ), linked AS (
            SELECT base.*, COALESCE(
                (SELECT e.id FROM events e WHERE e.id = base.event_id AND e.deleted_at IS NULL),
                (SELECT e.id FROM events e
                    WHERE e.deleted_at IS NULL AND e.tracking_id_event <> ''
                        AND e.calendar_id = COALESCE(json_extract(base.saved, '$.calendar_id'), '')
                        AND e.tracking_id_event = json_extract(base.saved, '$.tracking_id')
                    ORDER BY e.started_at, e.id LIMIT 1)) AS live
            FROM base
        ), timed AS (
            SELECT linked.*,
                CASE WHEN live IS NULL THEN json_extract(saved, '$.started_at')
                    ELSE (SELECT started_at FROM events WHERE id = live) END AS event_start,
                CASE WHEN live IS NULL THEN json_extract(saved, '$.ended_at')
                    ELSE (SELECT ended_at FROM events WHERE id = live) END AS event_end
            FROM linked
        )
        SELECT id, title, created_at, updated_at, folder_path,
            COALESCE(CAST(event_start AS TEXT), '') AS event_start,
            COALESCE(CAST(event_end AS TEXT), '') AS event_end
        FROM timed
        ORDER BY julianday(COALESCE(NULLIF(event_start, ''), created_at)) {order},
            created_at {order}, id {order}
        LIMIT ?2 OFFSET ?3"
    )
}

const SEARCH_FILTER: &str =
    "(?1 = '' OR instr(lower(title), lower(?1)) > 0 OR instr(lower(id), lower(?1)) > 0)";

#[derive(sqlx::FromRow)]
struct LibraryRow {
    id: String,
    title: String,
    created_at: String,
    updated_at: String,
    folder_path: String,
    event_start: String,
    event_end: String,
}

/// Reads `limit` rows (and whether more follow) with their tag lines.
async fn read_rows(
    services: &Services,
    search: &str,
    oldest: bool,
    offset: u32,
    limit: u32,
) -> Result<(Vec<SessionSummary>, bool)> {
    let mut rows =
        sqlx::query_as::<_, LibraryRow>(sqlx::AssertSqlSafe(library_sql(SEARCH_FILTER, oldest)))
            .bind(search.trim())
            .bind(i64::from(limit) + 1)
            .bind(i64::from(offset))
            .fetch_all(services.db.pool())
            .await
            .map_err(failure)?;
    let has_more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let mut tags = HashMap::new();
    for chunk in rows.chunks(CACHE_ROWS as usize) {
        let ids = chunk.iter().map(|row| row.id.as_str()).collect::<Vec<_>>();
        tags.extend(services.session_tag_lines(&ids).await?);
    }
    let items = rows
        .into_iter()
        .map(|row| SessionSummary {
            tag_line: tags.remove(&row.id).unwrap_or_default(),
            id: row.id.into(),
            title: row.title.into(),
            updated_at: row.updated_at.into(),
            created_at: row.created_at.into(),
            folder_path: row.folder_path.into(),
            event_start: row.event_start.into(),
            event_end: row.event_end.into(),
        })
        .collect();
    Ok((items, has_more))
}

impl RuntimeHandle {
    pub fn library(
        &self,
        query: LibraryQuery,
        cancel: CancellationToken,
    ) -> Result<Reply<LibraryPage>> {
        self.read(cancel, move |services| async move {
            if query.limit > 200 {
                let (items, has_more) = read_rows(
                    &services,
                    &query.search,
                    query.oldest,
                    query.offset,
                    query.limit,
                )
                .await?;
                return Ok(LibraryPage {
                    items: items.into(),
                    offset: query.offset,
                    has_more,
                });
            }
            let limit = query.limit.max(1);
            let offset = query.offset / PAGE_STRIDE * PAGE_STRIDE;
            let key: PageKey = (query.search.clone(), query.oldest);
            let mut cache = services.library.lock().await;
            let version = cache.version().await?;
            if version != cache.version {
                cache.clear(version);
            }
            if let Some(index) = cache
                .pages
                .iter()
                .position(|(cached, page)| *cached == key && page.offset == offset)
            {
                let entry = cache.pages.remove(index).expect("cache index exists");
                let result = page_slice(&entry.1, query.offset, limit);
                cache.pages.push_front(entry);
                return Ok(result);
            }
            let (items, has_more) =
                read_rows(&services, &query.search, query.oldest, offset, CACHE_ROWS).await?;
            let page = LibraryPage {
                items: items.into(),
                offset,
                has_more,
            };
            let result = page_slice(&page, query.offset, limit);
            if cache.version().await? == version {
                let bytes = key.0.len()
                    + page
                        .items
                        .iter()
                        .map(|row| {
                            row.id.0.len()
                                + row.title.len()
                                + row.updated_at.len()
                                + row.created_at.len()
                                + row.folder_path.len()
                                + row.tag_line.len()
                                + row.event_start.len()
                                + row.event_end.len()
                                + std::mem::size_of::<SessionSummary>()
                        })
                        .sum::<usize>();
                if cache.pages.len() >= 8 || cache.bytes.saturating_add(bytes) > CACHE_BYTES {
                    cache.clear(version);
                }
                if bytes <= CACHE_BYTES {
                    cache.bytes += bytes;
                    cache.pages.push_front((key, page));
                }
            }
            Ok(result)
        })
    }

    pub fn open_session(
        &self,
        id: SessionId,
        cancel: CancellationToken,
    ) -> Result<Reply<OpenSession>> {
        self.read(cancel, move |services| async move {
            load_session(&services, id).await
        })
    }

    pub fn create_note(&self, title: Arc<str>) -> Result<Reply<OpenSession>> {
        self.submit(move |services| async move {
            let id = Uuid::new_v4().to_string();
            services
                .executor
                .execute_transaction(vec![
                    // Like shipping's `insert_session`: the note joins the synced workspace and
                    // is owned by the active library, or CloudSync never uploads it.
                    TransactionStatement {
                        sql: "INSERT INTO sessions (id, title, kind, workspace_id, owner_user_id)
                              SELECT ?, ?, 'note', COALESCE(binding.workspace_id, ''),
                                COALESCE((SELECT library_workspace_id FROM local_library_connections WHERE active = 1),
                                         NULLIF(binding.workspace_id, ''), '')
                              FROM (SELECT (SELECT json_extract(value_json, '$.workspace_id') FROM app_settings
                                            WHERE id = 'cloudsync_workspace_binding') AS workspace_id) AS binding"
                            .into(),
                        params: vec![json!(id), json!(title)],
                        expected_rows_affected: Some(1),
                    },
                    TransactionStatement {
                        sql: "INSERT INTO session_documents (id, session_id, body, workspace_id)
                              SELECT ?, id, ?, workspace_id FROM sessions WHERE id = ?"
                            .into(),
                        params: vec![
                            json!(id),
                            json!({"type":"doc","content":[{"type":"paragraph"}]})
                                .to_string()
                                .into(),
                            json!(id),
                        ],
                        expected_rows_affected: Some(1),
                    },
                ])
                .await
                .map_err(failure)?;
            load_session(&services, id.into()).await
        })
    }

    pub fn rename_session(&self, input: RenameSession) -> Result<Reply<OpenSession>> {
        self.submit(move |services| async move {
            services.executor.execute_transaction(vec![TransactionStatement {
                sql: "UPDATE sessions SET title = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ? AND title = ? AND updated_at = ? AND deleted_at IS NULL".into(),
                params: vec![json!(input.title), json!(input.base.id), json!(input.base.title), json!(input.base.updated_at)],
                expected_rows_affected: Some(1),
            }]).await.map_err(map_write_error)?;
            load_session(&services, input.base.id).await
        })
    }

    pub fn save_document(&self, input: SaveDocument) -> Result<Reply<DocumentSnapshot>> {
        if input.body.len() > MAX_DOCUMENT_BYTES {
            return Err(ServiceError::Unsupported(
                "Document exceeds the 16 MiB save limit; original preserved.".into(),
            ));
        }
        self.submit(move |services| async move {
            // Shipping decodes Markdown notes for editing and stores the edit as ProseMirror JSON.
            if !matches!(input.base.body_format.as_ref(), "prosemirror_json" | "markdown") {
                return Err(ServiceError::Unsupported("Document format is read-only.".into()));
            }
            let body: serde_json::Value = serde_json::from_str(&input.body).map_err(failure)?;
            if body.get("type").and_then(serde_json::Value::as_str) != Some("doc") {
                return Err(ServiceError::Unsupported("Expected a ProseMirror doc root.".into()));
            }
            if input.base.is_unsaved() {
                // Shipping's memo upsert, restricted so it can only create: a live memo that
                // appeared meanwhile (or a document of another session with this id) is a
                // conflict, never overwritten.
                let row = services.executor.execute(
                    "INSERT INTO session_documents (id, workspace_id, session_id, kind, body_format, body, created_by, updated_by) SELECT ?, workspace_id, id, 'note', 'prosemirror_json', ?, owner_user_id, owner_user_id FROM sessions WHERE id = ? AND deleted_at IS NULL AND NOT EXISTS (SELECT 1 FROM session_documents WHERE session_id = sessions.id AND kind = 'note' AND deleted_at IS NULL) ON CONFLICT(id) DO UPDATE SET kind = 'note', body_format = excluded.body_format, body = excluded.body, updated_by = excluded.updated_by, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'), deleted_at = NULL WHERE session_documents.deleted_at IS NOT NULL AND session_documents.session_id = excluded.session_id RETURNING id, session_id, body_format, body, updated_at".into(),
                    vec![json!(input.base.id), json!(input.body), json!(input.base.session_id)],
                ).await.map_err(failure)?.into_iter().next().ok_or(ServiceError::Conflict)?;
                return serde_json::from_value(row).map_err(failure);
            }
            let row = services.executor.execute(
                "UPDATE session_documents SET body = ?, body_format = 'prosemirror_json', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ? AND session_id = ? AND body = ? AND body_format = ? AND updated_at = ? AND deleted_at IS NULL AND EXISTS (SELECT 1 FROM sessions WHERE id = session_documents.session_id AND deleted_at IS NULL) RETURNING id, session_id, body_format, body, updated_at".into(),
                vec![json!(input.body), json!(input.base.id), json!(input.base.session_id), json!(input.base.body), json!(input.base.body_format), json!(input.base.updated_at)],
            ).await.map_err(failure)?.into_iter().next().ok_or(ServiceError::Conflict)?;
            serde_json::from_value(row).map_err(failure)
        })
    }
}

fn map_write_error(error: anlg_db_execute::Error) -> ServiceError {
    match error {
        anlg_db_execute::Error::UnexpectedRowsAffected { .. } => ServiceError::Conflict,
        error => failure(error),
    }
}

async fn load_session(services: &Services, id: SessionId) -> Result<OpenSession> {
    let row = anlg_db_app::get_session(services.db.pool(), &id.0)
        .await
        .map_err(failure)?
        .ok_or(ServiceError::Conflict)?;
    let note = anlg_db_app::get_session_note(services.db.pool(), &id.0)
        .await
        .map_err(failure)?
        .map(|row| DocumentSnapshot {
            id: row.id.into(),
            session_id: row.session_id.into(),
            body_format: row.body_format.into(),
            body: row.body.into(),
            updated_at: row.updated_at.into(),
        });
    let (event_start, event_end) =
        sqlx::query_as::<_, LibraryRow>(sqlx::AssertSqlSafe(library_sql("id = ?1", false)))
            .bind(id.0.as_ref())
            .bind(1_i64)
            .bind(0_i64)
            .fetch_optional(services.db.pool())
            .await
            .map_err(failure)?
            .map(|row| (row.event_start, row.event_end))
            .unwrap_or_default();
    Ok(OpenSession {
        summary: SessionSummary {
            event_start: event_start.into(),
            event_end: event_end.into(),
            tag_line: services
                .session_tag_lines(&[&id.0])
                .await?
                .remove(id.0.as_ref())
                .unwrap_or_default(),
            id,
            title: row.title.into(),
            updated_at: row.updated_at.into(),
            created_at: row.created_at.into(),
            folder_path: row.folder_path.into(),
        },
        note,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posix_tags_match_javascriptcore_case_and_accent_order() {
        for locale in ["C", "POSIX", "en-US-posix"] {
            let collator =
                Collator::try_new(tag_locale(Some(locale)).into(), CollatorOptions::default())
                    .unwrap();
            let mut tags = TagNames::default();
            for name in ["é", "e", "É", "E", "日本"] {
                tags.insert(name, &collator);
            }
            assert_eq!(tags.line().as_ref(), "#E #É #e #é #日本");
        }
        assert_eq!(tag_locale(Some("sv-SE")).to_string(), "sv-SE");
        assert_eq!(tag_locale(Some("invalid locale")).to_string(), "en");
    }

    #[test]
    fn tags_trim_deduplicate_and_collate_without_splitting_commas() {
        let collator = Collator::try_new(
            icu_locale_core::locale!("en").into(),
            CollatorOptions::default(),
        )
        .unwrap();
        let mut tags = TagNames::default();
        for tag in [
            "zebra",
            "  alpha ",
            "éclair",
            "\u{feff}日本\u{feff}",
            "alpha",
            "Alpha",
            "launch, prep",
            "\u{a0}",
        ] {
            tags.insert(tag, &collator);
        }
        assert_eq!(
            tags.line().as_ref(),
            "#alpha #Alpha #éclair #launch, prep #zebra #日本"
        );
    }

    #[test]
    fn tag_order_uses_the_system_collators_locale_rules() {
        let collator = Collator::try_new(
            icu_locale_core::locale!("sv").into(),
            CollatorOptions::default(),
        )
        .unwrap();
        let mut tags = TagNames::default();
        for tag in ["öga", "Zebra", "åland", "äpple", "apple"] {
            tags.insert(tag, &collator);
        }
        assert_eq!(tags.line().as_ref(), "#apple #Zebra #åland #äpple #öga");
    }

    #[test]
    fn streamed_tags_keep_the_sorted_visible_prefix_with_bounded_memory() {
        let collator = Collator::try_new(
            icu_locale_core::locale!("en").into(),
            CollatorOptions::default(),
        )
        .unwrap();
        let mut tags = TagNames::default();
        let mut all = Vec::new();
        for index in (0..5000).rev() {
            let name = format!("tag-{index:04}-日本🚀");
            tags.insert(&name, &collator);
            tags.insert(&name, &collator);
            all.push(format!("#{name}"));
            assert!(tags.bytes <= TAG_LINE_BYTES + name.len() + 2);
        }
        all.reverse();
        let expected = all.join(" ");
        assert_eq!(
            tags.line().as_ref(),
            &expected[..expected.floor_char_boundary(TAG_LINE_BYTES)]
        );
    }
}
