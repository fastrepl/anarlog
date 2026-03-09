use crate::ir::Collection;
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Serialize)]
struct OrganizationOutput {
    user_id: String,
    created_at: String,
    name: String,
}

#[derive(Serialize)]
struct HumanOutput {
    user_id: String,
    created_at: String,
    name: String,
    email: String,
    org_id: String,
    job_title: String,
    linkedin_username: String,
    memo: String,
}

#[derive(Serialize)]
struct SessionOutput {
    user_id: String,
    created_at: String,
    folder_id: String,
    event_id: String,
    title: String,
    raw_md: String,
}

#[derive(Serialize)]
struct EnhancedNoteOutput {
    user_id: String,
    created_at: String,
    session_id: String,
    content: String,
    template_id: String,
    position: i32,
    title: String,
}

#[derive(Serialize)]
struct TranscriptOutput {
    user_id: String,
    created_at: String,
    session_id: String,
    started_at: i64,
    ended_at: i64,
    words: String,
    speaker_hints: String,
}

#[derive(Serialize)]
struct WordInTranscript {
    id: String,
    text: String,
    start_ms: i64,
    end_ms: i64,
    channel: i32,
}

#[derive(Serialize)]
struct SessionParticipantOutput {
    user_id: String,
    created_at: String,
    session_id: String,
    human_id: String,
    source: String,
}

#[derive(Serialize)]
struct TagOutput {
    user_id: String,
    created_at: String,
    name: String,
}

#[derive(Serialize)]
struct TagSessionMappingOutput {
    user_id: String,
    created_at: String,
    tag_id: String,
    session_id: String,
}

pub fn to_tinybase_json(data: &Collection, user_id: &str) -> Value {
    let mut tables: Map<String, Value> = Map::new();

    insert_organizations(&mut tables, data, user_id);
    insert_humans(&mut tables, data, user_id);
    insert_sessions(&mut tables, data, user_id);
    insert_enhanced_notes(&mut tables, data, user_id);
    insert_transcripts_and_words(&mut tables, data, user_id);
    insert_participants(&mut tables, data, user_id);
    insert_tags(&mut tables, data, user_id);

    serde_json::json!([Value::Object(tables), Value::Null])
}

fn insert_organizations(tables: &mut Map<String, Value>, data: &Collection, user_id: &str) {
    if data.organizations.is_empty() {
        return;
    }

    let entries: Map<String, Value> = data
        .organizations
        .iter()
        .map(|org| {
            let value = OrganizationOutput {
                user_id: user_id.to_string(),
                created_at: normalize_datetime(&org.created_at),
                name: org.name.clone(),
            };
            (org.id.clone(), serde_json::to_value(value).unwrap())
        })
        .collect();

    tables.insert("organizations".to_string(), Value::Object(entries));
}

fn insert_humans(tables: &mut Map<String, Value>, data: &Collection, user_id: &str) {
    if data.humans.is_empty() {
        return;
    }

    let entries: Map<String, Value> = data
        .humans
        .iter()
        .map(|human| {
            let value = HumanOutput {
                user_id: user_id.to_string(),
                created_at: normalize_datetime(&human.created_at),
                name: human.name.clone(),
                email: human.email.clone().unwrap_or_default(),
                org_id: human.org_id.clone().unwrap_or_default(),
                job_title: human.job_title.clone().unwrap_or_default(),
                linkedin_username: human.linkedin_username.clone().unwrap_or_default(),
                memo: String::new(),
            };
            (human.id.clone(), serde_json::to_value(value).unwrap())
        })
        .collect();

    tables.insert("humans".to_string(), Value::Object(entries));
}

fn insert_sessions(tables: &mut Map<String, Value>, data: &Collection, user_id: &str) {
    if data.sessions.is_empty() {
        return;
    }

    let entries: Map<String, Value> = data
        .sessions
        .iter()
        .map(|session| {
            let value = SessionOutput {
                user_id: user_id.to_string(),
                created_at: normalize_datetime(&session.created_at),
                folder_id: session.folder_id.clone().unwrap_or_default(),
                event_id: session.event_id.clone().unwrap_or_default(),
                title: session.title.clone(),
                raw_md: session.raw_md.clone().unwrap_or_default(),
            };
            (session.id.clone(), serde_json::to_value(value).unwrap())
        })
        .collect();

    tables.insert("sessions".to_string(), Value::Object(entries));
}

fn insert_enhanced_notes(tables: &mut Map<String, Value>, data: &Collection, user_id: &str) {
    if data.enhanced_notes.is_empty() {
        return;
    }

    let entries: Map<String, Value> = data
        .enhanced_notes
        .iter()
        .map(|note| {
            let value = EnhancedNoteOutput {
                user_id: user_id.to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                session_id: note.session_id.clone(),
                content: note.content.clone(),
                template_id: note.template_id.clone().unwrap_or_default(),
                position: note.position,
                title: note.title.clone(),
            };
            (note.id.clone(), serde_json::to_value(value).unwrap())
        })
        .collect();

    tables.insert("enhanced_notes".to_string(), Value::Object(entries));
}

fn insert_transcripts_and_words(tables: &mut Map<String, Value>, data: &Collection, user_id: &str) {
    if data.transcripts.is_empty() {
        return;
    }

    let mut transcript_entries: Map<String, Value> = Map::new();

    for transcript in &data.transcripts {
        let words_json: Vec<WordInTranscript> = transcript
            .words
            .iter()
            .map(|word| WordInTranscript {
                id: word.id.clone(),
                text: word.text.clone(),
                start_ms: word.start_ms.unwrap_or(0.0) as i64,
                end_ms: word.end_ms.unwrap_or(0.0) as i64,
                channel: 0,
            })
            .collect();

        let value = TranscriptOutput {
            user_id: user_id.to_string(),
            created_at: normalize_datetime(&transcript.created_at),
            session_id: transcript.session_id.clone(),
            started_at: transcript.start_ms.unwrap_or(0.0) as i64,
            ended_at: transcript.end_ms.map(|ms| ms as i64).unwrap_or(0),
            words: serde_json::to_string(&words_json).unwrap_or_else(|_| "[]".to_string()),
            speaker_hints: "[]".to_string(),
        };
        transcript_entries.insert(transcript.id.clone(), serde_json::to_value(value).unwrap());
    }

    tables.insert("transcripts".to_string(), Value::Object(transcript_entries));
}

fn insert_participants(tables: &mut Map<String, Value>, data: &Collection, user_id: &str) {
    if data.participants.is_empty() {
        return;
    }

    let entries: Map<String, Value> = data
        .participants
        .iter()
        .map(|p| {
            let id = format!("{}_{}", p.session_id, p.human_id);
            let value = SessionParticipantOutput {
                user_id: user_id.to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                session_id: p.session_id.clone(),
                human_id: p.human_id.clone(),
                source: p.source.clone(),
            };
            (id, serde_json::to_value(value).unwrap())
        })
        .collect();

    tables.insert(
        "mapping_session_participant".to_string(),
        Value::Object(entries),
    );
}

fn insert_tags(tables: &mut Map<String, Value>, data: &Collection, user_id: &str) {
    if data.tags.is_empty() {
        return;
    }

    let tag_entries: Map<String, Value> = data
        .tags
        .iter()
        .map(|tag| {
            let value = TagOutput {
                user_id: user_id.to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                name: tag.name.clone(),
            };
            (tag.id.clone(), serde_json::to_value(value).unwrap())
        })
        .collect();

    let mapping_entries: Map<String, Value> = data
        .tag_mappings
        .iter()
        .map(|mapping| {
            let value = TagSessionMappingOutput {
                user_id: user_id.to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                tag_id: mapping.tag_id.clone(),
                session_id: mapping.session_id.clone(),
            };
            (mapping.id.clone(), serde_json::to_value(value).unwrap())
        })
        .collect();

    tables.insert("tags".to_string(), Value::Object(tag_entries));
    tables.insert(
        "mapping_tag_session".to_string(),
        Value::Object(mapping_entries),
    );
}

fn normalize_datetime(s: &str) -> String {
    if s.is_empty() {
        return chrono::Utc::now().to_rfc3339();
    }

    if chrono::DateTime::parse_from_rfc3339(s).is_ok() {
        return s.to_string();
    }

    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return naive.and_utc().to_rfc3339();
    }

    chrono::Utc::now().to_rfc3339()
}
