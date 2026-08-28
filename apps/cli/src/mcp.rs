use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt, service::RequestContext, tool,
    tool_handler, tool_router,
};
use serde::Serialize;

use crate::Error;
use anlg_agent_access as access;

#[derive(Clone)]
struct AnarlogMcpServer {
    db: Arc<anlg_db_core::Db>,
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
    fn new(db: Arc<anlg_db_core::Db>) -> Self {
        Self { db }
    }
}

#[tool_router]
impl AnarlogMcpServer {
    #[tool(
        description = "List recent Anarlog meetings with pagination metadata. Use query to narrow by title or meeting id, then pass next_offset as offset to continue.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_meetings(
        &self,
        Parameters(input): Parameters<access::ListMeetingsInput>,
    ) -> std::result::Result<CallToolResult, McpError> {
        let page = access::list_meetings(self.db.pool(), input)
            .await
            .map_err(command_error)?;
        structured(&page)
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
        Parameters(input): Parameters<access::GetMeetingInput>,
    ) -> std::result::Result<CallToolResult, McpError> {
        let meeting = access::get_meeting(self.db.pool(), input)
            .await
            .map_err(command_error)?;
        structured(&meeting)
    }

    #[tool(
        description = "Get a bounded page of transcript words and readable text for an Anarlog meeting. Pass pagination.next_offset as offset to continue.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_meeting_transcript(
        &self,
        Parameters(input): Parameters<access::GetMeetingTranscriptInput>,
    ) -> std::result::Result<CallToolResult, McpError> {
        let page = access::get_meeting_transcript(self.db.pool(), input)
            .await
            .map_err(command_error)?;
        structured(&page)
    }

    #[tool(
        description = "List meetings in the same recurring series as the supplied meeting, newest first, with pagination metadata.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_recurring_meeting_history(
        &self,
        Parameters(input): Parameters<access::GetRecurringMeetingHistoryInput>,
    ) -> std::result::Result<CallToolResult, McpError> {
        let page = access::get_recurring_meeting_history(self.db.pool(), input)
            .await
            .map_err(command_error)?;
        structured(&page)
    }

    #[tool(
        description = "Propose a complete summary replacement. The proposal stays pending until a human applies it in the Anarlog desktop app. Specify target_id when the meeting has multiple summaries.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn propose_summary_edit(
        &self,
        Parameters(input): Parameters<ProposeSummaryInput>,
    ) -> std::result::Result<CallToolResult, McpError> {
        let proposal = access::create_proposal(
            self.db.pool(),
            access::CreateProposalInput {
                meeting_id: input.meeting_id,
                kind: "summary_replace".to_string(),
                target_id: input.target_id,
                content: input.content,
                source: Some("mcp".to_string()),
            },
        )
        .await
        .map_err(command_error)?;
        structured(&proposal)
    }

    #[tool(
        description = "Propose a complete memo replacement. The proposal stays pending until a human applies it in the Anarlog desktop app.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn propose_memo_edit(
        &self,
        Parameters(input): Parameters<ProposeMemoInput>,
    ) -> std::result::Result<CallToolResult, McpError> {
        let proposal = access::create_proposal(
            self.db.pool(),
            access::CreateProposalInput {
                meeting_id: input.meeting_id,
                kind: "memo_replace".to_string(),
                target_id: None,
                content: input.content,
                source: Some("mcp".to_string()),
            },
        )
        .await
        .map_err(command_error)?;
        structured(&proposal)
    }

    #[tool(
        description = "List staged Anarlog meeting proposals. Defaults to pending proposals. Pass status all to include applied and declined rows, and next_offset as offset to continue.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_proposals(
        &self,
        Parameters(input): Parameters<access::ListProposalsInput>,
    ) -> std::result::Result<CallToolResult, McpError> {
        let page = access::list_proposals(self.db.pool(), input)
            .await
            .map_err(command_error)?;
        structured(&page)
    }

    #[tool(
        description = "Get one staged Anarlog proposal, including its unified diff. The proposal is not applied.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_proposal(
        &self,
        Parameters(input): Parameters<access::GetProposalInput>,
    ) -> std::result::Result<CallToolResult, McpError> {
        let proposal = access::get_proposal(self.db.pool(), input)
            .await
            .map_err(command_error)?;
        structured(&proposal)
    }

    #[tool(
        description = "Decline a pending proposal without changing the meeting. Applied proposals cannot be declined.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn decline_proposal(
        &self,
        Parameters(input): Parameters<access::DeclineProposalInput>,
    ) -> std::result::Result<CallToolResult, McpError> {
        let proposal = access::decline_proposal(self.db.pool(), input)
            .await
            .map_err(command_error)?;
        structured(&proposal)
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
struct ProposeSummaryInput {
    meeting_id: String,
    content: String,
    target_id: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
struct ProposeMemoInput {
    meeting_id: String,
    content: String,
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
            "Local access to Anarlog meeting data. Start with list_meetings to resolve a meeting_id, then call get_meeting for notes, summaries, participants, and action items. Request transcript pages with get_meeting_transcript and continue with pagination.next_offset; each page is capped at 500 words. Use get_recurring_meeting_history for series context. To persist an edit, call propose_summary_edit or propose_memo_edit; the result stays pending until a human applies it in the desktop app. List or inspect staged work with list_proposals and get_proposal. decline_proposal discards a pending proposal without changing the meeting. Never invent meeting titles, dates, or ids. If list_meetings returns no meetings, say so. Never access SQLite directly, or claim a proposal was applied. Documentation: https://docs.anarlog.so",
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
        let page = access::list_meetings(
            self.db.pool(),
            access::ListMeetingsInput {
                query: None,
                series_id: None,
                limit: Some(access::DEFAULT_LIST_LIMIT),
                offset: Some(offset),
            },
        )
        .await
        .map_err(command_error)?;
        let next_cursor = page.pagination.next_offset.map(|offset| offset.to_string());
        let resources = page
            .meetings
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
                let meeting =
                    access::get_meeting(self.db.pool(), access::GetMeetingInput { meeting_id })
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
                let page = access::get_meeting_transcript(
                    self.db.pool(),
                    access::GetMeetingTranscriptInput {
                        meeting_id,
                        offset: Some(offset),
                        limit: Some(limit),
                    },
                )
                .await
                .map_err(command_error)?;
                ResourceContents::text(page.text, params.uri).with_mime_type("text/plain")
            }
            ResourceRequest::Series { series_id } => {
                let page = access::list_meetings(
                    self.db.pool(),
                    access::ListMeetingsInput {
                        query: None,
                        series_id: Some(series_id),
                        limit: Some(100),
                        offset: Some(0),
                    },
                )
                .await
                .map_err(command_error)?;
                let text = page
                    .meetings
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

pub async fn serve(db: Arc<anlg_db_core::Db>) -> crate::Result<()> {
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
            let mut limit = access::DEFAULT_TRANSCRIPT_LIMIT;
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
                limit: limit.clamp(1, access::MAX_TRANSCRIPT_LIMIT),
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

fn command_error(error: access::Error) -> McpError {
    match error {
        access::Error::NotFound(what) => {
            McpError::invalid_params(format!("{what} not found"), None)
        }
        other => internal_error(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

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
                limit: access::MAX_TRANSCRIPT_LIMIT,
            }
        );
        assert!(parse_resource_uri("file:///tmp/meeting").is_err());
    }

    #[tokio::test]
    async fn server_advertises_tools_and_resources() {
        let db = Arc::new(anlg_db_core::Db::connect_memory_plain().await.unwrap());
        let info = AnarlogMcpServer::new(db).get_info();
        assert!(info.capabilities.tools.is_some());
        assert!(info.capabilities.resources.is_some());
        let instructions = info.instructions.unwrap();
        assert!(instructions.contains("Start with list_meetings"));
        assert!(instructions.contains("https://docs.anarlog.so"));
        assert!(instructions.contains("propose_summary_edit"));
        assert!(instructions.contains("claim a proposal was applied"));
        assert!(instructions.contains("Never invent meeting titles"));
    }

    #[tokio::test]
    async fn list_tool_returns_structured_meeting_data() {
        let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, title, started_at) VALUES ('meeting-1', 'Planning', '2026-07-13')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        let server = AnarlogMcpServer::new(Arc::new(db));

        let result = server
            .list_meetings(Parameters(access::ListMeetingsInput {
                query: Some("plan".to_string()),
                series_id: None,
                limit: None,
                offset: None,
            }))
            .await
            .unwrap();

        let meetings = result.structured_content.unwrap();
        assert_eq!(meetings["meetings"][0]["id"], "meeting-1");
        assert_eq!(meetings["meetings"][0]["title"], "Planning");
        assert_eq!(meetings["pagination"]["returned"], 1);
        assert!(meetings["pagination"]["next_offset"].is_null());
    }

    #[tokio::test]
    async fn client_server_handshake_lists_tools_and_resources() {
        let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, title, started_at) VALUES ('meeting-1', 'Planning', '2026-07-13')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        let (server_transport, client_transport) = tokio::io::duplex(64 * 1024);
        let server = AnarlogMcpServer::new(Arc::new(db));
        let info = server.get_info();
        let server_handle = tokio::spawn(async move { server.serve(server_transport).await });

        let client = ().serve(client_transport).await.unwrap();
        let tools = client.list_all_tools().await.unwrap();
        let templates = client.list_all_resource_templates().await.unwrap();
        let resources = client.list_all_resources().await.unwrap();
        insta::assert_json_snapshot!(
            "mcp_contract",
            canonicalize_json(serde_json::json!({
                "protocol_version": info.protocol_version,
                "instructions": info.instructions,
                "tools": tools,
                "resource_templates": templates,
            }))
        );

        let mut tool_names = tools
            .iter()
            .map(|tool| tool.name.to_string())
            .collect::<Vec<_>>();
        tool_names.sort();
        assert_eq!(
            tool_names,
            [
                "decline_proposal",
                "get_meeting",
                "get_meeting_transcript",
                "get_proposal",
                "get_recurring_meeting_history",
                "list_meetings",
                "list_proposals",
                "propose_memo_edit",
                "propose_summary_edit",
            ]
        );
        let mcp_docs = include_str!("../../../docs/reference/mcp.mdx");
        let mcp_skill = include_str!("../../../skills/anarlog/references/mcp.md");
        for tool_name in &tool_names {
            assert!(
                mcp_docs.contains(tool_name),
                "MCP docs are missing `{tool_name}`"
            );
            assert!(
                mcp_skill.contains(tool_name),
                "Anarlog skill is missing `{tool_name}`"
            );
        }
        for tool in tools {
            let properties = tool
                .input_schema
                .get("properties")
                .and_then(Value::as_object)
                .expect("tool input properties");
            for parameter in properties.keys() {
                assert!(
                    mcp_docs.contains(&format!("`{parameter}`")),
                    "MCP docs are missing `{parameter}`"
                );
            }
            let annotations = tool.annotations.expect("tool annotations");
            let write_tool = matches!(
                tool.name.as_ref(),
                "propose_summary_edit" | "propose_memo_edit" | "decline_proposal"
            );
            assert_eq!(annotations.read_only_hint, Some(!write_tool));
            assert_eq!(annotations.destructive_hint, Some(false));
            assert_eq!(annotations.idempotent_hint, Some(!write_tool));
            assert_eq!(annotations.open_world_hint, Some(false));
        }

        let mut template_contract = templates
            .iter()
            .map(|template| {
                (
                    template.raw.name.clone(),
                    template.raw.uri_template.clone(),
                    template.annotations.clone(),
                )
            })
            .collect::<Vec<_>>();
        template_contract.sort_by(|left, right| left.1.cmp(&right.1));
        assert_eq!(
            template_contract,
            [
                (
                    "Anarlog meeting".to_string(),
                    "anarlog://meetings/{meeting_id}".to_string(),
                    None,
                ),
                (
                    "Anarlog meeting transcript".to_string(),
                    "anarlog://meetings/{meeting_id}/transcript{?offset,limit}".to_string(),
                    None,
                ),
                (
                    "Anarlog meeting series".to_string(),
                    "anarlog://series/{series_id}".to_string(),
                    None,
                ),
            ]
        );
        for (_, uri, _) in &template_contract {
            assert!(mcp_docs.contains(uri), "MCP docs are missing `{uri}`");
            assert!(mcp_skill.contains(uri), "Anarlog skill is missing `{uri}`");
        }
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].raw.name, "Planning");
        assert_eq!(resources[0].raw.uri, "anarlog://meetings/meeting-1");
        assert!(resources[0].annotations.is_none());

        client.cancel().await.unwrap();
        let server = server_handle.await.unwrap().unwrap();
        server.cancel().await.unwrap();
    }

    fn canonicalize_json(value: Value) -> Value {
        match value {
            Value::Object(object) => Value::Object(
                object
                    .into_iter()
                    .map(|(key, value)| (key, canonicalize_json(value)))
                    .collect::<std::collections::BTreeMap<_, _>>()
                    .into_iter()
                    .collect(),
            ),
            Value::Array(values) => {
                Value::Array(values.into_iter().map(canonicalize_json).collect())
            }
            value => value,
        }
    }
}
