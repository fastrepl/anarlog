use hypr_agent_access as access;
use hypr_mcp::McpAuth;
use rmcp::{
    ErrorData as McpError, ServerHandler, handler::server::wrapper::Parameters, model::*, tool,
    tool_handler, tool_router,
};
use serde::Serialize;

use crate::{
    routes::{
        HistoryQuery, ListMeetingsQuery, history_for_user, list_meetings_for_user, read_export,
    },
    state::AppState,
};

#[derive(Clone)]
pub(crate) struct CloudMcpServer {
    state: AppState,
}

#[tool_router]
impl CloudMcpServer {
    #[tool(
        description = "List Anarlog meetings with optional title/id search, recurring series filter, and pagination.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_meetings(
        &self,
        McpAuth(auth): McpAuth,
        Parameters(input): Parameters<access::ListMeetingsInput>,
    ) -> Result<CallToolResult, McpError> {
        let user_id = user_id(auth)?;
        let page = list_meetings_for_user(
            &self.state,
            &user_id,
            ListMeetingsQuery {
                query: input.query,
                series_id: input.series_id,
                limit: input.limit,
                offset: input.offset,
            },
        )
        .await
        .map_err(command_error)?;
        structured(&page)
    }

    #[tool(
        description = "Get notes, summaries, participants, and action items for an Anarlog meeting.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_meeting(
        &self,
        McpAuth(auth): McpAuth,
        Parameters(input): Parameters<access::GetMeetingInput>,
    ) -> Result<CallToolResult, McpError> {
        let user_id = user_id(auth)?;
        let export = read_export(&self.state, &user_id, &input.meeting_id)
            .await
            .map_err(command_error)?;
        structured(&export.meeting)
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
        McpAuth(auth): McpAuth,
        Parameters(input): Parameters<access::GetMeetingTranscriptInput>,
    ) -> Result<CallToolResult, McpError> {
        let user_id = user_id(auth)?;
        let export = read_export(&self.state, &user_id, &input.meeting_id)
            .await
            .map_err(command_error)?;
        structured(&access::paginate_transcripts(
            &input.meeting_id,
            &export.transcripts,
            input.offset.unwrap_or(0),
            input.limit.unwrap_or(access::DEFAULT_TRANSCRIPT_LIMIT),
        ))
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
        McpAuth(auth): McpAuth,
        Parameters(input): Parameters<access::GetRecurringMeetingHistoryInput>,
    ) -> Result<CallToolResult, McpError> {
        let user_id = user_id(auth)?;
        let page = history_for_user(
            &self.state,
            &user_id,
            input.meeting_id,
            HistoryQuery {
                limit: input.limit,
                offset: input.offset,
            },
        )
        .await
        .map_err(command_error)?;
        structured(&page)
    }
}

#[tool_handler]
impl ServerHandler for CloudMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(ProtocolVersion::LATEST)
            .with_server_info(Implementation::new(
                "anarlog-cloud",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Read-only hosted access to the user's opted-in Anarlog meeting data. Start with list_meetings, then use get_meeting, get_meeting_transcript, and get_recurring_meeting_history. Every tool is idempotent and performs no writes.",
            )
    }
}

pub(crate) fn mcp_service(
    state: AppState,
) -> rmcp::transport::streamable_http_server::StreamableHttpService<
    CloudMcpServer,
    rmcp::transport::streamable_http_server::session::local::LocalSessionManager,
> {
    hypr_mcp::create_service(move || {
        Ok(CloudMcpServer {
            state: state.clone(),
        })
    })
}

fn user_id(auth: Option<hypr_api_auth::AuthContext>) -> Result<String, McpError> {
    auth.map(|auth| auth.claims.sub)
        .ok_or_else(|| McpError::invalid_request("Provide a valid Anarlog cloud API key", None))
}

fn structured(value: &impl Serialize) -> Result<CallToolResult, McpError> {
    serde_json::to_value(value)
        .map(CallToolResult::structured)
        .map_err(|error| McpError::internal_error(error.to_string(), None))
}

fn command_error(error: crate::CloudApiError) -> McpError {
    match error {
        crate::CloudApiError::NotFound(message) | crate::CloudApiError::InvalidRequest(message) => {
            McpError::invalid_params(message, None)
        }
        other => McpError::internal_error(other.to_string(), None),
    }
}
