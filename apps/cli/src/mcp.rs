use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::schemars::{self, JsonSchema};
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt, service::RequestContext, tool,
    tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::context::{Meeting, Transcript, load_transcripts};

const DEFAULT_LIST_LIMIT: u32 = 20;
const DEFAULT_TRANSCRIPT_LIMIT: u32 = 200;
const MAX_TRANSCRIPT_LIMIT: u32 = 500;

#[derive(Clone)]
struct AnarlogMcpServer {
    db: Arc<hypr_db_core::Db>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ListMeetingsParams {
    #[schemars(description = "Case-insensitive title or meeting id substring")]
    query: Option<String>,
    #[schemars(description = "Exact recurring series id")]
    series_id: Option<String>,
    #[schemars(description = "Maximum results; defaults to 20 and is capped at 200")]
    limit: Option<u32>,
    #[schemars(description = "Number of results to skip; defaults to 0")]
    offset: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct MeetingParams {
    #[schemars(description = "Anarlog meeting id")]
    meeting_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct TranscriptParams {
    #[schemars(description = "Anarlog meeting id")]
    meeting_id: String,
    #[schemars(description = "Word offset; defaults to 0")]
    offset: Option<u32>,
    #[schemars(description = "Maximum words; defaults to 200 and is capped at 500")]
    limit: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct HistoryParams {
    #[schemars(description = "A meeting id used to resolve its recurring series")]
    meeting_id: String,
    #[schemars(description = "Maximum meetings; defaults to 20 and is capped at 200")]
    limit: Option<u32>,
}

#[derive(Debug, Serialize)]
struct TranscriptPage {
    meeting_id: String,
    offset: u32,
    limit: u32,
    total_words: usize,
    next_offset: Option<u32>,
    text: String,
    words: Vec<serde_json::Value>,
}

#[derive(Debug, PartialEq, Eq)]
enum ResourceRequest {
    Meeting {
        meeting_id: String,
    },
    Transcript {
        meeting_id: String,
        offset: u32,
        limit: u32,
    },
    Series {
        series_id: String,
    },
}

impl AnarlogMcpServer {
    fn new(db: Arc<hypr_db_core::Db>) -> Self {
        Self { db }
    }
}

#[tool_router]
impl AnarlogMcpServer {
    #[tool(
        description = "List recent Anarlog meetings. Use query to narrow by title or meeting id.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_meetings(
        &self,
        Parameters(params): Parameters<ListMeetingsParams>,
    ) -> std::result::Result<CallToolResult, McpError> {
        let meetings = hypr_db_app::list_sessions(
            self.db.pool(),
            hypr_db_app::ListSessions {
                query: params.query.as_deref(),
                series_id: params.series_id.as_deref(),
                limit: params.limit.unwrap_or(DEFAULT_LIST_LIMIT).clamp(1, 200),
                offset: params.offset.unwrap_or(0),
            },
        )
        .await
        .map_err(internal_error)?;
        structured(&meetings)
    }

    #[tool(
        description = "Get one Anarlog meeting with its canonical note, summaries, participants, and action items. Use get_meeting_transcript separately for transcript words.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_meeting(
        &self,
        Parameters(params): Parameters<MeetingParams>,
    ) -> std::result::Result<CallToolResult, McpError> {
        let meeting = Meeting::load(self.db.as_ref(), &params.meeting_id)
            .await
            .map_err(command_error)?;
        structured(&meeting)
    }

    #[tool(
        description = "Get a bounded page of transcript words and readable text for an Anarlog meeting.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_meeting_transcript(
        &self,
        Parameters(params): Parameters<TranscriptParams>,
    ) -> std::result::Result<CallToolResult, McpError> {
        ensure_meeting(self.db.as_ref(), &params.meeting_id).await?;
        let transcripts = load_transcripts(self.db.as_ref(), &params.meeting_id)
            .await
            .map_err(command_error)?;
        let page = transcript_page(
            &params.meeting_id,
            &transcripts,
            params.offset.unwrap_or(0),
            params
                .limit
                .unwrap_or(DEFAULT_TRANSCRIPT_LIMIT)
                .clamp(1, MAX_TRANSCRIPT_LIMIT),
        );
        structured(&page)
    }

    #[tool(
        description = "List meetings in the same recurring series as the supplied meeting, newest first.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_recurring_meeting_history(
        &self,
        Parameters(params): Parameters<HistoryParams>,
    ) -> std::result::Result<CallToolResult, McpError> {
        ensure_meeting(self.db.as_ref(), &params.meeting_id).await?;
        let meetings = hypr_db_app::list_recurring_sessions(
            self.db.pool(),
            &params.meeting_id,
            params.limit.unwrap_or(DEFAULT_LIST_LIMIT).clamp(1, 200),
        )
        .await
        .map_err(internal_error)?;
        structured(&meetings)
    }
}

#[tool_handler]
impl ServerHandler for AnarlogMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_protocol_version(ProtocolVersion::V_2024_11_05)
        .with_server_info(Implementation::new(
            "anarlog",
            env!("CARGO_PKG_VERSION"),
        ))
        .with_instructions(
            "Read-only access to local Anarlog meetings, notes, summaries, transcripts, participants, action items, and recurring meeting history.",
        )
    }

    async fn list_resources(
        &self,
        params: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListResourcesResult, McpError> {
        use rmcp::model::AnnotateAble;

        let offset = params
            .and_then(|params| params.cursor)
            .map(|cursor| {
                cursor.parse::<u32>().map_err(|_| {
                    McpError::invalid_params("resource cursor must be an integer", None)
                })
            })
            .transpose()?
            .unwrap_or(0);
        let meetings = hypr_db_app::list_sessions(
            self.db.pool(),
            hypr_db_app::ListSessions {
                query: None,
                series_id: None,
                limit: DEFAULT_LIST_LIMIT,
                offset,
            },
        )
        .await
        .map_err(internal_error)?;
        let next_cursor = (meetings.len() == DEFAULT_LIST_LIMIT as usize)
            .then(|| (offset + DEFAULT_LIST_LIMIT).to_string());
        let resources = meetings
            .into_iter()
            .map(|meeting| {
                let name = if meeting.title.trim().is_empty() {
                    "Untitled meeting".to_string()
                } else {
                    meeting.title
                };
                RawResource::new(format!("anarlog://meetings/{}", meeting.id), name)
                    .with_description("Anarlog meeting context")
                    .with_mime_type("text/markdown")
                    .no_annotation()
            })
            .collect();

        Ok(ListResourcesResult {
            meta: None,
            next_cursor,
            resources,
        })
    }

    async fn list_resource_templates(
        &self,
        _params: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListResourceTemplatesResult, McpError> {
        use rmcp::model::AnnotateAble;

        Ok(ListResourceTemplatesResult::with_all_items(vec![
            RawResourceTemplate::new("anarlog://meetings/{meeting_id}", "Anarlog meeting")
                .with_description("Meeting metadata, note, summaries, people, and action items")
                .with_mime_type("text/markdown")
                .no_annotation(),
            RawResourceTemplate::new(
                "anarlog://meetings/{meeting_id}/transcript{?offset,limit}",
                "Anarlog meeting transcript",
            )
            .with_description("A bounded page of meeting transcript text")
            .with_mime_type("text/plain")
            .no_annotation(),
            RawResourceTemplate::new("anarlog://series/{series_id}", "Anarlog meeting series")
                .with_description("Recurring meeting history")
                .with_mime_type("text/markdown")
                .no_annotation(),
        ]))
    }

    async fn read_resource(
        &self,
        params: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ReadResourceResult, McpError> {
        let request = parse_resource_uri(&params.uri)?;
        let contents = match request {
            ResourceRequest::Meeting { meeting_id } => {
                let meeting = Meeting::load(self.db.as_ref(), &meeting_id)
                    .await
                    .map_err(command_error)?;
                ResourceContents::text(meeting.to_markdown(), params.uri)
                    .with_mime_type("text/markdown")
            }
            ResourceRequest::Transcript {
                meeting_id,
                offset,
                limit,
            } => {
                ensure_meeting(self.db.as_ref(), &meeting_id).await?;
                let transcripts = load_transcripts(self.db.as_ref(), &meeting_id)
                    .await
                    .map_err(command_error)?;
                let page = transcript_page(&meeting_id, &transcripts, offset, limit);
                ResourceContents::text(page.text, params.uri).with_mime_type("text/plain")
            }
            ResourceRequest::Series { series_id } => {
                let meetings = hypr_db_app::list_sessions(
                    self.db.pool(),
                    hypr_db_app::ListSessions {
                        query: None,
                        series_id: Some(&series_id),
                        limit: 100,
                        offset: 0,
                    },
                )
                .await
                .map_err(internal_error)?;
                let text = meetings
                    .into_iter()
                    .map(|meeting| {
                        let title = if meeting.title.is_empty() {
                            "Untitled"
                        } else {
                            &meeting.title
                        };
                        let date = if meeting.started_at.is_empty() {
                            &meeting.created_at
                        } else {
                            &meeting.started_at
                        };
                        format!("- {date} — [{title}](anarlog://meetings/{})", meeting.id)
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                ResourceContents::text(text, params.uri).with_mime_type("text/markdown")
            }
        };

        Ok(ReadResourceResult::new(vec![contents]))
    }
}

pub async fn serve(db: Arc<hypr_db_core::Db>) -> crate::Result<()> {
    let running = AnarlogMcpServer::new(db)
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|error| Error::operation("start MCP server", error.to_string()))?;
    running
        .waiting()
        .await
        .map_err(|error| Error::operation("run MCP server", error.to_string()))?;
    Ok(())
}

async fn ensure_meeting(
    db: &hypr_db_core::Db,
    meeting_id: &str,
) -> std::result::Result<(), McpError> {
    let exists = hypr_db_app::get_session(db.pool(), meeting_id)
        .await
        .map_err(internal_error)?
        .is_some();
    if exists {
        Ok(())
    } else {
        Err(McpError::invalid_params(
            format!("meeting '{meeting_id}' not found"),
            None,
        ))
    }
}

fn transcript_page(
    meeting_id: &str,
    transcripts: &[Transcript],
    offset: u32,
    limit: u32,
) -> TranscriptPage {
    let mut words = Vec::new();
    for transcript in transcripts {
        for word in &transcript.words {
            let mut word = word.clone();
            if let Some(object) = word.as_object_mut() {
                object.insert(
                    "transcript_id".to_string(),
                    serde_json::Value::String(transcript.id.clone()),
                );
            }
            words.push(word);
        }
    }

    let total_words = words.len();
    let offset_usize = offset as usize;
    let limit = limit.clamp(1, MAX_TRANSCRIPT_LIMIT);
    let page = words
        .into_iter()
        .skip(offset_usize)
        .take(limit as usize)
        .collect::<Vec<_>>();
    let text = if page.is_empty() && offset == 0 {
        transcripts
            .iter()
            .map(|transcript| transcript.text.trim())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    } else {
        page.iter()
            .filter_map(|word| word.get("text").and_then(serde_json::Value::as_str))
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let consumed = offset_usize.saturating_add(page.len());

    TranscriptPage {
        meeting_id: meeting_id.to_string(),
        offset,
        limit,
        total_words,
        next_offset: (consumed < total_words).then_some(consumed as u32),
        text,
        words: page,
    }
}

fn parse_resource_uri(uri: &str) -> std::result::Result<ResourceRequest, McpError> {
    let url = url::Url::parse(uri)
        .map_err(|_| McpError::invalid_params("invalid Anarlog resource URI", None))?;
    if url.scheme() != "anarlog" {
        return Err(McpError::invalid_params(
            "resource URI must use the anarlog scheme",
            None,
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| McpError::invalid_params("resource URI is missing a type", None))?;
    let segments = url
        .path_segments()
        .map(|segments| {
            segments
                .filter(|segment| !segment.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    match (host, segments.as_slice()) {
        ("meetings", [meeting_id]) => Ok(ResourceRequest::Meeting {
            meeting_id: (*meeting_id).to_string(),
        }),
        ("meetings", [meeting_id, "transcript"]) => {
            let mut offset = 0;
            let mut limit = DEFAULT_TRANSCRIPT_LIMIT;
            for (key, value) in url.query_pairs() {
                match key.as_ref() {
                    "offset" => {
                        offset = value.parse().map_err(|_| {
                            McpError::invalid_params("transcript offset must be an integer", None)
                        })?;
                    }
                    "limit" => {
                        limit = value.parse::<u32>().map_err(|_| {
                            McpError::invalid_params("transcript limit must be an integer", None)
                        })?;
                    }
                    _ => {}
                }
            }
            Ok(ResourceRequest::Transcript {
                meeting_id: (*meeting_id).to_string(),
                offset,
                limit: limit.clamp(1, MAX_TRANSCRIPT_LIMIT),
            })
        }
        ("series", [series_id]) => Ok(ResourceRequest::Series {
            series_id: (*series_id).to_string(),
        }),
        _ => Err(McpError::invalid_params(
            "unsupported Anarlog resource URI",
            None,
        )),
    }
}

fn structured(value: &impl Serialize) -> std::result::Result<CallToolResult, McpError> {
    serde_json::to_value(value)
        .map(CallToolResult::structured)
        .map_err(internal_error)
}

fn internal_error(error: impl std::fmt::Display) -> McpError {
    McpError::internal_error(error.to_string(), None)
}

fn command_error(error: Error) -> McpError {
    match error {
        Error::NotFound(what) => McpError::invalid_params(format!("{what} not found"), None),
        other => internal_error(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_resource_uris_and_bounds_transcript_limit() {
        assert_eq!(
            parse_resource_uri("anarlog://meetings/meeting-1").unwrap(),
            ResourceRequest::Meeting {
                meeting_id: "meeting-1".to_string()
            }
        );
        assert_eq!(
            parse_resource_uri("anarlog://meetings/meeting-1/transcript?offset=4&limit=900")
                .unwrap(),
            ResourceRequest::Transcript {
                meeting_id: "meeting-1".to_string(),
                offset: 4,
                limit: MAX_TRANSCRIPT_LIMIT,
            }
        );
        assert!(parse_resource_uri("file:///tmp/meeting").is_err());
    }

    #[test]
    fn transcript_page_is_bounded_and_has_next_offset() {
        let transcript = Transcript {
            id: "transcript-1".to_string(),
            source: String::new(),
            provider: String::new(),
            model: String::new(),
            language: "en".to_string(),
            started_at_ms: 0,
            ended_at_ms: None,
            memo: String::new(),
            text: String::new(),
            words: vec![
                serde_json::json!({"text": "one"}),
                serde_json::json!({"text": "two"}),
                serde_json::json!({"text": "three"}),
            ],
            speaker_hints: Vec::new(),
        };
        let page = transcript_page("meeting-1", &[transcript], 1, 1);
        assert_eq!(page.text, "two");
        assert_eq!(page.total_words, 3);
        assert_eq!(page.next_offset, Some(2));
        assert_eq!(page.words[0]["transcript_id"], "transcript-1");
    }

    #[tokio::test]
    async fn server_advertises_tools_and_resources() {
        let db = Arc::new(hypr_db_core::Db::connect_memory_plain().await.unwrap());
        let info = AnarlogMcpServer::new(db).get_info();
        assert!(info.capabilities.tools.is_some());
        assert!(info.capabilities.resources.is_some());
    }

    #[tokio::test]
    async fn list_tool_returns_structured_meeting_data() {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, title, started_at) VALUES ('meeting-1', 'Planning', '2026-07-13')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        let server = AnarlogMcpServer::new(Arc::new(db));

        let result = server
            .list_meetings(Parameters(ListMeetingsParams {
                query: Some("plan".to_string()),
                series_id: None,
                limit: None,
                offset: None,
            }))
            .await
            .unwrap();

        let meetings = result.structured_content.unwrap();
        assert_eq!(meetings[0]["id"], "meeting-1");
        assert_eq!(meetings[0]["title"], "Planning");
    }

    #[tokio::test]
    async fn client_server_handshake_lists_tools_and_resources() {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        let (server_transport, client_transport) = tokio::io::duplex(64 * 1024);
        let server = AnarlogMcpServer::new(Arc::new(db));
        let server_handle = tokio::spawn(async move { server.serve(server_transport).await });

        let client = ().serve(client_transport).await.unwrap();
        let tools = client.list_all_tools().await.unwrap();
        let templates = client.list_all_resource_templates().await.unwrap();

        assert_eq!(tools.len(), 4);
        assert!(tools.iter().any(|tool| tool.name == "list_meetings"));
        assert_eq!(templates.len(), 3);

        client.cancel().await.unwrap();
        let server = server_handle.await.unwrap().unwrap();
        server.cancel().await.unwrap();
    }
}
