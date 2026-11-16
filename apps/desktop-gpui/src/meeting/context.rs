use std::sync::Arc;

use anlg_template_app::{
    ContextBlock, EnhanceSystem, EnhanceTemplate, EnhanceUser, Participant, Segment, Session,
    SessionContext, Template, Transcript,
};
use desktop_runtime::{
    CancellationToken, DocumentId, DocumentSnapshot, Result, RuntimeHandle, ServiceError, SessionId,
};
use serde_json::{Value, json};

use super::{
    ai::{Message, Part, Role, SummaryContext},
    ai_view::{AiContext, ContextPurpose, ContextResolver},
    config::preferences,
    model::{MAX_TEXT, failure},
    store::{TranscriptStore, statement, string},
};

pub fn resolver(runtime: RuntimeHandle) -> ContextResolver {
    Arc::new(move |session, purpose| {
        let runtime = runtime.clone();
        Box::pin(async move {
            match purpose {
                ContextPurpose::Preview => {
                    load_context(&runtime, session, SummaryRequest::Preview).await
                }
                ContextPurpose::Summarize => load(&runtime, session).await,
            }
        })
    })
}

pub async fn load(runtime: &RuntimeHandle, session: SessionId) -> Result<AiContext> {
    load_context(
        runtime,
        session,
        SummaryRequest::Prepare {
            target: None,
            template: None,
        },
    )
    .await
}

/// The summary document to write and its context. `target` regenerates that summary;
/// `template` is an explicit choice (`Some(None)` is Auto), otherwise shipping's enhancer
/// order applies: the summary's template, the memo's, then the default template.
pub async fn load_summary(
    runtime: &RuntimeHandle,
    session: SessionId,
    target: Option<DocumentId>,
    template: Option<Option<String>>,
) -> Result<(DocumentSnapshot, SummaryContext)> {
    load_context(
        runtime,
        session,
        SummaryRequest::Prepare { target, template },
    )
    .await?
    .summary
    .ok_or(ServiceError::Conflict)
}

enum SummaryRequest {
    Preview,
    Prepare {
        target: Option<DocumentId>,
        template: Option<Option<String>>,
    },
}

/// Shipping's `resolveTemplateId` for a summary: an explicit choice wins, then the first
/// non-empty of the summary's, the memo's and the default template.
fn summary_template(
    explicit: Option<Option<String>>,
    summary: &str,
    memo: &str,
    default: &str,
) -> String {
    match explicit {
        Some(choice) => choice.unwrap_or_default(),
        None => [summary, memo, default]
            .into_iter()
            .find(|id| !id.is_empty())
            .unwrap_or_default()
            .to_owned(),
    }
}

async fn load_context(
    runtime: &RuntimeHandle,
    session: SessionId,
    request: SummaryRequest,
) -> Result<AiContext> {
    let settings = preferences(runtime).await?;
    let template_id = settings.text("selected_template_id");
    let content = snapshot(runtime, session.clone(), template_id, request).await?;
    let transcripts = TranscriptStore(runtime.clone())
        .load(session.clone(), CancellationToken::new())?
        .receive()
        .await?;
    let transcript = Transcript {
        started_at: transcripts
            .transcripts
            .iter()
            .filter_map(|t| u64::try_from(t.started_at).ok())
            .min(),
        ended_at: None,
        segments: transcripts
            .segments
            .iter()
            .map(|segment| Segment {
                speaker: segment.speaker_label.clone(),
                text: segment.text.clone(),
                start_ms: Some(segment.start_ms),
            })
            .collect(),
        // Shipping marks "now" only while the session is recording; this context has no capture state yet.
        now_ms: None,
    };
    let language = Some(settings.text("ai_language")).filter(|s| !s.is_empty());
    let system = anlg_template_app::render(Template::ChatSystem(anlg_template_app::ChatSystem {
        language: language.clone(),
    }))
    .map_err(failure)?;
    let block = anlg_template_app::render(Template::ContextBlock(ContextBlock {
        contexts: vec![SessionContext {
            session_id: Some(session.0.to_string()),
            title: content.session.title.clone(),
            date: content.session.started_at.clone(),
            raw_content: Some(content.memo.clone()),
            enhanced_content: content
                .summary
                .as_ref()
                .map(|summary| markdown(&summary.body, &summary.body_format))
                .transpose()?,
            meeting_chat: None,
            transcript: Some(transcript.clone()),
            participants: content.participants.clone(),
            event: content.session.event.clone(),
        }],
        current_session_id: Some(session.0.to_string()),
    }))
    .map_err(failure)?;
    let prompt = format!("{system}\n\n{block}");
    if prompt.len() > MAX_TEXT {
        return Err(failure("Meeting context exceeds the provider input limit."));
    }
    let mut history = vec![Message {
        id: format!("context:{}", session.0),
        role: Role::System,
        parts: vec![Part::Text { text: prompt }],
    }];
    history.extend(content.history);
    let summary = SummaryContext {
        system: EnhanceSystem {
            language,
            format_override: if content.template.is_none() {
                settings.text("auto_summary_prompt")
            } else {
                String::new()
            },
        },
        user: EnhanceUser {
            session: content.session,
            participants: content.participants,
            template: content.template,
            transcripts: vec![transcript],
            pre_meeting_memo: content.pre_memo,
            post_meeting_memo: content.memo,
        },
    };
    Ok(AiContext {
        group: session.0,
        history,
        summary: content.summary.map(|base| (base, summary)),
    })
}

struct Snapshot {
    session: Session,
    memo: String,
    pre_memo: String,
    participants: Vec<Participant>,
    template: Option<EnhanceTemplate>,
    summary: Option<DocumentSnapshot>,
    history: Vec<Message>,
}

async fn snapshot(
    runtime: &RuntimeHandle,
    session: SessionId,
    default_template: String,
    request: SummaryRequest,
) -> Result<Snapshot> {
    runtime.submit(move |services| async move {
        let rows = services.executor.execute("SELECT title, created_at, event_json, owner_user_id, workspace_id FROM sessions WHERE id = ? AND deleted_at IS NULL AND locked = 0".into(), vec![json!(session)]).await.map_err(failure)?;
        let row = rows.first().ok_or(ServiceError::Conflict)?;
        let title = string(row, "title")?.to_owned();
        let event: Value = serde_json::from_str(string(row, "event_json")?).unwrap_or(Value::Null);
        let event_name = event["title"].as_str().or_else(|| event["name"].as_str()).map(str::to_owned);
        let docs = services.executor.execute("SELECT id, session_id, body_format, body, updated_at, kind, template_id FROM session_documents WHERE session_id = ? AND deleted_at IS NULL ORDER BY sort_order, created_at, id LIMIT 101".into(), vec![json!(session)]).await.map_err(failure)?;
        if docs.len() > 100 { return Err(failure("Meeting contains too many documents for AI context.")); }
        let is_summary = |document: &Value| document["kind"] == "summary" || document["kind"] == "template_output";
        let (target, explicit) = match request {
            SummaryRequest::Preview => (None, None),
            SummaryRequest::Prepare { target, template } => (target, Some(template)),
        };
        let chosen = match &target {
            Some(id) => Some(docs.iter().find(|document| document["id"] == json!(id) && is_summary(document)).ok_or(ServiceError::Conflict)?),
            None => docs.iter().find(|document| is_summary(document)),
        };
        let memo_template = docs.iter().find(|document| document["kind"] == "note").and_then(|document| document["template_id"].as_str()).unwrap_or_default();
        let chosen_template = chosen.and_then(|document| document["template_id"].as_str()).unwrap_or_default();
        let template_id = match &explicit {
            Some(explicit) => summary_template(explicit.clone(), chosen_template, memo_template, &default_template),
            None => chosen.map_or(default_template.clone(), |_| chosen_template.to_owned()),
        };
        let template_row = if template_id.is_empty() { None } else {
            services.executor.execute("SELECT title, description, sections_json FROM templates WHERE id = ?".into(), vec![json!(template_id)]).await.map_err(failure)?.into_iter().next()
        };
        let template = template_row.as_ref().map(|row| Ok::<_, ServiceError>(EnhanceTemplate {
            title: string(row, "title")?.into(),
            description: row["description"].as_str().map(str::to_owned),
            sections: serde_json::from_str(string(row, "sections_json")?).map_err(failure)?,
        })).transpose()?;
        let summary_title = template_row.as_ref().and_then(|row| row["title"].as_str()).map(str::trim).filter(|title| !title.is_empty()).unwrap_or("Summary").to_owned();
        let kind = if template_id.is_empty() { "summary" } else { "template_output" };
        let mut memo = String::new();
        for document in &docs {
            if string(document, "body")?.len() > MAX_TEXT { return Err(failure("Meeting document exceeds context limit.")); }
            if document["kind"] == "note" { memo = markdown(string(document, "body")?, string(document, "body_format")?)?; }
        }
        let summary = match (chosen, explicit.is_some()) {
            (Some(document), true) if document["template_id"] != json!(template_id) || document["kind"] != kind => {
                // Shipping's `replaceSummaryDocumentTemplate`, keeping the body until the new
                // summary replaces it.
                let id = string(document, "id")?.to_owned();
                services.executor.execute_transaction(vec![statement(
                    "UPDATE session_documents SET kind = ?, template_id = ?, title = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND session_id = ? AND kind IN ('summary', 'template_output') AND deleted_at IS NULL",
                    vec![json!(kind), json!(template_id), json!(summary_title), json!(id), json!(session)], Some(1)),
                ]).await.map_err(failure)?;
                let rows = services.executor.execute("SELECT id, session_id, body_format, body, updated_at FROM session_documents WHERE id = ?".into(), vec![json!(id)]).await.map_err(failure)?;
                Some(document_snapshot(rows.first().ok_or(ServiceError::Conflict)?)?)
            }
            (Some(document), _) => Some(document_snapshot(document)?),
            (None, true) => {
                let id = uuid::Uuid::new_v4().to_string();
                let body = json!({"type": "doc", "content": []}).to_string();
                services.executor.execute_transaction(vec![statement(
                    "INSERT INTO session_documents (id, session_id, workspace_id, kind, template_id, title, body_format, body) SELECT ?, id, workspace_id, ?, ?, ?, 'prosemirror_json', ? FROM sessions WHERE id = ? AND deleted_at IS NULL AND locked = 0",
                    vec![json!(id), json!(kind), json!(template_id), json!(summary_title), json!(body), json!(session)], Some(1)),
                ]).await.map_err(failure)?;
                let rows = services.executor.execute("SELECT id, session_id, body_format, body, updated_at FROM session_documents WHERE id = ?".into(), vec![json!(id)]).await.map_err(failure)?;
                Some(document_snapshot(rows.first().ok_or(ServiceError::Conflict)?)?)
            }
            (None, false) => None,
        };
        let participants = services.executor.execute("SELECT COALESCE(NULLIF(h.name, ''), p.display_name) AS name, h.job_title FROM session_participants p LEFT JOIN humans h ON h.id = p.human_id AND h.deleted_at IS NULL WHERE p.session_id = ? AND p.source <> 'excluded' AND p.deleted_at IS NULL ORDER BY p.id LIMIT 500".into(), vec![json!(session)]).await.map_err(failure)?
            .into_iter().map(|row| Participant { name: row["name"].as_str().unwrap_or_default().into(), job_title: row["job_title"].as_str().map(str::to_owned) }).collect();
        let rows = services.executor.execute("SELECT memo FROM transcripts WHERE session_id = ? AND deleted_at IS NULL ORDER BY started_at_ms, id LIMIT 1".into(), vec![json!(session)]).await.map_err(failure)?;
        let pre_memo = rows.first().and_then(|row| row["memo"].as_str()).unwrap_or_default().to_owned();
        let rows = services.executor.execute("SELECT id, role, content, parts_json FROM chat_messages WHERE chat_group_id = ? AND deleted_at IS NULL ORDER BY created_at DESC, id DESC LIMIT 20".into(), vec![json!(session)]).await.map_err(failure)?;
        let mut history = Vec::new();
        for row in rows.into_iter().rev() {
            let parts_json = string(&row, "parts_json")?;
            if parts_json.len() > MAX_TEXT { return Err(failure("Saved chat message exceeds context limit.")); }
            let mut parts = history_parts(parts_json)?;
            if parts.is_empty() { parts.push(Part::Text { text: string(&row, "content")?.into() }); }
            history.push(Message { id: string(&row, "id")?.into(), role: serde_json::from_value(row["role"].clone()).map_err(failure)?, parts });
        }
        Ok(Snapshot {
            session: Session { title: Some(title), started_at: Some(string(row, "created_at")?.into()), ended_at: None, event: event_name.map(|name| anlg_template_app::Event { name }) },
            memo, pre_memo, participants, template, summary, history,
        })
    })?.receive().await
}

pub fn markdown(body: &str, format: &str) -> Result<String> {
    if body.is_empty() {
        return Ok(String::new());
    }
    match format {
        "markdown" | "text" => Ok(body.into()),
        "prosemirror_json" => {
            anlg_tiptap::tiptap_json_to_md(&serde_json::from_str(body).map_err(failure)?)
                .map_err(failure)
        }
        _ => Err(failure("Document format cannot be used as AI context.")),
    }
}

fn history_parts(encoded: &str) -> Result<Vec<Part>> {
    let parts: Vec<Value> = serde_json::from_str(encoded).map_err(failure)?;
    parts.into_iter().filter(|part| part["type"] != "step-start").map(|mut part| {
        if let Some(name) = part["type"].as_str().and_then(|kind| kind.strip_prefix("tool-")).map(str::to_owned) {
            part["type"] = json!("dynamic-tool");
            part["toolName"] = json!(name);
        }
        if part["type"] == "dynamic-tool" {
            if part.get("input").is_none() { part["input"] = Value::Null; }
            if part.get("output").is_none() {
                part["output"] = json!({"state":part["state"],"error":part["errorText"]});
            }
        }
        serde_json::from_value(part).map_err(|_| failure("This saved chat contains a part the native assistant cannot read; the original is preserved."))
    }).collect()
}

pub(super) fn document_snapshot(row: &Value) -> Result<DocumentSnapshot> {
    Ok(DocumentSnapshot {
        id: string(row, "id")?.to_owned().into(),
        session_id: string(row, "session_id")?.to_owned().into(),
        body_format: string(row, "body_format")?.into(),
        body: string(row, "body")?.into(),
        updated_at: string(row, "updated_at")?.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_template_follows_shipping_enhancer_order() {
        let pick = |explicit, summary, memo| summary_template(explicit, summary, memo, "default");
        assert_eq!(pick(Some(None), "summary", "memo"), "");
        assert_eq!(
            pick(Some(Some("chosen".into())), "summary", "memo"),
            "chosen"
        );
        assert_eq!(pick(None, "summary", "memo"), "summary");
        assert_eq!(pick(None, "", "memo"), "memo");
        assert_eq!(pick(None, "", ""), "default");
    }

    #[test]
    fn source_tool_parts_load_without_rewriting_persisted_history() {
        let encoded = r#"[{"type":"step-start"},{"type":"text","text":"회의"},{"type":"tool-search_meetings","toolCallId":"call","state":"output-available","input":{"query":"회의"},"output":[]}]"#;
        let parts = history_parts(encoded).unwrap();
        assert_eq!(parts.len(), 2);
        assert!(
            matches!(&parts[1], Part::Tool { name, call_id, .. } if name == "search_meetings" && call_id == "call")
        );
        assert!(history_parts(r#"[{"type":"unknown-rich-part"}]"#).is_err());
    }
}
