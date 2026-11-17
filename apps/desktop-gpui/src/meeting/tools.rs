//! Executors for shipping's chat tools (`apps/desktop/src/chat/tools`). Results keep shipping's
//! JSON shapes, since the model and the tool cards both read them.

use std::sync::{Arc, Mutex};

use desktop_runtime::{CancellationToken, Result, RuntimeHandle, ServiceError};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tokio::sync::oneshot;

pub use super::tool_search::WebAccess;
use super::{
    ai::{MeetingTool, ReviewOpener, ToolContext, ToolExecutor},
    model::{MAX_TEXT, failure},
};
use crate::workspace::proposals::{self, NewProposal};

#[derive(Clone, Debug, PartialEq)]
pub enum EditTarget {
    Memo,
    Summary(Arc<str>),
}

/// A chat edit waiting for review, like an entry of shipping's `usePendingEditStore`.
#[derive(Clone, Debug)]
pub struct PendingEdit {
    pub id: Arc<str>,
    pub session_id: Arc<str>,
    pub target: EditTarget,
    pub current: Arc<str>,
    pub proposed: Arc<str>,
    pub source: Arc<str>,
}

/// The first waiting edit, as the meeting pane's legacy AI panel shows it.
#[derive(Clone)]
pub struct Proposal {
    pub id: Arc<str>,
    pub before: Arc<str>,
    pub after: Arc<str>,
}

struct Waiting {
    edit: PendingEdit,
    reply: oneshot::Sender<bool>,
}

/// Chat edits waiting for the user's decision, keyed by tool call.
#[derive(Clone, Default)]
pub struct Approvals(Arc<Mutex<Vec<Waiting>>>);

impl Approvals {
    fn waiting(&self) -> std::sync::MutexGuard<'_, Vec<Waiting>> {
        self.0.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    pub fn get(&self, id: &str) -> Option<PendingEdit> {
        self.waiting()
            .iter()
            .find(|waiting| waiting.edit.id.as_ref() == id)
            .map(|waiting| waiting.edit.clone())
    }

    pub fn is_pending(&self, id: &str) -> bool {
        self.get(id).is_some()
    }

    /// Shipping's `resolveEdit`: answers the waiting tool and forgets the edit.
    pub fn resolve(&self, id: &str, approved: bool) -> bool {
        let mut waiting = self.waiting();
        let Some(index) = waiting
            .iter()
            .position(|entry| entry.edit.id.as_ref() == id)
        else {
            return false;
        };
        let _ = waiting.remove(index).reply.send(approved);
        true
    }

    pub fn pending(&self) -> Option<Proposal> {
        self.waiting().first().map(|waiting| Proposal {
            id: waiting.edit.id.clone(),
            before: waiting.edit.current.clone(),
            after: waiting.edit.proposed.clone(),
        })
    }

    pub fn decide(&self, id: &str, approved: bool) {
        self.resolve(id, approved);
    }

    /// Registers the edit, opens its review and waits. A stopped chat forgets the edit; its
    /// proposal stays pending for the note's banner.
    async fn wait(
        &self,
        edit: PendingEdit,
        review: Option<&ReviewOpener>,
        cancel: &CancellationToken,
    ) -> Result<bool> {
        let id = edit.id.clone();
        let (reply, receive) = oneshot::channel();
        self.waiting().push(Waiting { edit, reply });
        if let Some(review) = review {
            review(id.clone());
        }
        tokio::select! {
            _ = cancel.cancelled() => {
                self.waiting().retain(|waiting| waiting.edit.id != id);
                Err(ServiceError::Cancelled)
            }
            approved = receive => approved.map_err(|_| ServiceError::Closed),
        }
    }
}

pub fn executor(
    runtime: RuntimeHandle,
    approvals: Approvals,
    web: Option<WebAccess>,
) -> ToolExecutor {
    // Meeting search uses the chat's full-text index, kept warm across tool calls.
    let index = crate::chat::search::SearchIndex::default();
    Arc::new(move |tool, context, cancellation| {
        let runtime = runtime.clone();
        let approvals = approvals.clone();
        let index = index.clone();
        let web = web.clone();
        Box::pin(async move {
            let services = Services {
                runtime: &runtime,
                approvals: &approvals,
                index: &index,
                web: web.as_ref(),
            };
            execute(services, tool, context, cancellation).await
        })
    })
}

struct Services<'a> {
    runtime: &'a RuntimeHandle,
    approvals: &'a Approvals,
    index: &'a crate::chat::search::SearchIndex,
    web: Option<&'a WebAccess>,
}

/// Shipping's `withProAccess` message for the model.
const PRO_REQUIRED: &str = "This requires Anarlog Pro. Explain that an upgrade is required; do not claim the action succeeded.";

async fn execute(
    services: Services<'_>,
    tool: MeetingTool,
    context: ToolContext,
    cancellation: CancellationToken,
) -> Result<Value> {
    let Services {
        runtime,
        approvals,
        index,
        web,
    } = services;
    if cancellation.is_cancelled() {
        return Err(ServiceError::Cancelled);
    }
    let pro_only = matches!(
        tool,
        MeetingTool::GetRecurringMeetingHistory(_)
            | MeetingTool::ReadFolderMaterial { .. }
            | MeetingTool::FindRelatedMeetings { .. }
            | MeetingTool::WebSearch { .. }
    );
    if pro_only && !context.pro {
        return Err(failure(PRO_REQUIRED));
    }
    match tool {
        MeetingTool::EditMemo {
            session_id,
            content,
        } => {
            let Some(session) = session_id.or_else(|| context.session.clone()) else {
                return Ok(
                    json!({"status": "error", "message": "No active session selected. Provide sessionId explicitly when calling edit_memo."}),
                );
            };
            let Some(meeting) = load_meeting(runtime, &session, &cancellation).await? else {
                return Ok(json!({"status": "error", "message": "Session not found."}));
            };
            let (target, current) = meeting
                .note
                .map(|note| (note.id, note.markdown))
                .unwrap_or_else(|| (session.clone(), String::new()));
            review(
                runtime,
                approvals,
                &context,
                NewProposal {
                    id: proposal_id(&context),
                    session_id: session.into(),
                    kind: proposals::MEMO,
                    target_id: target.into(),
                    current,
                    proposed: content,
                },
                EditTarget::Memo,
                &cancellation,
            )
            .await
        }
        MeetingTool::EditSummary {
            session_id,
            enhanced_note_id,
            content,
        } => {
            let Some(session) = session_id.or_else(|| context.session.clone()) else {
                return Ok(
                    json!({"status": "error", "message": "No active session selected. Provide sessionId explicitly when calling edit_summary."}),
                );
            };
            let notes = load_meeting(runtime, &session, &cancellation)
                .await?
                .map(|meeting| meeting.summaries)
                .unwrap_or_default();
            if notes.is_empty() {
                return Ok(
                    json!({"status": "error", "message": "No summaries found for this session"}),
                );
            }
            let candidates = notes
                .iter()
                .map(|note| {
                    let mut candidate = json!({
                        "enhancedNoteId": note.id,
                        "title": Some(note.title.trim()).filter(|title| !title.is_empty()).unwrap_or("Summary"),
                        "position": note.sort_order,
                    });
                    if !note.template_id.is_empty() {
                        candidate["templateId"] = json!(note.template_id);
                    }
                    candidate
                })
                .collect::<Vec<_>>();
            let known = |id: &String| notes.iter().any(|note| &note.id == id);
            if enhanced_note_id.as_ref().is_some_and(|id| !known(id)) {
                return Ok(
                    json!({"status": "error", "message": "That summary does not belong to the target session.", "candidates": candidates}),
                );
            }
            // Shipping's `resolveActiveEnhancedNoteId`: the summary shown for this note.
            let active = context
                .summary
                .clone()
                .filter(|_| context.session.as_deref() == Some(session.as_str()));
            let chosen = enhanced_note_id
                .filter(known)
                .or_else(|| active.filter(known))
                .or_else(|| {
                    notes
                        .iter()
                        .find(|note| note.template_id.is_empty())
                        .map(|note| note.id.clone())
                })
                .or_else(|| (notes.len() == 1).then(|| notes[0].id.clone()));
            let Some(chosen) = chosen else {
                return Ok(
                    json!({"status": "error", "message": "Multiple summaries exist for this session. Specify enhancedNoteId explicitly.", "candidates": candidates}),
                );
            };
            let current = notes
                .iter()
                .find(|note| note.id == chosen)
                .map(|note| note.markdown.clone())
                .unwrap_or_default();
            review(
                runtime,
                approvals,
                &context,
                NewProposal {
                    id: proposal_id(&context),
                    session_id: session.into(),
                    kind: proposals::SUMMARY,
                    target_id: chosen.clone().into(),
                    current,
                    proposed: content,
                },
                EditTarget::Summary(chosen.into()),
                &cancellation,
            )
            .await
        }
        MeetingTool::ListMeetings(input) => {
            pooled(runtime, cancellation, |pool| async move {
                anlg_agent_access::list_meetings(&pool, input)
                    .await
                    .map_err(failure)
            })
            .await
        }
        MeetingTool::GetMeeting(input) => {
            pooled(runtime, cancellation, |pool| async move {
                anlg_agent_access::get_meeting(&pool, input)
                    .await
                    .map_err(failure)
            })
            .await
        }
        MeetingTool::GetMeetingTranscript(input) => {
            pooled(runtime, cancellation, |pool| async move {
                anlg_agent_access::get_meeting_transcript(&pool, input)
                    .await
                    .map_err(failure)
            })
            .await
        }
        MeetingTool::GetRecurringMeetingHistory(input) => {
            pooled(runtime, cancellation, |pool| async move {
                anlg_agent_access::get_recurring_meeting_history(&pool, input)
                    .await
                    .map_err(failure)
            })
            .await
        }
        MeetingTool::SearchMeetings {
            query,
            filters,
            limit,
        } => {
            super::tool_search::search_meetings(
                runtime,
                index,
                query.unwrap_or_default(),
                filters,
                limit.unwrap_or(5) as usize,
                &context.folders,
                cancellation,
            )
            .await
        }
        MeetingTool::SearchMeetingContent {
            query,
            meeting_ids,
            limit,
        } => {
            super::tool_search::search_meeting_content(
                runtime,
                query,
                meeting_ids,
                limit,
                &context.folders,
                cancellation,
            )
            .await
        }
        MeetingTool::FindRelatedMeetings { meeting_id, limit } => {
            let Some(session) = meeting_id.or_else(|| context.session.clone()) else {
                return Ok(
                    json!({"status": "error", "message": "No meeting is currently open", "results": []}),
                );
            };
            super::tool_search::find_related(runtime, session, limit, cancellation).await
        }
        MeetingTool::SearchContacts { query, limit } => {
            super::tool_search::contacts(runtime, query, limit.unwrap_or(8), cancellation).await
        }
        MeetingTool::SearchCalendarEvents { query, limit } => {
            super::tool_search::calendar_events(runtime, query, limit.unwrap_or(8), cancellation)
                .await
        }
        MeetingTool::ListFolders {
            query,
            limit,
            offset,
        } => {
            crate::workspace::chat_folders::list(
                runtime,
                query.unwrap_or_default(),
                limit.unwrap_or(100),
                offset.unwrap_or(0),
            )
            .await
        }
        MeetingTool::CreateFolder { folder_path } => {
            crate::workspace::chat_folders::create(runtime, folder_path).await
        }
        MeetingTool::MoveMeetingsToFolder {
            meeting_ids,
            folder_path,
        } => {
            crate::workspace::chat_folders::move_meetings(
                runtime,
                meeting_ids,
                folder_path,
                cancellation,
            )
            .await
        }
        MeetingTool::ReadFolderMaterial {
            material_id,
            max_chars,
        } => {
            crate::workspace::chat_folders::read_material(
                runtime,
                &context.folders,
                material_id,
                max_chars,
            )
            .await
        }
        MeetingTool::ApplySessionCorrection {
            session_id,
            target,
            enhanced_note_id,
            old_text,
            new_text,
            dictionary_terms,
        } => {
            super::correction::apply(
                runtime,
                super::correction::Request {
                    explicit_session: session_id.is_some(),
                    session: session_id.or_else(|| context.session.clone()),
                    target: target.unwrap_or_else(|| "summary_and_transcript".into()),
                    summary: enhanced_note_id,
                    active_summary: context.summary.clone(),
                    old_text,
                    new_text,
                    dictionary_terms,
                },
            )
            .await
        }
        MeetingTool::MoveMeetingContents {
            source_meeting_id,
            target_meeting_id,
        } => {
            let Some(source) = source_meeting_id.or_else(|| context.session.clone()) else {
                return Ok(
                    json!({"status": "error", "message": "No source meeting selected. Provide sourceMeetingId explicitly when calling move_meeting_contents."}),
                );
            };
            super::correction::move_contents(runtime, source, target_meeting_id).await
        }
        MeetingTool::WebSearch {
            query,
            include_domains,
            exclude_domains,
            limit,
        } => {
            super::tool_search::web_search(
                web,
                json!({
                    "query": query,
                    "numResults": limit,
                    "includeDomains": include_domains,
                    "excludeDomains": exclude_domains,
                }),
            )
            .await
        }
    }
}

fn proposal_id(context: &ToolContext) -> Arc<str> {
    if context.call_id.is_empty() {
        uuid::Uuid::new_v4().to_string().into()
    } else {
        context.call_id.as_str().into()
    }
}

/// Runs an `agent-access` query on the runtime's pool and returns its JSON.
async fn pooled<T, F, Fut>(
    runtime: &RuntimeHandle,
    cancellation: CancellationToken,
    query: F,
) -> Result<Value>
where
    T: serde::Serialize + Send + 'static,
    F: FnOnce(SqlitePool) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<T>> + Send + 'static,
{
    runtime
        .read(cancellation, move |services| async move {
            let result = query(services.db.pool().clone()).await?;
            serde_json::to_value(result).map_err(failure)
        })?
        .receive()
        .await
}

pub(super) async fn load_meeting(
    runtime: &RuntimeHandle,
    session: &str,
    cancellation: &CancellationToken,
) -> Result<Option<anlg_agent_access::Meeting>> {
    let meeting_id = session.to_owned();
    runtime
        .read(cancellation.clone(), move |services| async move {
            match anlg_agent_access::get_meeting(
                services.db.pool(),
                anlg_agent_access::GetMeetingInput { meeting_id },
            )
            .await
            {
                Ok(meeting) => Ok(Some(meeting)),
                Err(anlg_agent_access::Error::NotFound(_)) => Ok(None),
                Err(error) => Err(failure(error)),
            }
        })?
        .receive()
        .await
}

/// Shipping's edit tools: store the proposal, open its review, then apply or decline it.
async fn review(
    runtime: &RuntimeHandle,
    approvals: &Approvals,
    context: &ToolContext,
    proposal: NewProposal,
    target: EditTarget,
    cancellation: &CancellationToken,
) -> Result<Value> {
    if proposal.current.len() + proposal.proposed.len() > MAX_TEXT {
        return Err(failure("Proposed edit exceeds review limit."));
    }
    let label = if target == EditTarget::Memo {
        "memo"
    } else {
        "summary"
    };
    let edit = PendingEdit {
        id: proposal.id.clone(),
        session_id: proposal.session_id.clone(),
        target,
        current: proposal.current.as_str().into(),
        proposed: proposal.proposed.as_str().into(),
        source: "chat".into(),
    };
    if proposals::persist_chat(runtime, proposal).await.is_err() {
        return Ok(
            json!({"status": "error", "message": format!("Failed to save the proposed {label} edit.")}),
        );
    }
    let id = edit.id.clone();
    if !approvals
        .wait(edit, context.review.as_ref(), cancellation)
        .await?
    {
        proposals::decline(runtime, id).await?;
        return Ok(json!({"status": "declined"}));
    }
    if proposals::apply(runtime, id).await.is_err() {
        return Ok(
            json!({"status": "error", "message": format!("Failed to apply the {label} edit.")}),
        );
    }
    Ok(json!({"status": "applied"}))
}
