mod convert;

use hypr_db_core::libsql;
use hypr_db_user::UserDatabase;
use std::path::Path;

use crate::types::*;
use crate::{Error, Result};
use convert::{html_to_markdown, session_to_transcript};

const EXPECTED_TABLES: &[&str] = &["sessions", "humans", "organizations", "templates", "tags"];

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
    validate(path).await?;

    let db = hypr_db_core::DatabaseBuilder::default()
        .local(path)
        .build()
        .await?;
    let db = UserDatabase::from(db);

    // Older Char DBs can have `sessions.words` as NULL/empty, but db-user's
    // `Session::from_row` expects a non-null JSON string.
    let conn = db.conn()?;
    conn.execute(
        "UPDATE sessions SET words = '[]' WHERE words IS NULL OR words = ''",
        (),
    )
    .await
    .map_err(hypr_db_user::Error::from)?;

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
