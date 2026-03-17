use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use hypr_db_parser::{
    Collection, EnhancedNote, Human, Organization, Session, SessionParticipant, TagMapping,
    Transcript,
};
use hypr_frontmatter::Document;
use hypr_version::Version;

use super::utils::{FileOp, apply_ops};
use super::version_from_name;
use crate::Result;

mod files {
    pub const META: &str = "_meta.json";
    pub const MEMO: &str = "_memo.md";
    pub const SUMMARY: &str = "_summary.md";
    pub const TRANSCRIPT: &str = "transcript.json";
}

fn tiptap_to_md(json_str: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(json_str).ok()?;
    hypr_tiptap::tiptap_json_to_md(&json).ok()
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn group_by_session_id<T, F>(items: &[T], get_id: F) -> HashMap<&str, Vec<&T>>
where
    F: Fn(&T) -> &str,
{
    let mut map: HashMap<&str, Vec<&T>> = HashMap::new();
    for item in items {
        map.entry(get_id(item)).or_default().push(item);
    }
    map
}

fn build_tag_names(data: &Collection) -> HashMap<&str, &str> {
    data.tags
        .iter()
        .map(|t| (t.id.as_str(), t.name.as_str()))
        .collect()
}

fn build_template_titles(data: &Collection) -> HashMap<&str, &str> {
    data.templates
        .iter()
        .map(|t| (t.id.as_str(), t.title.as_str()))
        .collect()
}

fn resolve_session_tags<'a>(
    session_id: &str,
    tag_mappings: &'a [TagMapping],
    tag_names: &HashMap<&str, &'a str>,
) -> Vec<&'a str> {
    tag_mappings
        .iter()
        .filter(|m| m.session_id == session_id)
        .filter_map(|m| tag_names.get(m.tag_id.as_str()).copied())
        .collect()
}

pub struct Migrate;

impl super::Migration for Migrate {
    fn introduced_in(&self) -> &'static Version {
        version_from_name!()
    }

    fn run<'a>(&self, base_dir: &'a Path) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(run_inner(base_dir))
    }
}

async fn run_inner(base_dir: &Path) -> Result<()> {
    let sqlite_path = base_dir.join("db.sqlite");
    if !sqlite_path.exists() {
        return Ok(());
    }

    if hypr_db_parser::v1::validate(&sqlite_path).await.is_err() {
        return Ok(());
    }

    let data = hypr_db_parser::v1::parse_from_sqlite(&sqlite_path).await?;
    let ops = collect_ops(base_dir, &data)?;
    apply_ops(ops)?;

    Ok(())
}

fn collect_ops(base_dir: &Path, data: &Collection) -> Result<Vec<FileOp>> {
    let mut ops = vec![];

    ops.extend(collect_session_ops(base_dir, data)?);
    ops.extend(collect_human_ops(base_dir, data)?);
    ops.extend(collect_organization_ops(base_dir, data)?);

    Ok(ops)
}

fn collect_session_ops(base_dir: &Path, data: &Collection) -> Result<Vec<FileOp>> {
    let sessions_dir = base_dir.join("sessions");

    let transcripts = group_by_session_id(&data.transcripts, |t| &t.session_id);
    let participants = group_by_session_id(&data.participants, |p| &p.session_id);
    let enhanced_notes = group_by_session_id(&data.enhanced_notes, |n| &n.session_id);
    let tag_names = build_tag_names(data);
    let template_titles = build_template_titles(data);

    let mut ops = vec![];

    for session in &data.sessions {
        let dir = sessions_dir.join(&session.id);
        let sid = session.id.as_str();

        let session_participants = participants.get(sid).map(|v| v.as_slice()).unwrap_or(&[]);
        let session_tags = resolve_session_tags(sid, &data.tag_mappings, &tag_names);

        // _meta.json (always)
        ops.push(FileOp::Write {
            path: dir.join(files::META),
            content: build_meta_json(session, session_participants, &session_tags),
        });

        // transcript.json (if exists)
        if let Some(transcripts) = transcripts.get(sid)
            && let Some(t) = transcripts.first() {
                ops.push(FileOp::Write {
                    path: dir.join(files::TRANSCRIPT),
                    content: build_transcript_json(t),
                });
            }

        // _memo.md (if user has notes)
        ops.extend(build_memo_op(&dir, session));

        // _summary.md or {template}.md (AI-generated notes)
        if let Some(notes) = enhanced_notes.get(sid) {
            ops.extend(build_enhanced_note_ops(&dir, notes, &template_titles));
        }
    }

    Ok(ops)
}

fn collect_human_ops(base_dir: &Path, data: &Collection) -> Result<Vec<FileOp>> {
    let humans_dir = base_dir.join("humans");

    let ops = data
        .humans
        .iter()
        .map(|human| {
            // humans/{id}.md
            FileOp::Write {
                path: humans_dir.join(format!("{}.md", human.id)),
                content: build_human_doc(human),
            }
        })
        .collect();

    Ok(ops)
}

fn collect_organization_ops(base_dir: &Path, data: &Collection) -> Result<Vec<FileOp>> {
    let orgs_dir = base_dir.join("organizations");

    let ops = data
        .organizations
        .iter()
        .map(|org| {
            // organizations/{id}.md
            FileOp::Write {
                path: orgs_dir.join(format!("{}.md", org.id)),
                content: build_organization_doc(org),
            }
        })
        .collect();

    Ok(ops)
}

fn build_meta_json(
    session: &Session,
    participants: &[&SessionParticipant],
    tags: &[&str],
) -> String {
    let participants_json: Vec<serde_json::Value> = participants
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "user_id": p.user_id,
                "session_id": p.session_id,
                "human_id": p.human_id,
                "source": p.source,
            })
        })
        .collect();

    let tags_json: Option<Vec<&str>> = if tags.is_empty() {
        None
    } else {
        Some(tags.to_vec())
    };

    let meta = serde_json::json!({
        "id": session.id,
        "user_id": session.user_id,
        "created_at": session.created_at,
        "title": session.title,
        "event_id": session.event_id,
        "participants": participants_json,
        "tags": tags_json,
    });

    serde_json::to_string_pretty(&meta).unwrap()
}

fn build_transcript_json(transcript: &Transcript) -> String {
    let words: Vec<serde_json::Value> = transcript
        .words
        .iter()
        .map(|w| {
            serde_json::json!({
                "id": w.id,
                "text": w.text,
                "start_ms": w.start_ms,
                "end_ms": w.end_ms,
                "channel": w.channel,
                "speaker": w.speaker,
            })
        })
        .collect();

    let data = serde_json::json!({
        "transcripts": [{
            "id": transcript.id,
            "user_id": transcript.user_id,
            "created_at": transcript.created_at,
            "session_id": transcript.session_id,
            "started_at": transcript.started_at,
            "ended_at": transcript.ended_at,
            "words": words,
            "speaker_hints": [],
        }]
    });

    serde_json::to_string_pretty(&data).unwrap()
}

fn build_memo_op(dir: &Path, session: &Session) -> Option<FileOp> {
    let content = build_memo_content(session)?;
    Some(FileOp::Write {
        path: dir.join(files::MEMO),
        content,
    })
}

fn build_enhanced_note_ops(
    dir: &Path,
    notes: &[&EnhancedNote],
    template_titles: &HashMap<&str, &str>,
) -> Vec<FileOp> {
    notes
        .iter()
        .filter_map(|note| {
            let content = build_enhanced_note_content(note)?;
            let filename = get_enhanced_note_filename(note, template_titles);
            Some(FileOp::Write {
                path: dir.join(filename),
                content,
            })
        })
        .collect()
}

fn build_memo_content(session: &Session) -> Option<String> {
    let raw_md = session.raw_md.as_ref()?;
    if raw_md.is_empty() {
        return None;
    }

    let md_content = tiptap_to_md(raw_md).unwrap_or_default();
    if md_content.trim().is_empty() {
        return None;
    }

    let frontmatter = serde_json::json!({
        "id": session.id,
        "session_id": session.id,
    });

    let doc = Document::new(frontmatter, &md_content);
    doc.render().ok()
}

fn build_enhanced_note_content(note: &EnhancedNote) -> Option<String> {
    if note.content.is_empty() {
        return None;
    }

    let md_content = tiptap_to_md(&note.content).unwrap_or_default();
    if md_content.trim().is_empty() {
        return None;
    }

    let mut frontmatter = serde_json::json!({
        "id": note.id,
        "session_id": note.session_id,
    });

    if let Some(template_id) = &note.template_id {
        frontmatter["template_id"] = serde_json::json!(template_id);
    }
    if note.position != 0 {
        frontmatter["position"] = serde_json::json!(note.position);
    }
    if !note.title.is_empty() {
        frontmatter["title"] = serde_json::json!(note.title);
    }

    let doc = Document::new(frontmatter, &md_content);
    doc.render().ok()
}

fn get_enhanced_note_filename(
    note: &EnhancedNote,
    template_titles: &HashMap<&str, &str>,
) -> String {
    match &note.template_id {
        Some(template_id) => {
            let title = template_titles
                .get(template_id.as_str())
                .copied()
                .unwrap_or(template_id.as_str());
            format!("{}.md", sanitize_filename(title))
        }
        None => files::SUMMARY.to_string(),
    }
}

fn build_human_doc(human: &Human) -> String {
    let emails: Vec<&str> = human
        .email
        .as_ref()
        .map(|e| e.split(',').map(|s| s.trim()).collect())
        .unwrap_or_default();

    let frontmatter = serde_json::json!({
        "user_id": human.user_id,
        "name": human.name,
        "emails": emails,
        "org_id": human.org_id.as_deref().unwrap_or(""),
        "job_title": human.job_title.as_deref().unwrap_or(""),
        "linkedin_username": human.linkedin_username.as_deref().unwrap_or(""),
        "pinned": false,
    });

    let doc = Document::new(frontmatter, "");
    doc.render().unwrap()
}

fn build_organization_doc(org: &Organization) -> String {
    let frontmatter = serde_json::json!({
        "user_id": org.user_id,
        "name": org.name,
        "description": org.description.as_deref().unwrap_or(""),
    });

    let doc = Document::new(frontmatter, "");
    doc.render().unwrap()
}
