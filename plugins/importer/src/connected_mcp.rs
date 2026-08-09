use crate::types::{
    ConnectedImportAuthorization, ConnectedImportCredentials, ConnectedImportSyncResult,
    ImportTextFile,
};
use rmcp::{
    Peer, RoleClient, ServiceExt,
    model::{CallToolRequestParams, CallToolResult, JsonObject, Tool},
    transport::{
        auth::{AuthorizationManager, OAuthClientConfig, OAuthState, OAuthTokenResponse},
        streamable_http_client::{
            StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
        },
    },
};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, oneshot};
use url::Url;

const AUTHORIZATION_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const TOOL_TIMEOUT: Duration = Duration::from_secs(45);
const MAX_OAUTH_REQUEST_BYTES: usize = 16 * 1024;
const MAX_LIST_PAGES: usize = 100;
const MEETING_BATCH_SIZE: usize = 25;

#[derive(Clone, Copy)]
struct McpProvider {
    id: &'static str,
    name: &'static str,
    endpoint: &'static str,
    list_tools: &'static [&'static str],
    enrichment_tools: &'static [&'static str],
}

const MCP_PROVIDERS: &[McpProvider] = &[
    McpProvider {
        id: "granola",
        name: "Granola",
        endpoint: "https://mcp.granola.ai/mcp",
        list_tools: &["list_meetings"],
        enrichment_tools: &["get_meetings", "get_meeting_transcript"],
    },
    McpProvider {
        id: "circleback",
        name: "Circleback",
        endpoint: "https://circleback.ai/api/mcp",
        list_tools: &["SearchMeetings"],
        enrichment_tools: &["ReadMeetings", "GetTranscriptsForMeetings"],
    },
    McpProvider {
        id: "fireflies",
        name: "Fireflies.ai",
        endpoint: "https://api.fireflies.ai/mcp",
        list_tools: &["fireflies_get_transcripts", "fireflies_search"],
        enrichment_tools: &[
            "fireflies_fetch",
            "fireflies_get_transcript",
            "fireflies_get_summary",
        ],
    },
    McpProvider {
        id: "krisp",
        name: "Krisp",
        endpoint: "https://mcp.krisp.ai/mcp",
        list_tools: &["search_meetings"],
        enrichment_tools: &["get_document"],
    },
    McpProvider {
        id: "read-ai",
        name: "Read AI",
        endpoint: "https://api.read.ai/mcp",
        list_tools: &["list_meetings"],
        enrichment_tools: &["get_meeting", "get_meeting_by_id"],
    },
    McpProvider {
        id: "fellow",
        name: "Fellow",
        endpoint: "https://fellow.app/mcp",
        list_tools: &["search_meetings"],
        enrichment_tools: &[
            "get_meeting_summary",
            "get_meeting_transcript",
            "get_meeting_participants",
        ],
    },
    McpProvider {
        id: "tactiq",
        name: "Tactiq",
        endpoint: "https://mcp.tactiq.io",
        list_tools: &["list_meetings", "search_meetings"],
        enrichment_tools: &[
            "get_meeting",
            "get_meeting_details",
            "get_meeting_summary",
            "get_meeting_transcript",
        ],
    },
    McpProvider {
        id: "jiminny",
        name: "Jiminny",
        endpoint: "https://mcp.jiminny.com/mcp",
        list_tools: &["search_calls"],
        enrichment_tools: &["get_call"],
    },
];

fn provider(provider_id: &str) -> Result<McpProvider, String> {
    MCP_PROVIDERS
        .iter()
        .copied()
        .find(|provider| provider.id == provider_id)
        .ok_or_else(|| "This app does not support a direct MCP import".to_string())
}

#[derive(Default)]
pub struct ConnectedImportOAuthState {
    pending: Mutex<Option<PendingAuthorization>>,
}

struct PendingAuthorization {
    provider_id: &'static str,
    provider_name: &'static str,
    manager: AuthorizationManager,
    client_secret: Option<String>,
    callback: oneshot::Receiver<Result<AuthorizationCallback, String>>,
}

#[derive(Debug, PartialEq, Eq)]
struct AuthorizationCallback {
    code: String,
    state: String,
}

pub async fn begin_connection(
    provider_id: &str,
    state: &ConnectedImportOAuthState,
) -> Result<ConnectedImportAuthorization, String> {
    let provider = provider(provider_id)?;
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|error| format!("could not start {} sign-in: {error}", provider.name))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("could not prepare {} sign-in: {error}", provider.name))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    let mut manager = AuthorizationManager::new(provider.endpoint)
        .await
        .map_err(|error| auth_error(provider, error))?;
    let metadata = manager
        .discover_metadata()
        .await
        .map_err(|error| auth_error(provider, error))?;
    manager.set_metadata(metadata);
    let scopes = manager.select_scopes(None, &[]);
    let scope_refs = scopes.iter().map(String::as_str).collect::<Vec<_>>();
    let client = manager
        .register_client("Anarlog", &redirect_uri, &scope_refs)
        .await
        .map_err(|error| auth_error(provider, error))?;
    let authorization_url = manager
        .get_authorization_url(&scope_refs)
        .await
        .map_err(|error| auth_error(provider, error))?;

    let (callback_tx, callback_rx) = oneshot::channel();
    tokio::spawn(async move {
        let result = receive_authorization_callback(listener, provider.name).await;
        let _ = callback_tx.send(result);
    });

    *state.pending.lock().await = Some(PendingAuthorization {
        provider_id: provider.id,
        provider_name: provider.name,
        manager,
        client_secret: client.client_secret,
        callback: callback_rx,
    });

    Ok(ConnectedImportAuthorization {
        provider_id: provider.id.to_string(),
        authorization_url,
    })
}

pub async fn complete_connection(
    provider_id: &str,
    state: &ConnectedImportOAuthState,
) -> Result<ConnectedImportCredentials, String> {
    let provider = provider(provider_id)?;
    let Some(PendingAuthorization {
        provider_id: pending_provider_id,
        provider_name,
        manager,
        client_secret,
        callback,
    }) = state.pending.lock().await.take()
    else {
        return Err(format!("Start {} sign-in again", provider.name));
    };
    if pending_provider_id != provider.id {
        return Err(format!("Start {} sign-in again", provider.name));
    }

    let callback = tokio::time::timeout(AUTHORIZATION_TIMEOUT, callback)
        .await
        .map_err(|_| format!("{provider_name} sign-in timed out. Try again."))?
        .map_err(|_| format!("{provider_name} sign-in was interrupted. Try again."))??;

    manager
        .exchange_code_for_token(&callback.code, &callback.state)
        .await
        .map_err(|error| auth_error(provider, error))?;
    credentials_from_manager(provider, &manager, client_secret, Some(now_epoch_secs())).await
}

pub async fn sync(
    provider_id: &str,
    credentials: ConnectedImportCredentials,
    known_meeting_ids: Vec<String>,
) -> Result<ConnectedImportSyncResult, String> {
    let provider = provider(provider_id)?;
    if credentials.provider_id != provider.id {
        return Err(format!("Reconnect {} to keep importing", provider.name));
    }

    let mut oauth = OAuthState::new(provider.endpoint, None)
        .await
        .map_err(|error| auth_error(provider, error))?;
    let token: OAuthTokenResponse = serde_json::from_str(&credentials.token_json)
        .map_err(|_| format!("Reconnect {} to keep importing", provider.name))?;
    oauth
        .set_credentials(&credentials.client_id, token)
        .await
        .map_err(|error| auth_error(provider, error))?;
    let mut token_received_at = credentials.token_received_at;
    if token_needs_refresh(&credentials.token_json, token_received_at) {
        oauth
            .refresh_token()
            .await
            .map_err(|error| auth_error(provider, error))?;
        token_received_at = Some(now_epoch_secs());
    }
    let mut manager = oauth
        .into_authorization_manager()
        .ok_or_else(|| format!("Reconnect {} to keep importing", provider.name))?;
    if let Some(client_secret) = credentials.client_secret.as_deref() {
        manager
            .configure_client(
                OAuthClientConfig::new(&credentials.client_id, provider.endpoint)
                    .with_client_secret(client_secret),
            )
            .map_err(|error| auth_error(provider, error))?;
    }
    let access_token = manager
        .get_access_token()
        .await
        .map_err(|error| auth_error(provider, error))?;

    let transport = StreamableHttpClientTransport::from_config(
        StreamableHttpClientTransportConfig::with_uri(provider.endpoint).auth_header(access_token),
    );
    let service = ()
        .serve(transport)
        .await
        .map_err(|error| format!("could not connect to {}: {error}", provider.name))?;
    let tools = service
        .list_all_tools()
        .await
        .map_err(|error| format!("could not read {} tools: {error}", provider.name))?;

    let list_tool = find_tool(&tools, provider.list_tools).ok_or_else(|| {
        format!(
            "{} did not offer a supported meeting-history tool",
            provider.name
        )
    })?;
    let enrichment_tools = provider
        .enrichment_tools
        .iter()
        .filter_map(|name| find_tool(&tools, std::slice::from_ref(name)))
        .collect::<Vec<_>>();

    let list_payloads = list_all_meetings(provider, service.peer(), list_tool).await?;
    let meetings = meeting_records(&list_payloads);
    let mut warnings = Vec::new();
    if meetings.is_empty() {
        warnings.push(format!(
            "{} did not return any accessible meetings for this account and workspace.",
            provider.name
        ));
    }

    let known = known_meeting_ids.into_iter().collect::<HashSet<_>>();
    let mut enriched = Vec::new();
    let mut unavailable = 0;
    let mut without_content = 0;
    for (meeting_id, mut meeting) in meetings
        .into_iter()
        .filter(|(meeting_id, _)| !known.contains(meeting_id))
    {
        for tool in &enrichment_tools {
            let requests = meeting_arguments(tool, std::slice::from_ref(&meeting_id));
            if requests.is_empty() {
                unavailable += 1;
                continue;
            }
            let mut payloads = Vec::new();
            for arguments in requests {
                match call_tool(provider, service.peer(), tool, arguments).await {
                    Ok(values) => payloads.extend(values),
                    Err(_) => unavailable += 1,
                }
            }
            merge_enrichment(&mut meeting, tool.name.as_ref(), payloads);
        }
        if meeting_has_content(&meeting) {
            enriched.push((meeting_id, meeting));
        } else {
            without_content += 1;
        }
    }

    if unavailable > 0 {
        warnings.push(format!(
            "{unavailable} {} detail requests were unavailable. Meetings with usable notes or transcripts were still imported.",
            provider.name
        ));
    }
    if without_content > 0 {
        warnings.push(format!(
            "{without_content} {} meetings did not include notes or transcripts for this account or plan.",
            provider.name
        ));
    }

    let files = meeting_files(provider, enriched);
    let refreshed_credentials = credentials_from_manager(
        provider,
        &manager,
        credentials.client_secret,
        token_received_at,
    )
    .await?;
    let _ = service.cancel().await;

    Ok(ConnectedImportSyncResult {
        files,
        credentials: refreshed_credentials,
        warnings,
    })
}

async fn receive_authorization_callback(
    listener: TcpListener,
    provider_name: &'static str,
) -> Result<AuthorizationCallback, String> {
    let (mut stream, _) = listener
        .accept()
        .await
        .map_err(|error| format!("{provider_name} sign-in callback failed: {error}"))?;
    let result = read_authorization_callback(&mut stream, provider_name).await;
    let (status, title, message) = match &result {
        Ok(_) => (
            "200 OK",
            format!("{provider_name} connected"),
            "Your meeting history is being brought into Anarlog. You can close this window.",
        ),
        Err(_) => (
            "400 Bad Request",
            format!("{provider_name} connection failed"),
            "Return to Anarlog and try connecting again.",
        ),
    };
    let body = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>{title}</title><body style=\"font:16px system-ui;padding:48px;max-width:560px\"><h1>{title}</h1><p>{message}</p></body>"
    );
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
    result
}

async fn read_authorization_callback(
    stream: &mut TcpStream,
    provider_name: &str,
) -> Result<AuthorizationCallback, String> {
    let mut request = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let read = stream
            .read(&mut chunk)
            .await
            .map_err(|error| format!("could not read {provider_name} sign-in: {error}"))?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if request.len() >= MAX_OAUTH_REQUEST_BYTES {
            return Err(format!("{provider_name} sign-in response was too large"));
        }
    }
    parse_authorization_request(&String::from_utf8_lossy(&request), provider_name)
}

fn parse_authorization_request(
    request: &str,
    provider_name: &str,
) -> Result<AuthorizationCallback, String> {
    let request_target = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .ok_or_else(|| format!("{provider_name} sign-in response was invalid"))?;
    let callback_url = Url::parse(&format!("http://127.0.0.1{request_target}"))
        .map_err(|_| format!("{provider_name} sign-in response was invalid"))?;
    let query: HashMap<_, _> = callback_url.query_pairs().into_owned().collect();
    if let Some(error) = query.get("error") {
        let description = query
            .get("error_description")
            .map(String::as_str)
            .unwrap_or(error);
        return Err(format!("{provider_name} denied access: {description}"));
    }
    let code = query
        .get("code")
        .filter(|value| !value.is_empty())
        .cloned()
        .ok_or_else(|| format!("{provider_name} sign-in did not return an authorization code"))?;
    let state = query
        .get("state")
        .filter(|value| !value.is_empty())
        .cloned()
        .ok_or_else(|| format!("{provider_name} sign-in did not return a security state"))?;
    Ok(AuthorizationCallback { code, state })
}

async fn credentials_from_manager(
    provider: McpProvider,
    manager: &AuthorizationManager,
    client_secret: Option<String>,
    token_received_at: Option<u64>,
) -> Result<ConnectedImportCredentials, String> {
    let (client_id, token) = manager
        .get_credentials()
        .await
        .map_err(|error| auth_error(provider, error))?;
    credentials(provider, client_id, client_secret, token, token_received_at)
}

fn credentials(
    provider: McpProvider,
    client_id: String,
    client_secret: Option<String>,
    token: Option<OAuthTokenResponse>,
    token_received_at: Option<u64>,
) -> Result<ConnectedImportCredentials, String> {
    let token =
        token.ok_or_else(|| format!("{} did not return access credentials", provider.name))?;
    let token_json = serde_json::to_string(&token)
        .map_err(|error| format!("could not save {} access: {error}", provider.name))?;
    Ok(ConnectedImportCredentials {
        provider_id: provider.id.to_string(),
        client_id,
        client_secret,
        token_json,
        token_received_at,
    })
}

fn token_needs_refresh(token_json: &str, token_received_at: Option<u64>) -> bool {
    let Ok(token) = serde_json::from_str::<Value>(token_json) else {
        return false;
    };
    let has_refresh_token = token
        .get("refresh_token")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.is_empty());
    if !has_refresh_token {
        return false;
    }
    let Some(received_at) = token_received_at else {
        return true;
    };
    let Some(expires_in) = token.get("expires_in").and_then(Value::as_u64) else {
        return false;
    };
    now_epoch_secs().saturating_add(30) >= received_at.saturating_add(expires_in)
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn auth_error(provider: McpProvider, error: impl std::fmt::Display) -> String {
    format!("{} connection failed: {error}", provider.name)
}

fn find_tool<'a>(tools: &'a [Tool], candidates: &[&str]) -> Option<&'a Tool> {
    tools.iter().find(|tool| {
        let actual = normalized_tool_name(tool.name.as_ref());
        candidates.iter().any(|candidate| {
            let candidate = normalized_tool_name(candidate);
            actual == candidate || actual.ends_with(&candidate)
        })
    })
}

fn normalized_tool_name(name: &str) -> String {
    name.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

async fn list_all_meetings(
    provider: McpProvider,
    peer: &Peer<RoleClient>,
    tool: &Tool,
) -> Result<Vec<Value>, String> {
    let mut payloads = Vec::new();
    let mut cursor: Option<String> = None;
    let cursor_property = schema_property(tool, &["cursor", "next_cursor", "nextCursor"]);
    let offset_property = schema_property(tool, &["skip", "offset"]);
    let mut offset = 0_u64;

    for _ in 0..MAX_LIST_PAGES {
        let mut arguments = default_list_arguments(tool, offset);
        if let (Some(property), Some(value)) = (&cursor_property, &cursor) {
            arguments.insert(property.clone(), Value::String(value.clone()));
        }
        let page = call_tool(provider, peer, tool, arguments).await?;
        let next = next_cursor(&page);
        let page_meeting_count = meeting_records(&page).len();
        payloads.extend(page);
        if let Some(next) = next {
            if cursor.as_ref() == Some(&next) || cursor_property.is_none() {
                break;
            }
            cursor = Some(next);
        } else if offset_property.is_some() && page_meeting_count >= 50 {
            offset += 50;
        } else {
            break;
        }
    }
    Ok(payloads)
}

async fn call_tool(
    provider: McpProvider,
    peer: &Peer<RoleClient>,
    tool: &Tool,
    arguments: JsonObject,
) -> Result<Vec<Value>, String> {
    let request = CallToolRequestParams::new(tool.name.clone()).with_arguments(arguments);
    let result = tokio::time::timeout(TOOL_TIMEOUT, peer.call_tool(request))
        .await
        .map_err(|_| format!("{}'s {} request timed out", provider.name, tool.name))?
        .map_err(|error| format!("{}'s {} request failed: {error}", provider.name, tool.name))?;
    tool_payloads(provider, result)
}

fn tool_payloads(provider: McpProvider, result: CallToolResult) -> Result<Vec<Value>, String> {
    let text = result
        .content
        .iter()
        .filter_map(|content| content.as_text().map(|text| text.text.trim()))
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();
    if result.is_error.unwrap_or(false) {
        return Err(if text.is_empty() {
            format!("{} returned an import error", provider.name)
        } else {
            text.join("\n")
        });
    }

    let mut payloads = Vec::new();
    if let Some(structured) = result.structured_content {
        payloads.push(structured);
    }
    for text in text {
        if let Some(value) = parse_json_text(text) {
            payloads.push(value);
        } else {
            payloads.push(Value::String(text.to_string()));
        }
    }
    Ok(payloads)
}

fn parse_json_text(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    if let Ok(value) = serde_json::from_str(trimmed) {
        return Some(value);
    }

    let unfenced = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|value| value.strip_suffix("```"))
        .map(str::trim);
    if let Some(value) = unfenced
        && let Ok(parsed) = serde_json::from_str(value)
    {
        return Some(parsed);
    }

    let start = trimmed.find(['{', '['])?;
    let end = trimmed.rfind(['}', ']'])?;
    serde_json::from_str(trimmed.get(start..=end)?).ok()
}

fn default_list_arguments(tool: &Tool, offset: u64) -> JsonObject {
    let mut arguments = Map::new();
    if let Some(property) = schema_property(tool, &["limit", "page_size", "pageSize"]) {
        arguments.insert(property, Value::Number(50.into()));
    }
    if let Some(property) = schema_property(tool, &["skip", "offset"]) {
        arguments.insert(property, Value::Number(offset.into()));
    }
    if let Some(property) = schema_property(tool, &["format"]) {
        arguments.insert(property, Value::String("json".to_string()));
    }

    let required = tool
        .input_schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str);
    for property in required {
        if arguments.contains_key(property) {
            continue;
        }
        match property {
            "query" | "keyword" | "search" | "search_query" | "searchQuery" => {
                arguments.insert(property.to_string(), Value::String(String::new()));
            }
            "from" | "fromDate" | "from_date" | "startDate" | "start_date" => {
                arguments.insert(
                    property.to_string(),
                    Value::String("1970-01-01".to_string()),
                );
            }
            "to" | "toDate" | "to_date" | "endDate" | "end_date" => {
                arguments.insert(
                    property.to_string(),
                    Value::String("2100-01-01".to_string()),
                );
            }
            _ => {}
        }
    }
    arguments
}

fn meeting_arguments(tool: &Tool, ids: &[String]) -> Vec<JsonObject> {
    if ids.is_empty() {
        return Vec::new();
    }
    let Some(properties) = tool
        .input_schema
        .get("properties")
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };

    for candidate in [
        "meeting_ids",
        "meetingIds",
        "transcript_ids",
        "transcriptIds",
        "document_ids",
        "documentIds",
        "call_ids",
        "callIds",
        "recording_ids",
        "recordingIds",
        "ids",
        "meetings",
    ] {
        if properties
            .get(candidate)
            .and_then(|schema| schema.get("type"))
            .and_then(Value::as_str)
            == Some("array")
        {
            return ids
                .chunks(MEETING_BATCH_SIZE)
                .map(|chunk| {
                    let mut arguments = Map::from_iter([(
                        candidate.to_string(),
                        Value::Array(chunk.iter().cloned().map(Value::String).collect()),
                    )]);
                    add_enrichment_defaults(tool, &mut arguments);
                    arguments
                })
                .collect();
        }
    }

    for candidate in [
        "meeting_id",
        "meetingId",
        "transcript_id",
        "transcriptId",
        "document_id",
        "documentId",
        "call_id",
        "callId",
        "recording_id",
        "recordingId",
        "uuid",
        "id",
    ] {
        if properties.contains_key(candidate) {
            return ids
                .iter()
                .map(|id| {
                    let mut arguments =
                        Map::from_iter([(candidate.to_string(), Value::String(id.clone()))]);
                    add_enrichment_defaults(tool, &mut arguments);
                    arguments
                })
                .collect();
        }
    }
    Vec::new()
}

fn add_enrichment_defaults(tool: &Tool, arguments: &mut JsonObject) {
    for candidate in ["include_transcript", "includeTranscript"] {
        if schema_property(tool, &[candidate]).is_some() {
            arguments.insert(candidate.to_string(), Value::Bool(true));
        }
    }
    for candidate in ["include_call_insights", "includeCallInsights"] {
        if schema_property(tool, &[candidate]).is_some() {
            arguments.insert(candidate.to_string(), Value::Bool(true));
        }
    }
    for candidate in ["include_participants", "includeParticipants"] {
        if schema_property(tool, &[candidate]).is_some() {
            arguments.insert(candidate.to_string(), Value::Bool(true));
        }
    }
    if let Some(property) = schema_property(tool, &["format"]) {
        arguments.insert(property, Value::String("json".to_string()));
    }
}

fn schema_property(tool: &Tool, candidates: &[&str]) -> Option<String> {
    let properties = tool
        .input_schema
        .get("properties")
        .and_then(Value::as_object)?;
    candidates
        .iter()
        .find(|candidate| properties.contains_key(**candidate))
        .map(|candidate| (*candidate).to_string())
}

fn meeting_records(payloads: &[Value]) -> Vec<(String, Value)> {
    let mut meetings = Vec::new();
    let mut seen = HashSet::new();
    for payload in payloads {
        collect_meeting_records(payload, &mut meetings, &mut seen);
    }
    meetings
}

fn collect_meeting_records(
    value: &Value,
    meetings: &mut Vec<(String, Value)>,
    seen: &mut HashSet<String>,
) {
    match value {
        Value::Array(values) => {
            for value in values {
                collect_meeting_records(value, meetings, seen);
            }
        }
        Value::Object(record) => {
            if has_meeting_title(record)
                && let Some(id) = meeting_id(record)
                && seen.insert(id.clone())
            {
                meetings.push((id, value.clone()));
                return;
            }
            for value in record.values() {
                collect_meeting_records(value, meetings, seen);
            }
        }
        _ => {}
    }
}

fn has_meeting_title(record: &Map<String, Value>) -> bool {
    [
        "title",
        "name",
        "subject",
        "meeting_title",
        "meetingTitle",
        "call_title",
        "callTitle",
        "recording_title",
        "recordingTitle",
        "meeting_name",
        "meetingName",
        "topic",
    ]
    .iter()
    .any(|key| record.get(*key).and_then(Value::as_str).is_some())
}

fn meeting_has_content(meeting: &Value) -> bool {
    find_value(
        meeting,
        &[
            "notes",
            "private_notes",
            "privateNotes",
            "enhanced_notes",
            "enhancedNotes",
            "meeting_notes",
            "meetingNotes",
            "summary",
            "transcript",
            "transcription",
            "sentences",
            "segments",
            "utterances",
            "content",
            "ai_notes",
            "aiNotes",
            "action_items",
            "actionItems",
            "key_points",
            "keyPoints",
            "takeaways",
        ],
    )
    .is_some_and(has_value)
}

fn has_value(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(value) => !value.trim().is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
        Value::Bool(_) | Value::Number(_) => true,
    }
}

fn meeting_id(record: &Map<String, Value>) -> Option<String> {
    [
        "id",
        "meeting_id",
        "meetingId",
        "transcript_id",
        "transcriptId",
        "document_id",
        "documentId",
        "call_id",
        "callId",
        "recording_id",
        "recordingId",
        "uuid",
    ]
    .iter()
    .find_map(|key| match record.get(*key) {
        Some(Value::String(value)) if !value.is_empty() => Some(value.clone()),
        Some(Value::Number(value)) => Some(value.to_string()),
        _ => None,
    })
}

fn next_cursor(payloads: &[Value]) -> Option<String> {
    for payload in payloads {
        if let Some(cursor) = find_string(payload, &["next_cursor", "nextCursor"]) {
            return Some(cursor);
        }
    }
    None
}

fn transcript_value(payloads: &[Value]) -> Option<Value> {
    for payload in payloads {
        if let Some(value) = find_value(
            payload,
            &[
                "transcript",
                "raw_transcript",
                "rawTranscript",
                "segments",
                "utterances",
                "sentences",
            ],
        ) {
            return Some(value.clone());
        }
        if let Value::String(text) = payload
            && !text.trim().is_empty()
        {
            return Some(Value::String(text.clone()));
        }
    }
    None
}

fn merge_enrichment(meeting: &mut Value, tool_name: &str, payloads: Vec<Value>) {
    let Some(record) = meeting.as_object_mut() else {
        return;
    };
    let normalized_tool = normalized_tool_name(tool_name);

    if normalized_tool.contains("transcript")
        && let Some(transcript) = transcript_value(&payloads)
    {
        record.insert("transcript".to_string(), transcript);
    }
    if normalized_tool.contains("summary")
        && let Some(summary) = payloads
            .iter()
            .find_map(|payload| find_value(payload, &["summary", "overview", "content"]))
            .cloned()
            .or_else(|| payloads.iter().find(|payload| payload.is_string()).cloned())
    {
        record.insert("summary".to_string(), summary);
    }

    for (target, keys) in [
        ("summary", &["summary", "overview", "synopsis"][..]),
        (
            "notes",
            &[
                "notes",
                "meeting_notes",
                "meetingNotes",
                "ai_notes",
                "aiNotes",
                "key_points",
                "keyPoints",
            ][..],
        ),
        (
            "attendees",
            &["attendees", "participants", "people", "invitees"][..],
        ),
        (
            "action_items",
            &[
                "action_items",
                "actionItems",
                "tasks",
                "next_steps",
                "nextSteps",
            ][..],
        ),
    ] {
        if record.contains_key(target) {
            continue;
        }
        if let Some(value) = payloads
            .iter()
            .find_map(|payload| find_value(payload, keys))
            .filter(|value| has_value(value))
        {
            record.insert(target.to_string(), value.clone());
        }
    }

    for payload in payloads {
        if let Value::Object(payload_record) = payload {
            for (key, value) in payload_record {
                record.entry(key).or_insert(value);
            }
        }
    }
}

fn find_string(value: &Value, keys: &[&str]) -> Option<String> {
    find_value(value, keys)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn find_value<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    match value {
        Value::Array(values) => values.iter().find_map(|value| find_value(value, keys)),
        Value::Object(record) => {
            for key in keys {
                if let Some(value) = record.get(*key) {
                    return Some(value);
                }
            }
            record.values().find_map(|value| find_value(value, keys))
        }
        _ => None,
    }
}

fn meeting_files(provider: McpProvider, meetings: Vec<(String, Value)>) -> Vec<ImportTextFile> {
    meetings
        .into_iter()
        .filter_map(|(id, meeting)| {
            let content = serde_json::to_string(&meeting).ok()?;
            let safe_id = safe_file_component(&id);
            Some(ImportTextFile {
                path: format!("mcp://{}/{safe_id}.json", provider.id),
                name: format!("{safe_id}.json"),
                content,
            })
        })
        .collect()
}

fn safe_file_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "meeting".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(schema: Value) -> Tool {
        Tool::new(
            "get_meetings",
            "Get meeting details",
            schema.as_object().unwrap().clone(),
        )
    }

    #[test]
    fn parses_oauth_callback_and_rejects_denials() {
        assert_eq!(
            parse_authorization_request(
                "GET /callback?code=abc%20123&state=state-1 HTTP/1.1\r\nHost: localhost\r\n\r\n",
                "Granola",
            )
            .unwrap(),
            AuthorizationCallback {
                code: "abc 123".to_string(),
                state: "state-1".to_string(),
            }
        );
        assert!(
            parse_authorization_request(
                "GET /callback?error=access_denied&error_description=Nope HTTP/1.1\r\n\r\n",
                "Granola",
            )
            .unwrap_err()
            .contains("Nope")
        );
    }

    #[test]
    fn batches_ids_using_the_advertised_tool_schema() {
        let tool = tool(serde_json::json!({
            "type": "object",
            "properties": { "meeting_ids": { "type": "array" } }
        }));
        let ids = (0..51)
            .map(|index| format!("m-{index}"))
            .collect::<Vec<_>>();
        let requests = meeting_arguments(&tool, &ids);

        assert_eq!(requests.len(), 3);
        assert_eq!(
            requests[0]["meeting_ids"].as_array().unwrap().len(),
            MEETING_BATCH_SIZE
        );
    }

    #[test]
    fn extracts_stable_meeting_files_without_nested_people() {
        let payloads = vec![serde_json::json!({
            "meetings": [{
                "id": "meeting/one",
                "title": "Weekly planning",
                "notes": "Ship it",
                "attendees": [{ "id": "person-one", "name": "Ada" }]
            }]
        })];
        let meetings = meeting_records(&payloads);
        let files = meeting_files(provider("granola").unwrap(), meetings);

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "mcp://granola/meeting-one.json");
        assert!(files[0].content.contains("Weekly planning"));
    }

    #[test]
    fn matches_provider_tool_names_across_case_and_separators() {
        let tools = vec![Tool::new(
            "circleback_SearchMeetings",
            "Search meetings",
            Map::new(),
        )];

        assert!(find_tool(&tools, &["SearchMeetings"]).is_some());
    }

    #[test]
    fn merges_transcript_enrichment_into_a_meeting_record() {
        let mut meeting = serde_json::json!({
            "id": "meeting-one",
            "title": "Weekly planning"
        });
        merge_enrichment(
            &mut meeting,
            "fireflies_get_transcript",
            vec![serde_json::json!({
                "sentences": [{ "speaker": "Ada", "text": "Ship it" }]
            })],
        );

        assert!(meeting_has_content(&meeting));
        assert_eq!(meeting["transcript"][0]["text"], "Ship it");
    }

    #[test]
    fn accepts_json_inside_a_text_wrapper() {
        assert_eq!(
            parse_json_text("Result:\n```json\n{\"meetings\":[]}\n```").unwrap()["meetings"],
            serde_json::json!([])
        );
    }

    #[test]
    fn refreshes_legacy_and_expired_rotating_tokens() {
        let token = serde_json::json!({
            "access_token": "access",
            "refresh_token": "refresh",
            "expires_in": 60
        })
        .to_string();

        assert!(token_needs_refresh(&token, None));
        assert!(token_needs_refresh(
            &token,
            Some(now_epoch_secs().saturating_sub(60))
        ));
        assert!(!token_needs_refresh(&token, Some(now_epoch_secs())));
    }
}
