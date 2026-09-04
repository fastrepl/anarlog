use anlg_agent_access as access;
use anlg_mcp::McpAuth;
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
        title = "List meetings",
        description = "List Anarlog meetings with optional title/id search, recurring series filter, and pagination.",
        output_schema = rmcp::handler::server::tool::schema_for_type::<access::MeetingPage>(),
        meta = oauth_security_meta(),
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
        let Some(user_id) = user_id(auth) else {
            return Ok(authentication_required(&self.state));
        };
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
        title = "Get meeting",
        description = "Get notes, summaries, participants, and action items for an Anarlog meeting.",
        output_schema = rmcp::handler::server::tool::schema_for_type::<access::Meeting>(),
        meta = oauth_security_meta(),
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
        let Some(user_id) = user_id(auth) else {
            return Ok(authentication_required(&self.state));
        };
        let export = read_export(&self.state, &user_id, &input.meeting_id)
            .await
            .map_err(command_error)?;
        structured(&export.meeting)
    }

    #[tool(
        title = "Get meeting transcript",
        description = "Get a bounded page of transcript words and readable text for an Anarlog meeting. Pass pagination.next_offset as offset to continue.",
        output_schema = rmcp::handler::server::tool::schema_for_type::<access::TranscriptPage>(),
        meta = oauth_security_meta(),
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
        let Some(user_id) = user_id(auth) else {
            return Ok(authentication_required(&self.state));
        };
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
        title = "Get recurring meeting history",
        description = "List meetings in the same recurring series as the supplied meeting, newest first, with pagination metadata.",
        output_schema = rmcp::handler::server::tool::schema_for_type::<access::MeetingPage>(),
        meta = oauth_security_meta(),
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
        let Some(user_id) = user_id(auth) else {
            return Ok(authentication_required(&self.state));
        };
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

    #[tool(
        title = "Export meeting",
        description = "Get a complete Anarlog meeting export with notes, summaries, participants, action items, and transcripts.",
        output_schema = rmcp::handler::server::tool::schema_for_type::<access::MeetingExport>(),
        meta = oauth_security_meta(),
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn export_meeting(
        &self,
        McpAuth(auth): McpAuth,
        Parameters(input): Parameters<access::GetMeetingInput>,
    ) -> Result<CallToolResult, McpError> {
        let Some(user_id) = user_id(auth) else {
            return Ok(authentication_required(&self.state));
        };
        let export = read_export(&self.state, &user_id, &input.meeting_id)
            .await
            .map_err(command_error)?;
        structured(&export)
    }
}

#[tool_handler]
impl ServerHandler for CloudMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(ProtocolVersion::V_2026_07_28)
            .with_server_info(Implementation::new(
                "anarlog",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Read-only hosted access to the user's opted-in Anarlog meeting data. Start with list_meetings, then use get_meeting, get_meeting_transcript, and get_recurring_meeting_history. Use export_meeting only when the task needs the complete record including transcripts. Every tool is idempotent and performs no writes. Report only meetings these tools return. If list_meetings is empty, say there are no opted-in Cloud snapshots. Never invent titles, dates, or ids.",
            )
    }
}

pub(crate) fn mcp_service(
    state: AppState,
) -> rmcp::transport::streamable_http_server::StreamableHttpService<
    CloudMcpServer,
    rmcp::transport::streamable_http_server::session::local::LocalSessionManager,
> {
    anlg_mcp::create_stateless_service(move || {
        Ok(CloudMcpServer {
            state: state.clone(),
        })
    })
}

fn user_id(auth: Option<anlg_api_auth::AuthContext>) -> Option<String> {
    auth.map(|auth| auth.claims.sub)
}

fn oauth_security_meta() -> MetaObject {
    let mut meta = MetaObject::new();
    meta.insert(
        "securitySchemes".to_string(),
        serde_json::json!([{
            "type": "oauth2",
            "scopes": crate::oauth::OAUTH_SCOPES,
        }]),
    );
    meta
}

fn authentication_required(state: &AppState) -> CallToolResult {
    let mut meta = MetaObject::new();
    meta.insert(
        "mcp/www_authenticate".to_string(),
        serde_json::json!([state.oauth().challenge(Some((
            "invalid_token",
            "Connect your Anarlog account to use this tool",
        )))]),
    );
    CallToolResult::error(vec![ContentBlock::text(
        "Connect your Anarlog account to use this tool.",
    )])
    .with_meta(Some(meta))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CloudApiConfig;

    #[test]
    fn server_advertises_the_current_protocol() {
        let state = AppState::new(
            CloudApiConfig::new("https://auth.example.com", "service-role-key").unwrap(),
        );

        assert_eq!(
            CloudMcpServer { state }.get_info().protocol_version,
            ProtocolVersion::V_2026_07_28
        );
    }

    #[test]
    fn every_hosted_tool_declares_oauth_security() {
        for tool in [
            CloudMcpServer::list_meetings_tool_attr(),
            CloudMcpServer::get_meeting_tool_attr(),
            CloudMcpServer::get_meeting_transcript_tool_attr(),
            CloudMcpServer::get_recurring_meeting_history_tool_attr(),
            CloudMcpServer::export_meeting_tool_attr(),
        ] {
            assert_eq!(
                tool.meta.unwrap().get("securitySchemes"),
                Some(&serde_json::json!([{
                    "type": "oauth2",
                    "scopes": [],
                }]))
            );
            assert_eq!(tool.output_schema.unwrap().get("type").unwrap(), "object");
        }
    }

    #[test]
    fn unauthenticated_tool_result_requests_account_connection() {
        let state = AppState::new(
            CloudApiConfig::new("https://auth.example.com", "service-role-key").unwrap(),
        );

        let result = authentication_required(&state);

        assert_eq!(result.is_error, Some(true));
        assert!(
            result
                .meta
                .unwrap()
                .get("mcp/www_authenticate")
                .unwrap()[0]
                .as_str()
                .unwrap()
                .contains("resource_metadata=\"https://api.anarlog.so/.well-known/oauth-protected-resource/mcp\"")
        );
    }
}
