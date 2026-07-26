mod convert;

use hypr_importer_core::ir::CollectionStats;
use legacy_db_core::libsql;
use legacy_db_user::UserDatabase;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::types::*;
use crate::{Error, Result};
use convert::{html_to_markdown, session_to_transcript};

const EXPECTED_TABLES: &[&str] = &["sessions", "humans", "organizations", "templates", "tags"];

struct SqliteSnapshot {
    _dir: tempfile::TempDir,
    path: PathBuf,
}

impl SqliteSnapshot {
    fn create(path: &Path) -> Result<Self> {
        let dir = tempfile::tempdir()?;
        let file_name = path.file_name().ok_or_else(|| {
            Error::InvalidData(format!(
                "v0 database path has no file name: {}",
                path.display()
            ))
        })?;
        let snapshot_path = dir.path().join(file_name);

        std::fs::copy(path, &snapshot_path)?;
        copy_sidecar_if_exists(path, dir.path(), file_name, "-wal")?;
        copy_sidecar_if_exists(path, dir.path(), file_name, "-shm")?;

        Ok(Self {
            _dir: dir,
            path: snapshot_path,
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

fn copy_sidecar_if_exists(
    source_db_path: &Path,
    snapshot_dir: &Path,
    file_name: &OsStr,
    suffix: &str,
) -> Result<()> {
    let source_path = source_db_path.with_file_name(sidecar_file_name(file_name, suffix));
    if !source_path.exists() {
        return Ok(());
    }

    let target_path = snapshot_dir.join(sidecar_file_name(file_name, suffix));
    std::fs::copy(source_path, target_path)?;
    Ok(())
}

fn sidecar_file_name(file_name: &OsStr, suffix: &str) -> std::ffi::OsString {
    let mut name = file_name.to_os_string();
    name.push(suffix);
    name
}

pub async fn validate(path: &Path) -> Result<()> {
    let db = libsql::Builder::new_local(path).build().await?;
    let conn = db.connect()?;

    let mut rows = conn
        .query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            (),
        )
        .await?;

    let mut tables = Vec::new();
    while let Some(row) = rows.next().await? {
        tables.push(row.get::<String>(0)?);
    }

    for expected in EXPECTED_TABLES {
        if !tables.iter().any(|t| t == *expected) {
            return Err(Error::InvalidData(format!(
                "v0 database missing required table: {}",
                expected
            )));
        }
    }

    if tables.len() < 10 {
        return Err(Error::InvalidData(format!(
            "v0 database expected 10+ tables, found {}",
            tables.len()
        )));
    }

    Ok(())
}

pub async fn parse_from_sqlite(path: &Path) -> Result<Collection> {
    let snapshot = SqliteSnapshot::create(path)?;
    parse_from_snapshot(snapshot.path()).await
}

pub async fn parse_stats_from_sqlite(path: &Path) -> Result<CollectionStats> {
    let snapshot = SqliteSnapshot::create(path)?;
    validate(snapshot.path()).await?;

    let db = libsql::Builder::new_local(snapshot.path()).build().await?;
    let conn = db.connect()?;
    normalize_empty_words(&conn).await?;
    let (sessions_count, transcripts_count) = count_session_rows(&conn).await?;

    Ok(CollectionStats {
        sessions_count,
        transcripts_count,
        humans_count: count_rows(&conn, "SELECT COUNT(*) FROM humans").await?,
        organizations_count: count_rows(&conn, "SELECT COUNT(*) FROM organizations").await?,
        participants_count: count_rows(
            &conn,
            "SELECT COUNT(*)
             FROM session_participants sp
             JOIN sessions s ON s.id = sp.session_id
             JOIN humans h ON h.id = sp.human_id
             WHERE sp.deleted = FALSE OR sp.deleted IS NULL",
        )
        .await?,
        templates_count: count_rows(&conn, "SELECT COUNT(*) FROM templates").await?,
        enhanced_notes_count: count_rows(
            &conn,
            "SELECT COUNT(*) FROM sessions WHERE COALESCE(enhanced_memo_html, '') <> ''",
        )
        .await?,
    })
}

async fn count_session_rows(conn: &libsql::Connection) -> Result<(usize, usize)> {
    let mut rows = conn
        .query(
            "SELECT raw_memo_html, enhanced_memo_html, words FROM sessions",
            (),
        )
        .await?;
    let mut sessions_count = 0;
    let mut transcripts_count = 0;

    while let Some(row) = rows.next().await? {
        let raw_memo_html: String = row.get(0)?;
        let enhanced_memo_html: Option<String> = row.get(1)?;
        let words_json: String = row.get(2)?;
        let words: Vec<owhisper_interface::Word2> = serde_json::from_str(&words_json)?;

        if !words.is_empty() {
            transcripts_count += 1;
        }

        if !legacy_db_user::is_session_content_empty(
            &raw_memo_html,
            enhanced_memo_html.as_deref(),
            words.is_empty(),
        ) {
            sessions_count += 1;
        }
    }

    Ok((sessions_count, transcripts_count))
}

async fn count_rows(conn: &libsql::Connection, sql: &str) -> Result<usize> {
    let mut rows = conn.query(sql, ()).await?;
    let row = rows
        .next()
        .await?
        .ok_or_else(|| Error::InvalidData("count query returned no rows".to_string()))?;
    let count: i64 = row.get(0)?;
    Ok(count.max(0) as usize)
}

async fn parse_from_snapshot(path: &Path) -> Result<Collection> {
    validate(path).await?;

    let db = legacy_db_core::DatabaseBuilder::default()
        .local(path)
        .build()
        .await?;
    let db = UserDatabase::from(db);

    let conn = db.conn()?;
    normalize_empty_words(&conn).await?;

    let sessions_raw = db.list_sessions(None).await?;

    let mut sessions = Vec::new();
    let mut transcripts = Vec::new();
    let mut participants = Vec::new();
    let mut enhanced_notes = Vec::new();
    let mut tags = Vec::new();
    let mut tag_mappings = Vec::new();

    for session in sessions_raw {
        let session_participants = db.session_list_participants(&session.id).await?;
        for human in session_participants {
            participants.push(SessionParticipant {
                id: format!("{}-{}", session.id, human.id),
                user_id: String::new(),
                session_id: session.id.clone(),
                human_id: human.id,
                source: "imported".to_string(),
            });
        }

        if !session.words.is_empty() {
            transcripts.push(session_to_transcript(&session));
        }

        if let Some(ref enhanced_html) = session.enhanced_memo_html {
            if !enhanced_html.is_empty() {
                enhanced_notes.push(EnhancedNote {
                    id: format!("enhanced-{}", session.id),
                    user_id: String::new(),
                    session_id: session.id.clone(),
                    content: enhanced_html.clone(),
                    template_id: None,
                    position: 1,
                    title: String::new(),
                });
            }
        }

        let session_tags = db.list_session_tags(&session.id).await?;
        for tag in session_tags {
            let tag_id = tag.id.clone();
            if !tags.iter().any(|t: &Tag| t.id == tag_id) {
                tags.push(Tag {
                    id: tag.id.clone(),
                    user_id: String::new(),
                    name: tag.name.clone(),
                });
            }
            tag_mappings.push(TagMapping {
                id: format!("{}-{}", tag.id, session.id),
                user_id: String::new(),
                tag_id: tag.id,
                session_id: session.id.clone(),
            });
        }

        if !session.is_empty() {
            let raw_md = if !session.raw_memo_html.is_empty() {
                Some(html_to_markdown(&session.raw_memo_html))
            } else {
                None
            };

            let enhanced_content = session
                .enhanced_memo_html
                .as_ref()
                .filter(|s| !s.is_empty())
                .map(|s| html_to_markdown(s));

            sessions.push(Session {
                id: session.id.clone(),
                user_id: String::new(),
                created_at: session.created_at.to_rfc3339(),
                title: session.title,
                raw_md,
                enhanced_content,
                folder_id: None,
                event_id: session.calendar_event_id,
            });
        }
    }

    let humans = db
        .list_humans(None)
        .await?
        .into_iter()
        .map(|h| Human {
            id: h.id,
            user_id: String::new(),
            created_at: String::new(),
            name: h.full_name.unwrap_or_default(),
            email: h.email,
            org_id: h.organization_id,
            job_title: h.job_title,
            linkedin_username: h.linkedin_username,
        })
        .collect();

    let organizations = db
        .list_organizations(None)
        .await?
        .into_iter()
        .map(|o| Organization {
            id: o.id,
            user_id: String::new(),
            created_at: String::new(),
            name: o.name,
            description: o.description,
        })
        .collect();

    let templates = db
        .list_templates("")
        .await?
        .into_iter()
        .map(|t| Template {
            id: t.id,
            user_id: String::new(),
            title: t.title,
            description: t.description,
            sections: t
                .sections
                .into_iter()
                .map(|s| TemplateSection {
                    title: s.title,
                    description: s.description,
                })
                .collect(),
            tags: t.tags,
            context_option: t.context_option,
        })
        .collect();

    Ok(Collection {
        sessions,
        transcripts,
        humans,
        organizations,
        participants,
        templates,
        enhanced_notes,
        tags,
        tag_mappings,
    })
}

async fn normalize_empty_words(conn: &libsql::Connection) -> Result<()> {
    // Older Char DBs can have `sessions.words` as NULL/empty, but db-user's
    // `Session::from_row` expects a non-null JSON string.
    conn.execute(
        "UPDATE sessions SET words = '[]' WHERE words IS NULL OR words = ''",
        (),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use legacy_db_core::DatabaseBuilder;
    use legacy_db_user::{UserDatabase, migrate};

    async fn setup_db(path: &Path) -> UserDatabase {
        let db = DatabaseBuilder::default()
            .local(path)
            .build()
            .await
            .unwrap();
        let db = UserDatabase::from(db);
        migrate(&db).await.unwrap();
        db
    }

    async fn seed_rows(db: &UserDatabase) {
        let conn = db.conn().unwrap();
        conn.execute(
            r#"
            INSERT INTO organizations (id, name, description)
            VALUES ('org-1', 'Acme', 'Customer')
            "#,
            (),
        )
        .await
        .unwrap();
        conn.execute(
            r#"
            INSERT INTO humans (
                id,
                organization_id,
                is_user,
                full_name,
                email,
                job_title,
                linkedin_username
            ) VALUES (
                'human-1',
                'org-1',
                FALSE,
                'Ada Lovelace',
                'ada@example.com',
                'Engineer',
                'ada'
            )
            "#,
            (),
        )
        .await
        .unwrap();
        conn.execute(
            r#"
            INSERT INTO sessions (
                id,
                created_at,
                visited_at,
                user_id,
                title,
                raw_memo_html,
                enhanced_memo_html,
                conversations,
                words
            ) VALUES (
                'session-1',
                '2026-01-01T00:00:00Z',
                '2026-01-01T00:00:00Z',
                'human-1',
                'Legacy meeting',
                '<p>Notes</p>',
                '<p>Summary</p>',
                '[]',
                '[{"text":"Hello","speaker":null,"confidence":1,"start_ms":1000,"end_ms":1500}]'
            )
            "#,
            (),
        )
        .await
        .unwrap();
        conn.execute(
            r#"
            INSERT INTO session_participants (session_id, human_id, deleted)
            VALUES ('session-1', 'human-1', FALSE)
            "#,
            (),
        )
        .await
        .unwrap();
        conn.execute(
            r#"
            INSERT INTO templates (
                id,
                user_id,
                title,
                description,
                sections,
                tags,
                context_option
            ) VALUES (
                'template-1',
                'human-1',
                'Template',
                'Description',
                '[]',
                '[]',
                NULL
            )
            "#,
            (),
        )
        .await
        .unwrap();
        conn.execute(
            "INSERT INTO tags (id, name) VALUES ('tag-1', 'Important')",
            (),
        )
        .await
        .unwrap();
        conn.execute(
            "INSERT INTO tags_sessions (tag_id, session_id) VALUES ('tag-1', 'session-1')",
            (),
        )
        .await
        .unwrap();
    }

    async fn insert_session_candidate(
        db: &UserDatabase,
        id: &str,
        title: &str,
        raw_memo_html: &str,
        enhanced_memo_html: Option<&str>,
        words: &str,
    ) {
        let conn = db.conn().unwrap();
        conn.execute(
            r#"
            INSERT INTO sessions (
                id,
                created_at,
                visited_at,
                user_id,
                title,
                raw_memo_html,
                enhanced_memo_html,
                conversations,
                words
            ) VALUES (
                :id,
                '2026-01-01T00:00:00Z',
                '2026-01-01T00:00:00Z',
                'human-1',
                :title,
                :raw_memo_html,
                :enhanced_memo_html,
                '[]',
                :words
            )
            "#,
            legacy_db_core::libsql::named_params! {
                ":id": id,
                ":title": title,
                ":raw_memo_html": raw_memo_html,
                ":enhanced_memo_html": enhanced_memo_html,
                ":words": words,
            },
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn parse_stats_from_sqlite_counts_rows_without_full_import() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db.sqlite");
        let db = setup_db(&path).await;
        seed_rows(&db).await;

        let stats = parse_stats_from_sqlite(&path).await.unwrap();

        assert_eq!(stats.sessions_count, 1);
        assert_eq!(stats.transcripts_count, 1);
        assert_eq!(stats.humans_count, 1);
        assert_eq!(stats.organizations_count, 1);
        assert_eq!(stats.participants_count, 1);
        assert_eq!(stats.templates_count, 1);
        assert_eq!(stats.enhanced_notes_count, 1);
    }

    #[tokio::test]
    async fn parse_stats_from_sqlite_uses_import_empty_session_filter() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db.sqlite");
        let db = setup_db(&path).await;
        seed_rows(&db).await;
        insert_session_candidate(&db, "title-only", "Title only", "", None, "[]").await;
        insert_session_candidate(&db, "raw-empty-html", "", "<p></p>", None, "[]").await;
        insert_session_candidate(&db, "enhanced-empty-html", "", "", Some("<p></p>"), "[]").await;

        let stats = parse_stats_from_sqlite(&path).await.unwrap();
        let collection = parse_from_sqlite(&path).await.unwrap();

        assert_eq!(stats.sessions_count, collection.sessions.len());
        assert_eq!(stats.sessions_count, 1);
        assert_eq!(stats.transcripts_count, collection.transcripts.len());
        assert_eq!(stats.transcripts_count, 1);
    }

    #[tokio::test]
    async fn parse_stats_from_sqlite_counts_only_joined_participants() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db.sqlite");
        let db = setup_db(&path).await;
        seed_rows(&db).await;

        let conn = db.conn().unwrap();
        conn.execute("PRAGMA foreign_keys = OFF", ()).await.unwrap();
        conn.execute(
            r#"
            INSERT INTO session_participants (session_id, human_id, deleted)
            VALUES ('session-1', 'missing-human', FALSE)
            "#,
            (),
        )
        .await
        .unwrap();

        let stats = parse_stats_from_sqlite(&path).await.unwrap();
        let collection = parse_from_sqlite(&path).await.unwrap();

        assert_eq!(stats.participants_count, collection.participants.len());
        assert_eq!(stats.participants_count, 1);
    }

    #[tokio::test]
    async fn parse_from_sqlite_does_not_mutate_source_empty_words() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db.sqlite");
        let db = setup_db(&path).await;
        seed_rows(&db).await;

        let conn = db.conn().unwrap();
        conn.execute("UPDATE sessions SET words = '' WHERE id = 'session-1'", ())
            .await
            .unwrap();

        parse_from_sqlite(&path).await.unwrap();

        let mut rows = conn
            .query("SELECT words FROM sessions WHERE id = 'session-1'", ())
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let words: String = row.get(0).unwrap();
        assert_eq!(words, "");
    }
}
